use pgsense_rs::rules::{detectors::Detector, validators};

fn main() {
    afl::fuzz!(|data: &[u8]| {
        let Ok(input) = std::str::from_utf8(data) else {
            return;
        };

        let detector = Detector::Email;
        let matches = detector.scan(input);

        for m in &matches {
            assert!(validators::email(m), "match {m:?} failed email validation");

            assert!(m.contains('@'), "match {m:?} has no '@'");

            let at_count = m.bytes().filter(|&b| b == b'@').count();
            assert_eq!(at_count, 1, "match {m:?} has {at_count} '@' signs, expected 1");

            let (local, domain) = m.split_once('@').unwrap();

            assert!(!local.is_empty() && local.len() <= 64, "match {m:?} local part length {} out of range", local.len());

            assert!(
                local.bytes().all(|b| b.is_ascii_alphanumeric() || b".!#$%&'*+/=?^_`{|}~-".contains(&b)),
                "match {m:?} local part contains invalid chars",
            );
            assert!(!local.starts_with('.') && !local.ends_with('.'), "match {m:?} local part has leading/trailing dot");
            assert!(!local.contains(".."), "match {m:?} local part has consecutive dots");

            assert!(!domain.is_empty() && domain.len() <= 253, "match {m:?} domain length {} out of range", domain.len());

            let labels: Vec<&str> = domain.split('.').collect();
            assert!(labels.len() >= 2, "match {m:?} domain has {} labels, expected >= 2", labels.len());

            for label in &labels {
                assert!(!label.is_empty() && label.len() <= 63, "match {m:?} label {label:?} length out of range");
                assert!(!label.starts_with('-') && !label.ends_with('-'), "match {m:?} label {label:?} has leading/trailing hyphen");
            }

            let tld = labels.last().unwrap();
            assert!(tld.len() >= 2, "match {m:?} TLD {tld:?} too short");
            assert!(tld.bytes().all(|b| b.is_ascii_alphabetic()), "match {m:?} TLD {tld:?} contains non-alpha");

            assert!(
                input.contains(m.as_str()),
                "match {m:?} is not a substring of input",
            );
        }
    });
}
