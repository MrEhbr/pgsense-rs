use crate::rules::validators;

/// Walk the string finding 13-19 digit sequences (optionally separated by
/// spaces/dashes), then validate each with Luhn.
pub(super) fn scan(value: &str) -> Vec<String> {
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
        assert_eq!(scan(input), expected);
    }
}
