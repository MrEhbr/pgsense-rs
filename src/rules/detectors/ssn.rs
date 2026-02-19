use crate::rules::validators;

/// Scan for SSN patterns (DDD-DD-DDDD) with boundary checks.
pub(super) fn scan(value: &str) -> Vec<String> {
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
        assert_eq!(scan(input), expected);
    }
}
