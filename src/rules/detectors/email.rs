use crate::rules::validators;

/// Walk the string looking for `@` characters, extract local+domain candidates,
/// enforce word boundaries, then validate structure. Works on bytes directly
/// since email patterns are ASCII-only.
pub(super) fn scan(value: &str) -> Vec<String> {
    value
        .as_bytes()
        .iter()
        .enumerate()
        .filter(|&(_, &b)| b == b'@')
        .filter_map(|(at, _)| extract_candidate(value, at))
        .filter(|c| validators::email(c))
        .map(|c| c.to_string())
        .collect()
}

/// Given an `@` position, expand outward to extract the widest email candidate.
/// Returns `None` if boundaries or structure prevent a viable candidate.
fn extract_candidate(value: &str, at: usize) -> Option<&str> {
    let bytes = value.as_bytes();

    // Walk backward for local part (max 64 bytes per RFC 5321)
    let min_local = at.saturating_sub(64);
    let mut local_start = at;
    while local_start > min_local && validators::is_email_local_char(bytes[local_start - 1]) {
        local_start -= 1;
    }
    if local_start == at {
        return None;
    }

    // Walk forward for domain part (max 253 bytes per RFC 5321)
    let max_domain = (at + 1 + 253).min(bytes.len());
    let mut domain_end = at + 1;
    while domain_end < max_domain && is_domain_char(bytes[domain_end]) {
        domain_end += 1;
    }
    if domain_end == at + 1 {
        return None;
    }

    // Right boundary: following byte must not be alphanumeric or '_'
    if bytes
        .get(domain_end)
        .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_')
    {
        return None;
    }

    // Trim trailing dots/hyphens the domain scanner may have included
    while domain_end > at + 1 && matches!(bytes[domain_end - 1], b'.' | b'-') {
        domain_end -= 1;
    }

    Some(&value[local_start..domain_end])
}

fn is_domain_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'-' || b == b'.'
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    // Positive detection
    #[case("email is user@example.com", &["user@example.com"])]
    #[case("first.last@domain.co.uk", &["first.last@domain.co.uk"])]
    #[case("user+tag@example.com", &["user+tag@example.com"])]
    #[case("from alice@one.com and bob@two.org", &["alice@one.com", "bob@two.org"])]
    // Right boundary: underscore after domain skips candidate
    #[case("user@example.com_suffix", &[])]
    // Validator rejects numeric TLD (domain chars absorbed by scanner)
    #[case("user@example.com3", &[])]
    // Structural rejection
    #[case("user@", &[])]
    #[case("@example.com", &[])]
    #[case("user..name@example.com", &[])]
    #[case(".user@example.com", &[])]
    #[case("user.@example.com", &[])]
    #[case("user@-example.com", &[])]
    #[case("user@example-.com", &[])]
    #[case("user@example.1", &[])]
    #[case("Hello world", &[])]
    #[case("@@", &[])]
    // UTF-8: multi-byte chars around the email
    #[case("連絡 user@example.com please", &["user@example.com"])]
    #[case("💌 user@example.com 💌", &["user@example.com"])]
    // UTF-8: multi-byte chars inside must not match
    #[case("user@exam\u{200B}ple.com", &[])]
    // Trailing-dot/hyphen trimming
    #[case("user@example.com.", &["user@example.com"])]
    #[case("user@example.com-", &["user@example.com"])]
    // Multi-@ produces artifact matches (known limitation)
    #[case("a@b.co@d.com", &["a@b.co", "b.co@d.com"])]
    fn email_scan(#[case] input: &str, #[case] expected: &[&str]) {
        assert_eq!(scan(input), expected);
    }
}
