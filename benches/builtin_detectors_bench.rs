use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use pgsense_rs::rules::detectors::Detector;

// Adding a new builtin detector = one function calling these two helpers
fn bench_detector_cases(c: &mut Criterion, prefix: &str, detector: Detector, cases: &[(&str, &str)]) {
    let mut group = c.benchmark_group(prefix);
    for &(label, input) in cases {
        group.bench_function(label, |b| {
            b.iter(|| detector.scan(black_box(input)));
        });
    }
    group.finish();
}

fn bench_detector_scaling(c: &mut Criterion, prefix: &str, detector: Detector, match_str: &str, sizes: &[usize]) {
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

fn bench_credit_card(c: &mut Criterion) {
    let dense_digits = "1234567890".repeat(20);

    bench_detector_cases(
        c,
        "cc_scan",
        Detector::CreditCard,
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

    bench_detector_scaling(c, "cc_scan", Detector::CreditCard, "4111111111111111", &[64, 256, 1024, 4096]);
}

fn bench_ssn(c: &mut Criterion) {
    let near_misses = (0..20)
        .map(|i| format!("000-{:02}-{:04}", i + 1, i + 1))
        .collect::<Vec<_>>()
        .join(" ");

    bench_detector_cases(
        c,
        "ssn_scan",
        Detector::Ssn,
        &[
            ("short_match", "ssn is 123-45-6789"),
            ("short_no_match", "just some ordinary text here"),
            ("multiple_matches", "records: 123-45-6789, 234-56-7890, 345-67-8901 end"),
            ("near_misses_20", &near_misses),
        ],
    );

    bench_detector_scaling(c, "ssn_scan", Detector::Ssn, "123-45-6789", &[64, 256, 1024, 4096]);
}

fn bench_phone(c: &mut Criterion) {
    bench_detector_cases(
        c,
        "phone_scan",
        Detector::Phone,
        &[
            ("e164_match", "call +44 20 7946 0958 for info"),
            ("nanp_match", "phone: (555) 234-5678"),
            ("short_no_match", "just some ordinary text here"),
            (
                "multiple_matches",
                "phones: +1 555 123 4567, (212) 555-9876, 0044 20 7946 0958 end",
            ),
            ("dense_digits", "1234567890123456789012345678901234567890"),
        ],
    );

    bench_detector_scaling(c, "phone_scan", Detector::Phone, "+44 20 7946 0958", &[64, 256, 1024, 4096]);
}

fn bench_email(c: &mut Criterion) {
    bench_detector_cases(
        c,
        "email_scan",
        Detector::Email,
        &[
            ("short_match", "contact user@example.com for info"),
            ("short_no_match", "just some ordinary text here"),
            ("multiple_matches", "from alice@corp.io to bob@example.com and carol@test.org"),
            ("near_misses", "user@ @domain.com user@.com @@ user@domain"),
        ],
    );

    bench_detector_scaling(c, "email_scan", Detector::Email, "alice@example.com", &[64, 256, 1024, 4096]);
}

fn bench_iban(c: &mut Criterion) {
    // Near-misses: valid country codes but wrong length/check digits
    let near_misses = ["DE0037040044053201300", "GB00NWBK60161331926819", "FR0030006000011234567890189"].join(" ");

    bench_detector_cases(
        c,
        "iban_scan",
        Detector::Iban,
        &[
            ("short_match", "pay to DE89370400440532013000 please"),
            ("short_no_match", "just some ordinary text here"),
            (
                "multiple_matches",
                "send DE89370400440532013000 and GB29NWBK60161331926819 to NL91ABNA0417164300",
            ),
            ("spaced_match", "DE89 3704 0044 0532 0130 00"),
            ("near_misses_3", &near_misses),
        ],
    );

    bench_detector_scaling(c, "iban_scan", Detector::Iban, "DE89370400440532013000", &[64, 256, 1024, 4096]);
}

criterion_group!(benches, bench_credit_card, bench_ssn, bench_phone, bench_email, bench_iban);
criterion_main!(benches);
