use pgsense_rs::rules::{detectors::Detector, validators};

fn main() {
    afl::fuzz!(|data: &[u8]| {
        let Ok(input) = std::str::from_utf8(data) else {
            return;
        };

        let detector = Detector::CreditCard;
        let matches = detector.scan(input);

        for m in &matches {
            assert!(validators::luhn(m), "match {m:?} failed Luhn check");

            assert!(
                m.chars().all(|c| c.is_ascii_digit() || c == '-' || c == ' '),
                "match {m:?} contains unexpected characters",
            );

            let digit_count = m.chars().filter(|c| c.is_ascii_digit()).count();
            assert!((13..=19).contains(&digit_count), "match {m:?} has {digit_count} digits, expected 13-19");
        }
    });
}
