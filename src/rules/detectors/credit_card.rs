use crate::rules::validators;

/// Walk the string finding 13-19 digit sequences (optionally separated by
/// spaces/dashes), then validate each with Luhn. Works on bytes directly
/// since credit card patterns are ASCII-only.
pub(super) fn scan(value: &str) -> Vec<String> {
    let bytes = value.as_bytes();
    let len = bytes.len();
    let mut results = Vec::new();
    let mut i = 0;

    while i < len {
        if !bytes[i].is_ascii_digit() {
            i += 1;
            continue;
        }

        if i > 0 && (bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_') {
            i += 1;
            continue;
        }

        let start = i;
        let mut digit_count = 0;
        let mut j = i;

        while j < len && digit_count < 19 {
            if bytes[j].is_ascii_digit() {
                digit_count += 1;
                j += 1;
            } else if (bytes[j] == b' ' || bytes[j] == b'-') && digit_count > 0 && j + 1 < len && bytes[j + 1].is_ascii_digit() {
                j += 1;
            } else {
                break;
            }
        }

        let at_boundary = j >= len || !(bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_');

        if (13..=19).contains(&digit_count) && at_boundary {
            let candidate = &value[start..j];
            if validators::luhn(candidate) {
                results.push(candidate.to_string());
            }
        }

        i = if j > i { j } else { i + 1 };
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
    // UTF-8: multi-byte chars around the number
    #[case("€4111111111111111", &["4111111111111111"])]
    #[case("カード4111111111111111番号", &["4111111111111111"])]
    #[case("💳 4111111111111111 ✓", &["4111111111111111"])]
    // UTF-8: multi-byte chars inside the number must not match
    #[case("4111€11111111111", &[])]
    #[case("4111💳1111111111111", &[])]
    #[case("4111\u{200B}111111111111", &[])]
    #[case("4111·1111·1111·1111", &[])]
    fn credit_card_scan(#[case] input: &str, #[case] expected: &[&str]) {
        assert_eq!(scan(input), expected);
    }
}
