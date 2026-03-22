use pgsense_rs::rules::{detectors::Detector, validators};

fn main() {
    afl::fuzz!(|data: &[u8]| {
        let Ok(input) = std::str::from_utf8(data) else {
            return;
        };

        let detector = Detector::Iban;
        let matches = detector.scan(input);

        for m in &matches {
            assert!(validators::iban(m), "match {m:?} failed mod-97 validation");

            assert!(
                m.bytes().all(|b| b.is_ascii_alphanumeric() || b == b' ' || b == b'-'),
                "match {m:?} contains unexpected characters",
            );

            // Check structural invariants on the compact (separator-stripped) form
            let compact: Vec<u8> = m.bytes().filter(|b| b.is_ascii_alphanumeric()).collect();
            assert!((15..=34).contains(&compact.len()), "match {m:?} has {} alnum chars, expected 15-34", compact.len());
            assert!(compact[0].is_ascii_uppercase() && compact[1].is_ascii_uppercase(), "match {m:?} does not start with two uppercase letters");
            assert!(compact[2].is_ascii_digit() && compact[3].is_ascii_digit(), "match {m:?} check digits are not digits");

            assert!(input.contains(m.as_str()), "match {m:?} is not a substring of input");
        }
    });
}
