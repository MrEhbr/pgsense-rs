use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use pgsense_rs::rules::validators;

fn bench_cases(c: &mut Criterion, group: &str, cases: &[(&str, &str)], f: fn(&str) -> bool) {
    let mut group = c.benchmark_group(group);
    for &(label, input) in cases {
        group.bench_function(label, |b| {
            b.iter(|| f(black_box(input)));
        });
    }
    group.finish();
}

fn bench_luhn(c: &mut Criterion) {
    bench_cases(
        c,
        "validator/luhn",
        &[
            ("valid_16", "4111111111111111"),
            ("valid_15", "378282246310005"),
            ("valid_dashed", "4111-1111-1111-1111"),
            ("invalid", "4111111111111112"),
        ],
        validators::luhn,
    );
}

fn bench_ssn(c: &mut Criterion) {
    bench_cases(
        c,
        "validator/ssn",
        &[("valid_dash", "123-45-6789"), ("valid_space", "123 45 6789"), ("invalid_area", "000-12-3456")],
        validators::ssn,
    );
}

criterion_group!(benches, bench_luhn, bench_ssn);
criterion_main!(benches);
