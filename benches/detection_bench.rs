use std::{hint::black_box, sync::Arc};

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use etl::types::{Cell, Event, InsertEvent, ReplicatedTableSchema, TableId, TableName, TableRow, Type};
use etl_postgres::types::{ColumnSchema, TableSchema};
use pgsense_rs::{
    events::{self, Action, ColumnValue, ScanEvent},
    rules::{
        config::{RuleConfig, RuleType, Severity},
        engine::RuleEngine,
    },
    scanner::Scanner,
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
        channels: None,
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
        database: "localhost/bench".to_string(),
        table_id: TableId::new(1),
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
        database: "localhost/bench".to_string(),
        table_id: TableId::new(1),
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
    let engine = RuleEngine::new(&noop_rule(), false).unwrap();
    let scanner = Scanner::new(engine);
    let ev = event(false);
    c.bench_function("scan/event_no_match", |b| b.iter(|| scanner.scan(black_box(&ev))));
}

fn bench_event_with_match(c: &mut Criterion) {
    let engine = RuleEngine::new(&noop_rule(), false).unwrap();
    let scanner = Scanner::new(engine);
    let ev = event(true);
    c.bench_function("scan/event_with_match", |b| b.iter(|| scanner.scan(black_box(&ev))));
}

fn bench_type_filtering(c: &mut Criterion) {
    let engine = RuleEngine::new(&noop_rule(), false).unwrap();
    let scanner = Scanner::new(engine);
    let ev = wide_event();
    c.bench_function("scan/type_filtering", |b| b.iter(|| scanner.scan(black_box(&ev))));
}

fn bench_value_sizes(c: &mut Criterion) {
    let engine = RuleEngine::new(&noop_rule(), false).unwrap();
    let scanner = Scanner::new(engine);
    let mut group = c.benchmark_group("scan/value_size");

    for size in [64, 512, 4096] {
        let ev = ScanEvent {
            database: "localhost/bench".to_string(),
            table_id: TableId::new(1),
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
    let engine = RuleEngine::new(&noop_rule(), false).unwrap();
    let scanner = Scanner::new(engine);
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

fn bench_profiling_overhead(c: &mut Criterion) {
    let off = Scanner::new(RuleEngine::new(&noop_rule(), false).unwrap());
    let on = Scanner::new(RuleEngine::new(&noop_rule(), true).unwrap());
    let ev = event(true);

    let mut group = c.benchmark_group("scan/profiling_overhead");
    group.bench_function("off", |b| b.iter(|| off.scan(black_box(&ev))));
    group.bench_function("on", |b| b.iter(|| on.scan(black_box(&ev))));
    group.finish();
}

fn make_event(replicated: ReplicatedTableSchema) -> Event {
    Event::Insert(InsertEvent {
        start_lsn: 100.into(),
        commit_lsn: 200.into(),
        tx_ordinal: 0,
        replicated_table_schema: replicated,
        table_row: TableRow::new(vec![
            Cell::I64(10042),
            Cell::I32(7),
            Cell::String("149.99".into()),
            Cell::Bool(true),
            Cell::String("short text value".into()),
            Cell::String("another text value here".into()),
            Cell::String("yet another text column".into()),
            Cell::Null,
        ]),
    })
}

fn bench_event_extraction(c: &mut Criterion) {
    let columns = vec![
        ColumnSchema::new("c_pk".into(), Type::INT8, -1, 1, Some(1), false),
        ColumnSchema::new("c_int".into(), Type::INT4, -1, 2, None, true),
        ColumnSchema::new("c_num".into(), Type::NUMERIC, -1, 3, None, true),
        ColumnSchema::new("c_bool".into(), Type::BOOL, -1, 4, None, true),
        ColumnSchema::new("c_text1".into(), Type::TEXT, -1, 5, None, true),
        ColumnSchema::new("c_text2".into(), Type::VARCHAR, -1, 6, None, true),
        ColumnSchema::new("c_text3".into(), Type::TEXT, -1, 7, None, true),
        ColumnSchema::new("c_null".into(), Type::JSONB, -1, 8, None, true),
    ];
    let table_schema = Arc::new(TableSchema::new(
        TableId::new(1),
        TableName::new("public".into(), "t1".into()),
        columns,
    ));
    let replicated = ReplicatedTableSchema::all(table_schema);

    // Event isn't Clone (only with test-utils feature), so build the batch by
    // calling make_event repeatedly.
    let batch: Vec<Event> = (0..100).map(|_| make_event(replicated.clone())).collect();

    c.bench_function("extract/batch_100", |b| {
        b.iter(|| events::extract_scan_events(black_box(&batch), "localhost/bench"));
    });
}

criterion_group!(
    benches,
    bench_event_no_match,
    bench_event_with_match,
    bench_type_filtering,
    bench_value_sizes,
    bench_throughput,
    bench_profiling_overhead,
    bench_event_extraction,
);
criterion_main!(benches);
