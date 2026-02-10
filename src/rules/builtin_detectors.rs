use super::{config::BuiltinKind, validators};

/// Algorithmic detectors that don't rely on regex.
#[derive(Debug, Clone, Copy)]
pub enum BuiltinDetector {
    CreditCard,
    Ssn,
}

impl BuiltinDetector {
    pub fn from_kind(kind: BuiltinKind) -> Self {
        match kind {
            BuiltinKind::CreditCard => BuiltinDetector::CreditCard,
            BuiltinKind::Ssn => BuiltinDetector::Ssn,
        }
    }

    pub fn scan(&self, value: &str) -> Vec<String> {
        match self {
            BuiltinDetector::CreditCard => scan_credit_cards(value),
            BuiltinDetector::Ssn => scan_ssns(value),
        }
    }
}

/// Walk the string finding 13-19 digit sequences (optionally separated by
/// spaces/dashes), then validate each with Luhn.
fn scan_credit_cards(value: &str) -> Vec<String> {
    let mut results = Vec::new();
    let chars: Vec<char> = value.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if !chars[i].is_ascii_digit() {
            i += 1;
            continue;
        }

        if i > 0 && (chars[i - 1].is_ascii_alphanumeric() || chars[i - 1] == '_') {
            i += 1;
            continue;
        }

        let start = i;
        let mut digit_count = 0;
        let mut j = i;

        while j < len && digit_count < 19 {
            if chars[j].is_ascii_digit() {
                digit_count += 1;
                j += 1;
            } else if (chars[j] == ' ' || chars[j] == '-') && digit_count > 0 && j + 1 < len && chars[j + 1].is_ascii_digit() {
                j += 1;
            } else {
                break;
            }
        }

        let at_boundary = j >= len || !(chars[j].is_ascii_alphanumeric() || chars[j] == '_');

        if (13..=19).contains(&digit_count) && at_boundary {
            let candidate: String = chars[start..j].iter().collect();
            if validators::luhn(&candidate) {
                results.push(candidate);
            }
        }

        i = if j > i { j } else { i + 1 };
    }

    results
}

/// Scan for SSN patterns (DDD-DD-DDDD) with boundary checks.
fn scan_ssns(value: &str) -> Vec<String> {
    let mut results = Vec::new();
    let chars: Vec<char> = value.chars().collect();
    let len = chars.len();

    // SSN is exactly 11 chars: DDD-DD-DDDD
    if len < 11 {
        return results;
    }

    for i in 0..=len - 11 {
        if i > 0 && (chars[i - 1].is_ascii_alphanumeric() || chars[i - 1] == '_') {
            continue;
        }

        let end = i + 11;
        if end < len && (chars[end].is_ascii_alphanumeric() || chars[end] == '_') {
            continue;
        }

        // Check pattern: DDD<sep>DD<sep>DDDD where sep is - or . or space
        let sep = chars[i + 3];
        if chars[i].is_ascii_digit()
            && chars[i + 1].is_ascii_digit()
            && chars[i + 2].is_ascii_digit()
            && (sep == '-' || sep == ' ' || sep == '.')
            && chars[i + 4].is_ascii_digit()
            && chars[i + 5].is_ascii_digit()
            && chars[i + 6] == sep
            && chars[i + 7].is_ascii_digit()
            && chars[i + 8].is_ascii_digit()
            && chars[i + 9].is_ascii_digit()
            && chars[i + 10].is_ascii_digit()
        {
            let candidate: String = chars[i..end].iter().collect();
            if validators::ssn(&candidate) {
                results.push(candidate);
            }
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case("pay 41111 with 4111111111111111 please", &["4111111111111111"])]
    #[case("card: 4111-1111-1111-1111", &["4111-1111-1111-1111"])]
    #[case("card: 4111 1111 1111 1111", &["4111 1111 1111 1111"])]
    #[case("card: 4111111111111112", &[])]
    #[case("abc4111111111111111xyz", &[])]
    #[case("Hello world, this is normal text", &[])]
    #[case("cards: 4111111111111111 and 5500000000000004", &["4111111111111111", "5500000000000004"])]
    fn credit_card_scan(#[case] input: &str, #[case] expected: &[&str]) {
        assert_eq!(scan_credit_cards(input), expected);
    }

    #[rstest]
    #[case("ssn is 123-45-6789", &["123-45-6789"])]
    #[case("ssn is 123 45 6789", &["123 45 6789"])]
    #[case("ssn is 123.45.6789", &["123.45.6789"])]
    #[case("123-45.6789", &[])]
    #[case("000-12-3456", &[])]
    #[case("666-12-3456", &[])]
    #[case("900-12-3456", &[])]
    #[case("abc123-45-6789xyz", &[])]
    #[case("Hello world", &[])]
    fn ssn_scan(#[case] input: &str, #[case] expected: &[&str]) {
        assert_eq!(scan_ssns(input), expected);
    }

    #[rstest]
    #[case(BuiltinDetector::CreditCard, "4111111111111111", 1)]
    #[case(BuiltinDetector::Ssn, "123-45-6789", 1)]
    fn detector_dispatch(#[case] detector: BuiltinDetector, #[case] input: &str, #[case] count: usize) {
        assert_eq!(detector.scan(input).len(), count);
    }
}
