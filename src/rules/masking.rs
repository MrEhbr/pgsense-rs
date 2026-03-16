use std::fmt::Write;

pub fn mask(s: &str) -> String {
    let char_len = s.chars().count();
    if char_len < 8 {
        return "*".repeat(char_len);
    }

    let mut chars = s.chars();
    let c0 = chars.next().unwrap();
    let c1 = chars.next().unwrap();

    let last_start = s.char_indices().nth(char_len - 2).unwrap().0;

    let mut result = String::with_capacity(s.len());
    result.push(c0);
    result.push(c1);
    for _ in 0..(char_len - 4) {
        result.push('*');
    }
    let _ = write!(result, "{}", &s[last_start..]);
    result
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
