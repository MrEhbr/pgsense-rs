/// Luhn-doubled values: index d maps to (d*2) with digit-sum reduction.
const LUHN_DOUBLE: [u8; 10] = [0, 2, 4, 6, 8, 1, 3, 5, 7, 9];

/// Strips non-digit characters before checking.
pub fn luhn(s: &str) -> bool {
    let mut digits = [0u8; 19];
    let mut len = 0usize;

    for &b in s.as_bytes() {
        if b.is_ascii_digit() {
            if len >= 19 {
                return false;
            }
            digits[len] = b - b'0';
            len += 1;
        }
    }

    if len < 13 {
        return false;
    }

    let mut sum: u16 = 0;
    let mut double = false;
    for &d in digits[..len].iter().rev() {
        sum += if double { u16::from(LUHN_DOUBLE[d as usize]) } else { u16::from(d) };
        double = !double;
    }

    sum.is_multiple_of(10)
}

/// Validate a phone number using libphonenumber.
/// Handles E.164 (`+` prefix), `00` dial prefix, and bare NANP numbers.
pub fn phone(s: &str) -> bool {
    if s.starts_with('+') {
        phonenumber::parse(None, s)
    } else if let Some(rest) = s.strip_prefix("00") {
        phonenumber::parse(None, format!("+{rest}"))
    } else {
        // Bare NANP — US covers all NANP regions (US, CA, Caribbean share CC 1)
        phonenumber::parse(Some(phonenumber::country::US), s)
    }
    .is_ok_and(|n| phonenumber::is_valid(&n))
}

/// Validate a US Social Security Number format and check for invalid ranges.
/// Accepts `XXX-XX-XXXX`, `XXX XX XXXX`, and `XXX.XX.XXXX` formats.
pub fn ssn(s: &str) -> bool {
    let sep = match s.chars().nth(3) {
        Some(c @ ('-' | ' ' | '.')) => c,
        _ => return false,
    };
    let parts: Vec<&str> = s.split(sep).collect();
    if parts.len() != 3 {
        return false;
    }

    let area: u16 = match parts[0].parse() {
        Ok(n) => n,
        Err(_) => return false,
    };
    let group: u16 = match parts[1].parse() {
        Ok(n) => n,
        Err(_) => return false,
    };
    let serial: u16 = match parts[2].parse() {
        Ok(n) => n,
        Err(_) => return false,
    };

    // Invalid: area 000, 666, or 900-999
    if area == 0 || area == 666 || area >= 900 {
        return false;
    }
    if group == 0 {
        return false;
    }
    if serial == 0 {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case("4111111111111111", true)]
    #[case("5500000000000004", true)]
    #[case("378282246310005", true)]
    #[case("6011111111111117", true)]
    #[case("4111-1111-1111-1111", true)]
    #[case("4111 1111 1111 1111", true)]
    #[case("4111111111111112", false)]
    #[case("411111", false)]
    #[case("41111111111111111111", false)]
    #[case("abcdefghijklm", false)]
    #[case("", false)]
    #[case("0000000000000000", true)]
    fn luhn_validation(#[case] input: &str, #[case] expected: bool) {
        assert_eq!(luhn(input), expected, "luhn({input:?})");
    }

    #[rstest]
    #[case("123-45-6789", true)]
    #[case("001-01-0001", true)]
    #[case("123 45 6789", true)]
    #[case("123.45.6789", true)]
    #[case("123-45.6789", false)]
    #[case("123 45-6789", false)]
    #[case("000-12-3456", false)]
    #[case("666-12-3456", false)]
    #[case("900-12-3456", false)]
    #[case("999-12-3456", false)]
    #[case("123-00-3456", false)]
    #[case("123-45-0000", false)]
    #[case("", false)]
    fn ssn_validation(#[case] input: &str, #[case] expected: bool) {
        assert_eq!(ssn(input), expected, "ssn({input:?})");
    }

    #[rstest]
    #[case("+44 20 7946 0958", true)]
    #[case("+1 212 234 5678", true)]
    #[case("+86 138 0013 8000", true)]
    #[case("0044 20 7946 0958", true)]
    #[case("(212) 234-5678", true)]
    #[case("312-456-7890", true)]
    #[case("(555) 234-5678", false)] // 555 is fictional
    #[case("+999 1234567", false)] // invalid country code
    #[case("not a phone", false)]
    #[case("", false)]
    fn phone_validation(#[case] input: &str, #[case] expected: bool) {
        assert_eq!(phone(input), expected, "phone({input:?})");
    }
}
