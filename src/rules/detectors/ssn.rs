use crate::rules::validators;

/// Scan for SSN patterns (DDD-DD-DDDD) with boundary checks.
/// Works on bytes directly since SSN patterns are ASCII-only.
pub(super) fn scan(value: &str) -> Vec<String> {
    let bytes = value.as_bytes();
    let len = bytes.len();

    if len < 11 {
        return Vec::new();
    }

    let mut results = Vec::new();

    for i in 0..=len - 11 {
        if i > 0 && (bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_') {
            continue;
        }

        let end = i + 11;
        if end < len && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
            continue;
        }

        let sep = bytes[i + 3];
        if bytes[i].is_ascii_digit()
            && bytes[i + 1].is_ascii_digit()
            && bytes[i + 2].is_ascii_digit()
            && matches!(sep, b'-' | b' ' | b'.')
            && bytes[i + 4].is_ascii_digit()
            && bytes[i + 5].is_ascii_digit()
            && bytes[i + 6] == sep
            && bytes[i + 7].is_ascii_digit()
            && bytes[i + 8].is_ascii_digit()
            && bytes[i + 9].is_ascii_digit()
            && bytes[i + 10].is_ascii_digit()
        {
            let candidate = &value[i..end];
            if validators::ssn(candidate) {
                results.push(candidate.to_string());
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
    #[case("ssn is 123-45-6789", &["123-45-6789"])]
    #[case("ssn is 123 45 6789", &["123 45 6789"])]
    #[case("ssn is 123.45.6789", &["123.45.6789"])]
    #[case("123-45.6789", &[])]
    #[case("000-12-3456", &[])]
    #[case("666-12-3456", &[])]
    #[case("900-12-3456", &[])]
    #[case("abc123-45-6789xyz", &[])]
    #[case("Hello world", &[])]
    // UTF-8: multi-byte chars around the number
    #[case("€123-45-6789", &["123-45-6789"])]
    #[case("番号123-45-6789です", &["123-45-6789"])]
    #[case("🔒 123-45-6789 🔒", &["123-45-6789"])]
    // UTF-8: multi-byte chars inside the number must not match
    #[case("123€45-6789", &[])]
    #[case("123-45€6789", &[])]
    #[case("123\u{200B}45-6789", &[])]
    fn ssn_scan(#[case] input: &str, #[case] expected: &[&str]) {
        assert_eq!(scan(input), expected);
    }
}
