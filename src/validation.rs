/// Returns a list of error messages describing why `self` is invalid.
/// Empty vec means valid.
///
/// `name` identifies the instance for error messages (e.g. an alert channel
/// name). Implementations should embed it in their messages so callers can
/// attribute failures back to a config entry.
pub trait Validate {
    fn validate(&self, name: &str) -> Vec<String>;
}
