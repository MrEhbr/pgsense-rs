mod credit_card;
mod email;
mod phone;
mod ssn;

use super::config::BuiltinKind;

/// Algorithmic detectors that don't rely on regex.
#[derive(Debug, Clone, Copy)]
pub enum Detector {
    CreditCard,
    Email,
    Phone,
    Ssn,
}

impl Detector {
    pub fn from_kind(kind: BuiltinKind) -> Self {
        match kind {
            BuiltinKind::CreditCard => Detector::CreditCard,
            BuiltinKind::Email => Detector::Email,
            BuiltinKind::Phone => Detector::Phone,
            BuiltinKind::Ssn => Detector::Ssn,
        }
    }

    pub fn scan(&self, value: &str) -> Vec<String> {
        match self {
            Detector::CreditCard => credit_card::scan(value),
            Detector::Email => email::scan(value),
            Detector::Phone => phone::scan(value),
            Detector::Ssn => ssn::scan(value),
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(Detector::CreditCard, "4111111111111111", 1)]
    #[case(Detector::Email, "user@example.com", 1)]
    #[case(Detector::Ssn, "123-45-6789", 1)]
    #[case(Detector::Phone, "+44 20 7946 0958", 1)]
    fn detector_dispatch(#[case] detector: Detector, #[case] input: &str, #[case] count: usize) {
        assert_eq!(detector.scan(input).len(), count);
    }
}
