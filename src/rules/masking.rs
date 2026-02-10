/// Mask a value: show first 2 and last 2 characters.
pub fn mask(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() < 8 {
        return "*".repeat(chars.len());
    }
    let first: String = chars[..2].iter().collect();
    let last: String = chars[chars.len() - 2..].iter().collect();
    let middle = "*".repeat(chars.len() - 4);
    format!("{first}{middle}{last}")
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case("", "")]
    #[case("a", "*")]
    #[case("ab", "**")]
    #[case("abcde", "*****")]
    #[case("abcdefg", "*******")]
    #[case("abcdefgh", "ab****gh")]
    #[case("1234567890", "12******90")]
    #[case("secretvalue", "se*******ue")]
    #[case("4111111111111111", "41************11")]
    fn mask_values(#[case] input: &str, #[case] expected: &str) {
        assert_eq!(mask(input), expected);
    }
}
