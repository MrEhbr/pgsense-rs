use pgsense_rs::rules::{detectors::Detector, validators};

fn main() {
    afl::fuzz!(|data: &[u8]| {
        let Ok(input) = std::str::from_utf8(data) else {
            return;
        };

        let detector = Detector::Ssn;
        let matches = detector.scan(input);

        for m in &matches {
            assert!(validators::ssn(m), "match {m:?} failed SSN validation");

            assert_eq!(m.len(), 11, "match {m:?} length is {}, expected 11", m.len());

            let chars: Vec<char> = m.chars().collect();
            let sep = chars[3];
            assert!(sep == '-' || sep == ' ' || sep == '.', "match {m:?} has unexpected separator {sep:?}");
            assert_eq!(chars[6], sep, "match {m:?} has inconsistent separators");
        }
    });
}
