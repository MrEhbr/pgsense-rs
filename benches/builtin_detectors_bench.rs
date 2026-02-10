use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use pgsense_rs::rules::builtin_detectors::BuiltinDetector;

// ---------------------------------------------------------------------------
// Helpers — adding a new builtin detector = one function calling these
// ---------------------------------------------------------------------------

fn bench_detector_cases(c: &mut Criterion, prefix: &str, detector: BuiltinDetector, cases: &[(&str, &str)]) {
    let mut group = c.benchmark_group(prefix);
    for &(label, input) in cases {
        group.bench_function(label, |b| {
            b.iter(|| detector.scan(black_box(input)));
        });
    }
    group.finish();
}

fn bench_detector_scaling(c: &mut Criterion, prefix: &str, detector: BuiltinDetector, match_str: &str, sizes: &[usize]) {
    let mut group = c.benchmark_group(format!("{prefix}/value_size"));
    for &size in sizes {
        let padding = "a".repeat(size.saturating_sub(match_str.len() + 5));
        let input = format!("{padding} {match_str} end");
        group.bench_with_input(BenchmarkId::from_parameter(format!("{size}b")), &input, |b, input| {
            b.iter(|| detector.scan(black_box(input)));
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Credit card detector
// ---------------------------------------------------------------------------

fn bench_credit_card(c: &mut Criterion) {
    let dense_digits = "1234567890".repeat(20);

    bench_detector_cases(
        c,
        "cc_scan",
        BuiltinDetector::CreditCard,
        &[
            ("short_match", "pay with 4111111111111111 please"),
            ("short_no_match", "just some ordinary text here"),
            (
                "multiple_matches",
                "cards: 4111111111111111, 5500000000000004, 378282246310005 done",
            ),
            ("dense_digits_200", &dense_digits),
        ],
    );

    bench_detector_scaling(
        c,
        "cc_scan",
        BuiltinDetector::CreditCard,
        "4111111111111111",
        &[64, 256, 1024, 4096],
    );
}

// ---------------------------------------------------------------------------
// SSN detector
// ---------------------------------------------------------------------------

fn bench_ssn(c: &mut Criterion) {
    let near_misses = (0..20)
        .map(|i| format!("000-{:02}-{:04}", i + 1, i + 1))
        .collect::<Vec<_>>()
        .join(" ");

    bench_detector_cases(
        c,
        "ssn_scan",
        BuiltinDetector::Ssn,
        &[
            ("short_match", "ssn is 123-45-6789"),
            ("short_no_match", "just some ordinary text here"),
            ("multiple_matches", "records: 123-45-6789, 234-56-7890, 345-67-8901 end"),
            ("near_misses_20", &near_misses),
        ],
    );

    bench_detector_scaling(c, "ssn_scan", BuiltinDetector::Ssn, "123-45-6789", &[64, 256, 1024, 4096]);
}

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

criterion_group!(benches, bench_credit_card, bench_ssn);
criterion_main!(benches);
