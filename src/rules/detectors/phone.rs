use phonenumber::metadata::DATABASE;

use crate::rules::validators;

pub(super) fn scan(value: &str) -> Vec<String> {
    let bytes = value.as_bytes();
    let mut results = Vec::new();
    let mut i = 0;

    while i < bytes.len() {
        if i > 0 && bytes[i - 1].is_ascii_alphanumeric() {
            i += 1;
            continue;
        }
        let parsed = match bytes[i] {
            b'+' if i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit() => parse_international(bytes, value, i, i + 1),
            b'0' if i + 2 < bytes.len() && bytes[i + 1] == b'0' && bytes[i + 2].is_ascii_digit() => parse_international(bytes, value, i, i + 2),
            b'(' | b'2'..=b'9' => parse_nanp(bytes, value, i),
            _ => None,
        };
        match parsed {
            Some((end, candidate)) => {
                results.push(candidate);
                i = end;
            },
            None => i += 1,
        }
    }

    results
}

/// Collect digits + separators, then validate against known country codes.
/// Handles both E.164 (`+` prefix) and `00` dial prefix.
fn parse_international(bytes: &[u8], value: &str, match_start: usize, digit_start: usize) -> Option<(usize, String)> {
    let mut digits = [0u8; 15];
    let mut count = 0;
    let mut j = digit_start;
    let mut end = digit_start;

    while j < bytes.len() && count < 15 {
        match bytes[j] {
            b @ b'0'..=b'9' => {
                digits[count] = b - b'0';
                count += 1;
                j += 1;
                end = j;
            },
            b' ' | b'-' | b'.' | b'(' | b')' if count > 0 => j += 1,
            _ => break,
        }
    }

    if !(7..=15).contains(&count) || bytes.get(end).is_some_and(|b| b.is_ascii_alphanumeric()) {
        return None;
    }

    let has_valid_cc = (1usize..=3).any(|len| {
        if count < len {
            return false;
        }
        let cc: u16 = digits[..len]
            .iter()
            .fold(0u16, |acc, &d| acc * 10 + d as u16);
        lookup_country_code(cc).is_some_and(|(min, max)| (min as usize..=max as usize).contains(&(count - len)))
    });

    if !has_valid_cc {
        return None;
    }

    let candidate = &value[match_start..end];
    validators::phone(candidate).then(|| (end, candidate.to_string()))
}

/// Match NANP-formatted numbers: (NNN) NNN-NNNN, NNN-NNN-NNNN, NNN.NNN.NNNN.
/// Requires at least one separator, area code and exchange starting with 2-9.
fn parse_nanp(bytes: &[u8], value: &str, start: usize) -> Option<(usize, String)> {
    let mut j = start;
    let mut digits = [0u8; 10];
    let mut count = 0;
    let mut has_sep = false;

    if bytes.get(j) == Some(&b'(') {
        j += 1;
    }

    let mut end = j;
    while j < bytes.len() && count < 10 {
        match bytes[j] {
            b @ b'0'..=b'9' => {
                digits[count] = b;
                count += 1;
                j += 1;
                end = j;
            },
            b')' | b'-' | b'.' | b' ' if count > 0 => {
                has_sep = true;
                j += 1;
            },
            _ => break,
        }
    }

    if count != 10 || !has_sep {
        return None;
    }
    if !matches!(digits[0], b'2'..=b'9') || !matches!(digits[3], b'2'..=b'9') {
        return None;
    }
    if bytes.get(end).is_some_and(|b| b.is_ascii_alphanumeric()) {
        return None;
    }

    let candidate = &value[start..end];
    validators::phone(candidate).then(|| (end, candidate.to_string()))
}

fn lookup_country_code(cc: u16) -> Option<(u8, u8)> {
    let regions = DATABASE.by_code(&cc)?;
    let meta = regions.first()?;
    let d = meta.descriptors();

    let all_lengths = [
        d.fixed_line(),
        d.mobile(),
        d.toll_free(),
        d.premium_rate(),
        d.shared_cost(),
        d.voip(),
        d.personal_number(),
        d.pager(),
        d.uan(),
    ]
    .into_iter()
    .flatten()
    .flat_map(|desc| desc.possible_length().iter().copied());

    let (min, max) = all_lengths.fold((u16::MAX, 0u16), |(lo, hi), len| (lo.min(len), hi.max(len)));
    if min > max {
        return None;
    }
    Some((min as u8, max as u8))
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    // E.164 format
    #[rstest]
    #[case("+44 20 7946 0958", &["+44 20 7946 0958"])]
    #[case("+1 212 234 5678", &["+1 212 234 5678"])]
    #[case("+33 1 23 45 67 89", &["+33 1 23 45 67 89"])]
    #[case("+49 30 12345678", &["+49 30 12345678"])]
    #[case("+81-3-1234-5678", &["+81-3-1234-5678"])]
    #[case("+91.98765.43210", &["+91.98765.43210"])]
    #[case("+61 2 1234 5678", &["+61 2 1234 5678"])]
    #[case("+86 138 0013 8000", &["+86 138 0013 8000"])]
    fn e164_detected(#[case] input: &str, #[case] expected: &[&str]) {
        assert_eq!(scan(input), expected);
    }

    // 00-prefix international
    #[rstest]
    #[case("0033 1 23 45 67 89", &["0033 1 23 45 67 89"])]
    #[case("0044 20 7946 0958", &["0044 20 7946 0958"])]
    #[case("0049-30-12345678", &["0049-30-12345678"])]
    fn double_zero_prefix_detected(#[case] input: &str, #[case] expected: &[&str]) {
        assert_eq!(scan(input), expected);
    }

    // NANP formatted (area code and exchange must start 2-9)
    #[rstest]
    #[case("(212) 234-5678", &["(212) 234-5678"])]
    #[case("312-456-7890", &["312-456-7890"])]
    #[case("415.234.5678", &["415.234.5678"])]
    #[case("202 345 6789", &["202 345 6789"])]
    fn nanp_detected(#[case] input: &str, #[case] expected: &[&str]) {
        assert_eq!(scan(input), expected);
    }

    // Embedded in text
    #[rstest]
    #[case("call +44 20 7946 0958 for info", &["+44 20 7946 0958"])]
    #[case("phone: (212) 234-5678, fax: 312-456-7890", &["(212) 234-5678", "312-456-7890"])]
    fn phone_in_context(#[case] input: &str, #[case] expected: &[&str]) {
        assert_eq!(scan(input), expected);
    }

    // False positive rejection
    #[rstest]
    #[case("1234567890")] // bare 10 digits, no separator
    #[case("12345")] // too short
    #[case("192.168.1.1")] // IP address (no area code 2-9 match for 1xx)
    #[case("2024-01-15")] // date
    #[case("order #5551234567")] // bare digits attached to '#'
    #[case("Hello world")] // plain text
    #[case("123-456-789")] // 9 digits, not 10
    #[case("+")] // lone plus
    #[case("+1")] // too few digits after CC
    #[case("+7612345")] // CC 7 requires 10 national digits, not 6
    fn phone_false_positive_rejected(#[case] input: &str) {
        assert_eq!(scan(input), Vec::<String>::new());
    }

    // Boundary checks
    #[rstest]
    #[case("abc+44 20 7946 0958xyz", &[])] // alphanumeric boundary
    #[case("_212-234-5678_", &["212-234-5678"])] // underscore is not a word boundary
    #[case("x5551234567", &[])] // leading alpha
    fn phone_boundary_rejected(#[case] input: &str, #[case] expected: &[&str]) {
        assert_eq!(scan(input), expected);
    }

    // NANP: area code and exchange must start with 2-9
    #[rstest]
    #[case("(155) 234-5678", &[])] // area code starts with 1
    #[case("(055) 234-5678", &[])] // area code starts with 0
    #[case("(555) 234-5678", &[])] // 555 is fictional/reserved
    #[case("(212) 023-4567", &[])] // exchange starts with 0
    #[case("(212) 234-5678", &["(212) 234-5678"])] // valid
    fn nanp_digit_constraints(#[case] input: &str, #[case] expected: &[&str]) {
        assert_eq!(scan(input), expected);
    }

    #[test]
    fn lookup_known_country_code() {
        let uk = lookup_country_code(44);
        assert!(uk.is_some());
        let (min, max) = uk.unwrap();
        assert!(min <= 10 && max >= 10, "UK should accept 10-digit numbers: ({min}, {max})");

        let us = lookup_country_code(1);
        assert!(us.is_some());
        let (min, max) = us.unwrap();
        assert_eq!((min, max), (10, 10), "US national numbers are always 10 digits");

        let cn = lookup_country_code(86);
        assert!(cn.is_some());
    }

    #[test]
    fn lookup_unknown_country_code() {
        assert_eq!(lookup_country_code(999), None);
    }

    #[test]
    fn unknown_cc_rejected() {
        assert_eq!(scan("+999 1234567"), Vec::<String>::new());
    }

    #[test]
    fn multibyte_utf8_safe() {
        assert_eq!(scan("€+44 20 7946 0958"), vec!["+44 20 7946 0958"]);
        assert_eq!(scan("电话+86 138 0013 8000号码"), vec!["+86 138 0013 8000"]);
    }
}
