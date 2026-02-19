use std::{collections::HashMap, hint::black_box};

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use etl::types::{Cell, Event, InsertEvent, TableId, TableRow};
use pgsense_rs::{
    events::{self, Action, ColumnMeta, ColumnValue, ScanEvent, TableMeta},
    rules::{
        config::{RuleConfig, RuleType, Severity},
        engine::RuleEngine,
    },
    scanner::{ScanFilter, Scanner},
};

// Single trivial rule — keeps rule cost near-zero so benchmarks measure scanner
// machinery, not regex speed. Users bring their own rules; we bench our
// overhead.
fn noop_rule() -> Vec<RuleConfig> {
    vec![RuleConfig {
        id: "bench-match".into(),
        description: "bench".into(),
        category: "BENCH".into(),
        severity: Severity::Low,
        rule_type: RuleType::Regex,
        pattern: Some(r"MATCH_THIS_SENTINEL".into()),
        validate: None,
        builtin: None,
        script: None,
        allowlist: None,
        scope: None,
    }]
}

fn col(name: &str, type_name: &str, value: &str) -> ColumnValue {
    ColumnValue {
        name: name.to_string(),
        type_name: type_name.to_string(),
        value: Some(value.to_string()),
    }
}

/// 15-column event. Mostly non-text types, 3 scannable text columns.
fn event(with_match: bool) -> ScanEvent {
    ScanEvent {
        table_id: etl::types::TableId(1),
        schema_name: "public".to_string(),
        table_name: "t1".to_string(),
        action: Action::Insert,
        columns: vec![
            col("c_pk", "int8", "10042"),
            col("c_int1", "int4", "7"),
            col("c_num", "numeric", "149.99"),
            col("c_int2", "int2", "3"),
            col("c_bool", "bool", "true"),
            col("c_ts1", "timestamptz", "2025-01-15T10:30:00Z"),
            col("c_ts2", "timestamptz", "2025-01-15T12:00:00Z"),
            col("c_ts3", "timestamp", "2025-01-16T09:00:00Z"),
            col("c_uuid", "uuid", "550e8400-e29b-41d4-a716-446655440000"),
            col("c_f1", "float4", "2.5"),
            col("c_f2", "float8", "0.08"),
            col("c_text1", "text", "short text value"),
            col("c_text2", "varchar", "another text value here"),
            col(
                "c_text3",
                "text",
                if with_match {
                    "secret: MATCH_THIS_SENTINEL"
                } else {
                    "yet another text column"
                },
            ),
            ColumnValue {
                name: "c_null".into(),
                type_name: "jsonb".into(),
                value: None,
            },
        ],
        primary_keys: vec![("c_pk".to_string(), "10042".to_string())],
        start_lsn: 100,
        commit_lsn: 200,
    }
}

/// 50-column event, only 5 scannable text. Tests type-filter throughput.
fn wide_event() -> ScanEvent {
    let mut columns: Vec<ColumnValue> = Vec::with_capacity(50);
    for i in 0..20 {
        columns.push(col(&format!("c_int_{i}"), "int4", &i.to_string()));
    }
    for i in 0..10 {
        columns.push(col(&format!("c_ts_{i}"), "timestamptz", "2025-01-01T00:00:00Z"));
    }
    for i in 0..5 {
        columns.push(col(&format!("c_bool_{i}"), "bool", "true"));
    }
    for i in 0..5 {
        columns.push(col(&format!("c_uuid_{i}"), "uuid", "550e8400-e29b-41d4-a716-446655440000"));
    }
    for i in 0..5 {
        columns.push(col(&format!("c_float_{i}"), "float8", "3.14"));
    }
    for i in 0..5 {
        columns.push(col(&format!("c_text_{i}"), "text", "clean ordinary value"));
    }
    ScanEvent {
        table_id: etl::types::TableId(1),
        schema_name: "public".to_string(),
        table_name: "t2".to_string(),
        action: Action::Insert,
        columns,
        primary_keys: vec![("c_int_0".to_string(), "0".to_string())],
        start_lsn: 100,
        commit_lsn: 200,
    }
}

fn bench_event_no_match(c: &mut Criterion) {
    let engine = RuleEngine::new(&noop_rule()).unwrap();
    let scanner = Scanner::new(engine, ScanFilter::default());
    let ev = event(false);
    c.bench_function("scan/event_no_match", |b| b.iter(|| scanner.scan(black_box(&ev))));
}

fn bench_event_with_match(c: &mut Criterion) {
    let engine = RuleEngine::new(&noop_rule()).unwrap();
    let scanner = Scanner::new(engine, ScanFilter::default());
    let ev = event(true);
    c.bench_function("scan/event_with_match", |b| b.iter(|| scanner.scan(black_box(&ev))));
}

fn bench_type_filtering(c: &mut Criterion) {
    let engine = RuleEngine::new(&noop_rule()).unwrap();
    let scanner = Scanner::new(engine, ScanFilter::default());
    let ev = wide_event();
    c.bench_function("scan/type_filtering", |b| b.iter(|| scanner.scan(black_box(&ev))));
}

fn bench_table_exclusion(c: &mut Criterion) {
    let engine = RuleEngine::new(&noop_rule()).unwrap();
    let scanner = Scanner::new(
        engine,
        ScanFilter {
            exclude_tables: vec!["t1".to_string()],
            ..Default::default()
        },
    );
    let ev = event(false);
    c.bench_function("scan/table_exclusion", |b| b.iter(|| scanner.scan(black_box(&ev))));
}

fn bench_value_sizes(c: &mut Criterion) {
    let engine = RuleEngine::new(&noop_rule()).unwrap();
    let scanner = Scanner::new(engine, ScanFilter::default());
    let mut group = c.benchmark_group("scan/value_size");

    for size in [64, 512, 4096] {
        let ev = ScanEvent {
            table_id: etl::types::TableId(1),
            schema_name: "public".to_string(),
            table_name: "t3".to_string(),
            action: Action::Insert,
            columns: vec![col("c_text", "text", &"a".repeat(size))],
            primary_keys: vec![("c_pk".to_string(), "1".to_string())],
            start_lsn: 100,
            commit_lsn: 200,
        };
        group.bench_with_input(BenchmarkId::from_parameter(format!("{size}b")), &ev, |b, ev| {
            b.iter(|| scanner.scan(black_box(ev)));
        });
    }
    group.finish();
}

fn bench_throughput(c: &mut Criterion) {
    let engine = RuleEngine::new(&noop_rule()).unwrap();
    let scanner = Scanner::new(engine, ScanFilter::default());
    let clean = event(false);
    let hit = event(true);

    // 95% clean, 5% matches — realistic production mix
    let batch: Vec<ScanEvent> = (0..1000)
        .map(|i| if i % 20 == 0 { hit.clone() } else { clean.clone() })
        .collect();

    c.bench_function("scan/throughput_1000", |b| {
        b.iter(|| {
            let mut total = 0usize;
            for ev in &batch {
                total += scanner.scan(black_box(ev)).len();
            }
            total
        });
    });
}

fn bench_event_extraction(c: &mut Criterion) {
    let meta = TableMeta {
        schema: "public".to_string(),
        name: "t1".to_string(),
        columns: vec![
            ColumnMeta {
                name: "c_pk".into(),
                type_name: "int8".into(),
                primary: true,
            },
            ColumnMeta {
                name: "c_int".into(),
                type_name: "int4".into(),
                primary: false,
            },
            ColumnMeta {
                name: "c_num".into(),
                type_name: "numeric".into(),
                primary: false,
            },
            ColumnMeta {
                name: "c_bool".into(),
                type_name: "bool".into(),
                primary: false,
            },
            ColumnMeta {
                name: "c_text1".into(),
                type_name: "text".into(),
                primary: false,
            },
            ColumnMeta {
                name: "c_text2".into(),
                type_name: "varchar".into(),
                primary: false,
            },
            ColumnMeta {
                name: "c_text3".into(),
                type_name: "text".into(),
                primary: false,
            },
            ColumnMeta {
                name: "c_null".into(),
                type_name: "jsonb".into(),
                primary: false,
            },
        ],
    };

    let mut registry = HashMap::new();
    registry.insert(TableId(1), meta);

    let insert = Event::Insert(InsertEvent {
        start_lsn: 100.into(),
        commit_lsn: 200.into(),
        table_id: TableId(1),
        table_row: TableRow {
            values: vec![
                Cell::I64(10042),
                Cell::I32(7),
                Cell::String("149.99".into()),
                Cell::Bool(true),
                Cell::String("short text value".into()),
                Cell::String("another text value here".into()),
                Cell::String("yet another text column".into()),
                Cell::Null,
            ],
        },
    });

    let batch: Vec<Event> = vec![insert; 100];

    c.bench_function("extract/batch_100", |b| {
        b.iter(|| events::extract_scan_events(black_box(&batch), black_box(&registry)));
    });
}

criterion_group!(
    benches,
    bench_event_no_match,
    bench_event_with_match,
    bench_type_filtering,
    bench_table_exclusion,
    bench_value_sizes,
    bench_throughput,
    bench_event_extraction,
);
criterion_main!(benches);
