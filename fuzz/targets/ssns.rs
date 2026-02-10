use pgsense_rs::rules::{builtin_detectors::BuiltinDetector, validators};

fn main() {
    afl::fuzz!(|data: &[u8]| {
        let Ok(input) = std::str::from_utf8(data) else {
            return;
        };

        let detector = BuiltinDetector::Ssn;
        let matches = detector.scan(input);

        for m in &matches {
            // Every match must pass SSN validation
            assert!(validators::ssn(m), "match {m:?} failed SSN validation");

            // Every match must be exactly 11 characters: DDD<sep>DD<sep>DDDD
            assert_eq!(m.len(), 11, "match {m:?} length is {}, expected 11", m.len());

            // Separators must be consistent
            let chars: Vec<char> = m.chars().collect();
            let sep = chars[3];
            assert!(sep == '-' || sep == ' ' || sep == '.', "match {m:?} has unexpected separator {sep:?}");
            assert_eq!(chars[6], sep, "match {m:?} has inconsistent separators");
        }
    });
}
