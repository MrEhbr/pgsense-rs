use pgsense_rs::rules::detectors::Detector;

fn main() {
    afl::fuzz!(|data: &[u8]| {
        let Ok(input) = std::str::from_utf8(data) else {
            return;
        };

        let detector = Detector::Phone;
        let matches = detector.scan(input);

        for m in &matches {
            let digit_count = m.chars().filter(|c| c.is_ascii_digit()).count();
            // "00" dial prefix adds 2 digits to the matched string but is not part of the E.164 number
            let max_digits: usize = if m.starts_with("00") { 17 } else { 15 };
            assert!(digit_count >= 7, "match {m:?} has only {digit_count} digits, expected >= 7");
            assert!(digit_count <= max_digits, "match {m:?} has {digit_count} digits, expected <= {max_digits}");

            assert!(
                m.chars().all(|c| c.is_ascii_digit() || matches!(c, '+' | '-' | ' ' | '.' | '(' | ')')),
                "match {m:?} contains unexpected characters",
            );

            // E.164 matches must start with '+'
            // 00-prefix matches must start with "00"
            // NANP matches must start with '(' or digit 2-9
            let first = m.chars().next().unwrap();
            assert!(
                first == '+' || first == '0' || first == '(' || ('2'..='9').contains(&first),
                "match {m:?} starts with unexpected char {first:?}",
            );
        }
    });
}
