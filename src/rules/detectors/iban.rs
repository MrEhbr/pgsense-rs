use crate::rules::validators;

/// Find IBAN candidates by scanning for uppercase country-code pairs at word
/// boundaries, then accumulate + validate each candidate.
pub(super) fn scan(value: &str) -> Vec<String> {
    let bytes = value.as_bytes();
    bytes
        .windows(2)
        .enumerate()
        .filter(|&(i, w)| w[0].is_ascii_uppercase() && w[1].is_ascii_uppercase() && (i == 0 || !bytes[i - 1].is_ascii_alphanumeric()))
        .filter_map(|(i, w)| {
            let expected_len = country_length([w[0], w[1]])?;
            extract_candidate(bytes, value, i, expected_len)
        })
        .collect()
}

/// Accumulate alphanumeric chars (plus space/dash separators) starting at
/// `start`, validate length + right boundary + mod-97 check digit.
fn extract_candidate(bytes: &[u8], value: &str, start: usize, expected_len: u8) -> Option<String> {
    let expected_len = usize::from(expected_len);
    let mut j = start + 2;

    // IBAN spec: positions 2-3 are always decimal check digits (00-97)
    if !bytes.get(j).is_some_and(|b| b.is_ascii_digit()) || !bytes.get(j + 1).is_some_and(|b| b.is_ascii_digit()) {
        return None;
    }

    let mut alnum_count = 2usize;

    while j < bytes.len() && alnum_count < expected_len {
        match bytes[j] {
            b if b.is_ascii_alphanumeric() => {
                alnum_count += 1;
                j += 1;
            },
            b' ' | b'-' if alnum_count > 2 && bytes.get(j + 1).is_some_and(|b| b.is_ascii_alphanumeric()) => {
                j += 1;
            },
            _ => break,
        }
    }

    if alnum_count != expected_len {
        return None;
    }
    if bytes.get(j).is_some_and(|b| b.is_ascii_alphanumeric()) {
        return None;
    }

    let candidate = &value[start..j];
    validators::iban(candidate).then(|| candidate.to_string())
}

/// Return the total IBAN length (including country code and check digits) for a
/// SWIFT-registered country code, or `None` for unknown codes.
///
/// Lengths are packed into a `u16` match (`code[0] << 8 | code[1]`) for a
/// single branch instruction.
fn country_length(code: [u8; 2]) -> Option<u8> {
    let key = u16::from(code[0]) << 8 | u16::from(code[1]);
    // Source: https://www.swift.com/standards/data-standards/iban-international-bank-account-number
    match key {
        0x4144 => Some(28), // AD — Andorra
        0x4145 => Some(23), // AE — United Arab Emirates
        0x414C => Some(28), // AL — Albania
        0x4154 => Some(20), // AT — Austria
        0x415A => Some(28), // AZ — Azerbaijan
        0x4241 => Some(22), // BA — Bosnia and Herzegovina
        0x4245 => Some(16), // BE — Belgium
        0x4247 => Some(22), // BG — Bulgaria
        0x4248 => Some(22), // BH — Bahrain
        0x4249 => Some(29), // BI — Burundi
        0x4252 => Some(29), // BR — Brazil
        0x4259 => Some(28), // BY — Belarus
        0x4348 => Some(21), // CH — Switzerland
        0x4352 => Some(22), // CR — Costa Rica
        0x4359 => Some(28), // CY — Cyprus
        0x435A => Some(24), // CZ — Czech Republic
        0x4445 => Some(22), // DE — Germany
        0x444A => Some(18), // DJ — Djibouti
        0x444B => Some(18), // DK — Denmark
        0x444F => Some(28), // DO — Dominican Republic
        0x4545 => Some(20), // EE — Estonia
        0x4547 => Some(27), // EG — Egypt
        0x4553 => Some(24), // ES — Spain
        0x4649 => Some(18), // FI — Finland
        0x464B => Some(18), // FK — Falkland Islands
        0x464F => Some(18), // FO — Faroe Islands
        0x4652 => Some(27), // FR — France
        0x4742 => Some(22), // GB — United Kingdom
        0x4745 => Some(22), // GE — Georgia
        0x4749 => Some(24), // GI — Gibraltar
        0x474C => Some(18), // GL — Greenland
        0x4752 => Some(27), // GR — Greece
        0x4754 => Some(28), // GT — Guatemala
        0x4852 => Some(21), // HR — Croatia
        0x4855 => Some(22), // HU — Hungary
        0x4945 => Some(22), // IE — Ireland
        0x494C => Some(23), // IL — Israel
        0x4951 => Some(23), // IQ — Iraq
        0x4953 => Some(26), // IS — Iceland
        0x4954 => Some(27), // IT — Italy
        0x4A4F => Some(30), // JO — Jordan
        0x4B57 => Some(30), // KW — Kuwait
        0x4B5A => Some(20), // KZ — Kazakhstan
        0x4C42 => Some(28), // LB — Lebanon
        0x4C43 => Some(21), // LC — Saint Lucia
        0x4C49 => Some(21), // LI — Liechtenstein
        0x4C54 => Some(20), // LT — Lithuania
        0x4C55 => Some(20), // LU — Luxembourg
        0x4C56 => Some(21), // LV — Latvia
        0x4C59 => Some(25), // LY — Libya
        0x4D43 => Some(27), // MC — Monaco
        0x4D44 => Some(24), // MD — Moldova
        0x4D45 => Some(22), // ME — Montenegro
        0x4D4B => Some(19), // MK — North Macedonia
        0x4D52 => Some(27), // MR — Mauritania
        0x4D54 => Some(31), // MT — Malta
        0x4D55 => Some(30), // MU — Mauritius
        0x4D5A => Some(29), // MZ — Mozambique
        0x4E49 => Some(28), // NI — Nicaragua
        0x4E4C => Some(18), // NL — Netherlands
        0x4E4F => Some(15), // NO — Norway
        0x4F4D => Some(23), // OM — Oman
        0x504B => Some(24), // PK — Pakistan
        0x504C => Some(28), // PL — Poland
        0x5053 => Some(29), // PS — Palestinian territories
        0x5054 => Some(25), // PT — Portugal
        0x5141 => Some(29), // QA — Qatar
        0x524F => Some(24), // RO — Romania
        0x5253 => Some(33), // RS — Serbia
        0x5255 => Some(33), // RU — Russia
        0x5341 => Some(24), // SA — Saudi Arabia
        0x5343 => Some(31), // SC — Seychelles
        0x5344 => Some(18), // SD — Sudan
        0x5345 => Some(24), // SE — Sweden
        0x5349 => Some(19), // SI — Slovenia
        0x534B => Some(24), // SK — Slovakia
        0x534D => Some(28), // SM — San Marino
        0x534E => Some(28), // SN — Senegal
        0x5356 => Some(25), // SV — El Salvador
        0x544E => Some(24), // TN — Tunisia
        0x5452 => Some(26), // TR — Turkey
        0x5541 => Some(34), // UA — Ukraine
        0x5641 => Some(24), // VA — Vatican City
        0x5643 => Some(29), // VC — Saint Vincent
        0x564E => Some(30), // VN — Vietnam
        0x584B => Some(24), // XK — Kosovo
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    // Valid, compact
    #[case("DE89370400440532013000", &["DE89370400440532013000"])]
    #[case("GB29NWBK60161331926819", &["GB29NWBK60161331926819"])]
    #[case("FR7630006000011234567890189", &["FR7630006000011234567890189"])]
    #[case("ES9121000418450200051332", &["ES9121000418450200051332"])]
    #[case("NL91ABNA0417164300", &["NL91ABNA0417164300"])]
    #[case("NO9386011117947", &["NO9386011117947"])] // minimum length (15)
    // Spaced and dashed formatting
    #[case("DE89 3704 0044 0532 0130 00", &["DE89 3704 0044 0532 0130 00"])]
    #[case("DE89-3704-0044-0532-0130-00", &["DE89-3704-0044-0532-0130-00"])]
    // Embedded in text
    #[case("pay to DE89370400440532013000 please", &["DE89370400440532013000"])]
    #[case("ref: GB29NWBK60161331926819 thx", &["GB29NWBK60161331926819"])]
    // Multiple IBANs in one string
    #[case(
        "send DE89370400440532013000 and NL91ABNA0417164300",
        &["DE89370400440532013000", "NL91ABNA0417164300"]
    )]
    // Invalid check digits — must not match
    #[case("DE00370400440532013000", &[])]
    // Wrong length — DE needs 22, this has 21 alphanumeric chars
    #[case("DE8937040044053201300", &[])]
    // Unknown country code
    #[case("XX89370400440532013000", &[])]
    // Left-boundary violation: preceding alphanumeric must suppress match
    #[case("xDE89370400440532013000", &[])]
    // Right-boundary violation: trailing alphanumeric must suppress match
    #[case("DE89370400440532013000x", &[])]
    // False-positive defense: common English words starting with two uppercase letters
    #[case("DELIVERY2024", &[])]
    #[case("DECEMBER", &[])]
    // Empty / short input
    #[case("", &[])]
    #[case("DE", &[])]
    fn iban_scan(#[case] input: &str, #[case] expected: &[&str]) {
        assert_eq!(scan(input), expected);
    }
}
