use pgsense_rs::rules::{builtin_detectors::BuiltinDetector, validators};

fn main() {
    afl::fuzz!(|data: &[u8]| {
        let Ok(input) = std::str::from_utf8(data) else {
            return;
        };

        let detector = BuiltinDetector::CreditCard;
        let matches = detector.scan(input);

        for m in &matches {
            // Every match must pass Luhn validation
            assert!(validators::luhn(m), "match {m:?} failed Luhn check");

            // Every match must contain only digits and valid separators
            assert!(
                m.chars().all(|c| c.is_ascii_digit() || c == '-' || c == ' '),
                "match {m:?} contains unexpected characters",
            );

            // Digit count must be 13-19
            let digit_count = m.chars().filter(|c| c.is_ascii_digit()).count();
            assert!((13..=19).contains(&digit_count), "match {m:?} has {digit_count} digits, expected 13-19");
        }
    });
}
