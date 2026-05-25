/// Luhn-doubled values: index d maps to (d*2) with digit-sum reduction.
const LUHN_DOUBLE: [u8; 10] = [0, 2, 4, 6, 8, 1, 3, 5, 7, 9];

/// Lookup table for valid email local-part characters (RFC 5321 dot-atom).
const EMAIL_LOCAL_CHAR: [bool; 128] = {
    let mut t = [false; 128];
    let mut b = b'a';
    while b <= b'z' {
        t[b as usize] = true;
        b += 1;
    }
    b = b'A';
    while b <= b'Z' {
        t[b as usize] = true;
        b += 1;
    }
    b = b'0';
    while b <= b'9' {
        t[b as usize] = true;
        b += 1;
    }
    t[b'.' as usize] = true;
    t[b'!' as usize] = true;
    t[b'#' as usize] = true;
    t[b'$' as usize] = true;
    t[b'%' as usize] = true;
    t[b'&' as usize] = true;
    t[b'\'' as usize] = true;
    t[b'*' as usize] = true;
    t[b'+' as usize] = true;
    t[b'/' as usize] = true;
    t[b'=' as usize] = true;
    t[b'?' as usize] = true;
    t[b'^' as usize] = true;
    t[b'_' as usize] = true;
    t[b'`' as usize] = true;
    t[b'{' as usize] = true;
    t[b'|' as usize] = true;
    t[b'}' as usize] = true;
    t[b'~' as usize] = true;
    t[b'-' as usize] = true;
    t
};

pub(crate) fn is_email_local_char(b: u8) -> bool {
    b.is_ascii() && EMAIL_LOCAL_CHAR[b as usize]
}

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

/// Validate a phone number via parse + validity check.
/// Handles E.164 (`+` prefix), `00` dial prefix, and bare NANP numbers.
pub fn phone(s: &str) -> bool {
    use rlibphonenumber::{PHONE_NUMBER_UTIL, Region};

    if s.starts_with('+') {
        PHONE_NUMBER_UTIL.parse(s, None)
    } else if let Some(rest) = s.strip_prefix("00") {
        PHONE_NUMBER_UTIL.parse(format!("+{rest}"), None)
    } else {
        // Bare NANP — US covers all NANP regions (US, CA, Caribbean share CC 1)
        PHONE_NUMBER_UTIL.parse(s, Some(Region::US))
    }
    .is_ok_and(|n| n.is_valid())
}

/// Validate an email address structure.
/// Checks local part (1-64 chars, valid charset, no
/// leading/trailing/consecutive dots), domain (labels split by `.`, each 1-63
/// chars, no leading/trailing hyphen), and TLD (>= 2 alpha chars).
pub fn email(s: &str) -> bool {
    let Some(at_pos) = s.find('@') else { return false };
    if s[at_pos + 1..].contains('@') {
        return false;
    }

    let local = &s[..at_pos];
    let domain = &s[at_pos + 1..];

    if local.is_empty() || local.len() > 64 {
        return false;
    }
    if local.starts_with('.') || local.ends_with('.') {
        return false;
    }
    if local.as_bytes().windows(2).any(|w| w == b"..") {
        return false;
    }
    if !local.as_bytes().iter().all(|&b| is_email_local_char(b)) {
        return false;
    }

    if domain.is_empty() || domain.len() > 253 {
        return false;
    }
    let mut label_count = 0u32;
    let mut last_label = "";
    for label in domain.split('.') {
        if label.is_empty() || label.len() > 63 {
            return false;
        }
        if label.starts_with('-') || label.ends_with('-') {
            return false;
        }
        for &b in label.as_bytes() {
            if !(b.is_ascii_alphanumeric() || b == b'-') {
                return false;
            }
        }
        last_label = label;
        label_count += 1;
    }

    if label_count < 2 {
        return false;
    }
    if last_label.len() < 2 {
        return false;
    }
    // Intentionally rejects punycode/IDN TLDs (e.g. xn--p1ai) — targets common PII
    // patterns
    last_label
        .as_bytes()
        .iter()
        .all(|b| b.is_ascii_alphabetic())
}

/// Validate a US Social Security Number format and check for invalid ranges.
/// Accepts `XXX-XX-XXXX`, `XXX XX XXXX`, and `XXX.XX.XXXX` formats.
/// SSN is always 11 ASCII characters, so we index bytes directly.
pub fn ssn(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 11 {
        return false;
    }

    let sep = b[3];
    if !matches!(sep, b'-' | b' ' | b'.') {
        return false;
    }
    if b[6] != sep {
        return false;
    }

    let Ok(area) = s[0..3].parse::<u16>() else { return false };
    let Ok(group) = s[4..6].parse::<u16>() else { return false };
    let Ok(serial) = s[7..11].parse::<u16>() else { return false };

    // Invalid: area 000, 666, or 900-999
    if area == 0 || area == 666 || area >= 900 {
        return false;
    }
    if group == 0 || serial == 0 {
        return false;
    }

    true
}

/// Validate an IBAN check digit using ISO 7064 mod-97-10.
/// Separators (spaces and dashes) are stripped before validation.
pub fn iban(s: &str) -> bool {
    let compact: Vec<u8> = s.bytes().filter(|b| b.is_ascii_alphanumeric()).collect();
    if compact.len() < 15 {
        return false;
    }

    // Move first 4 chars to end, convert letters A=10..Z=35, compute mod 97
    let rearranged = compact[4..].iter().chain(compact[..4].iter());
    let mut remainder: u32 = 0;

    for &b in rearranged {
        if b.is_ascii_digit() {
            remainder = (remainder * 10 + u32::from(b - b'0')) % 97;
        } else if b.is_ascii_uppercase() {
            let val = u32::from(b - b'A') + 10;
            remainder = (remainder * 100 + val) % 97;
        } else {
            return false;
        }
    }

    remainder == 1
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
    #[case("user@example.com", true)]
    #[case("first.last@domain.co.uk", true)]
    #[case("user+tag@sub.example.com", true)]
    #[case("a@b.cc", true)]
    #[case("user_name@example.com", true)]
    #[case("user@localhost", false)]
    #[case("user@.com", false)]
    #[case("user@domain.", false)]
    #[case(".user@example.com", false)]
    #[case("user.@example.com", false)]
    #[case("user..name@example.com", false)]
    #[case("@example.com", false)]
    #[case("user@", false)]
    #[case("user@-host.com", false)]
    #[case("user@host-.com", false)]
    #[case("user@example.1", false)]
    #[case("user@example.c", false)]
    #[case("", false)]
    // Boundary lengths
    #[case("abcdefghijklmnopqrstuvwxyz.abcdefghijklmnopqrstuvwxyz.abcdefghij@example.com", true)] // local = 64
    #[case("abcdefghijklmnopqrstuvwxyz.abcdefghijklmnopqrstuvwxyz.abcdefghijk@example.com", false)] // local = 65
    // Numeric non-TLD labels
    #[case("user@123.com", true)]
    #[case("user@example.co2", false)]
    // Punycode TLD intentionally rejected
    #[case("user@example.xn--p1ai", false)]
    fn email_validation(#[case] input: &str, #[case] expected: bool) {
        assert_eq!(email(input), expected, "email({input:?})");
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

    #[rstest]
    #[case("DE89370400440532013000", true)]
    #[case("GB29NWBK60161331926819", true)]
    #[case("FR7630006000011234567890189", true)]
    #[case("ES9121000418450200051332", true)]
    #[case("NL91ABNA0417164300", true)]
    #[case("NO9386011117947", true)]
    #[case("DE89 3704 0044 0532 0130 00", true)]
    #[case("DE89-3704-0044-0532-0130-00", true)]
    #[case("DE00370400440532013000", false)]
    #[case("NOTANIBAN", false)]
    #[case("", false)]
    fn iban_validation(#[case] input: &str, #[case] expected: bool) {
        assert_eq!(iban(input), expected, "iban({input:?})");
    }
}
