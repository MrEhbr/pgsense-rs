use std::path::PathBuf;

use anyhow::{Context, Result};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};

/// A secret value sourced either inline or from a file on disk.
///
/// Deserialized via serde from either a string or a `{ file = "..." }` table.
/// In TOML, this looks like:
///   `key = "literal-value"`        → [`Secret::Inline`]
///   `key = { file = "/path" }`     → [`Secret::File`]
///
/// File-backed secrets must be resolved with [`Secret::resolve`] before use;
/// [`Secret::expose`] panics on an unresolved [`Secret::File`].
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Secret {
    Inline(#[serde(skip_serializing)] SecretString),
    File { file: PathBuf },
}

impl Secret {
    /// If this is `Secret::File`, read the file (trimming trailing whitespace)
    /// and replace `self` with `Secret::Inline`. No-op for already-inline
    /// secrets.
    pub fn resolve(&mut self) -> Result<()> {
        if let Secret::File { file } = self {
            let content = std::fs::read_to_string(&file).with_context(|| format!("failed to read secret file: {}", file.display()))?;
            *self = Secret::Inline(SecretString::from(content.trim_end().to_string()));
        }
        Ok(())
    }

    /// Expose the underlying secret. Panics if the secret has not been
    /// resolved (i.e. is still `Secret::File`) — this indicates a programmer
    /// error: someone forgot to call [`Secret::resolve`] (or
    /// `Config::resolve_secrets`) before consuming the value.
    pub fn expose(&self) -> &SecretString {
        match self {
            Secret::Inline(s) => s,
            Secret::File { file } => panic!("Secret::expose called on unresolved File variant ({})", file.display()),
        }
    }
}

impl From<SecretString> for Secret {
    fn from(s: SecretString) -> Self {
        Secret::Inline(s)
    }
}

impl From<String> for Secret {
    fn from(s: String) -> Self {
        Secret::Inline(SecretString::from(s))
    }
}

impl From<&str> for Secret {
    fn from(s: &str) -> Self {
        Secret::Inline(SecretString::from(s.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use secrecy::ExposeSecret;
    use serde::Deserialize;
    use tempfile::NamedTempFile;

    use super::*;

    #[derive(Deserialize)]
    struct Holder {
        secret: Secret,
    }

    #[test]
    fn deserializes_inline_string() {
        let toml = r#"secret = "hello""#;
        let h: Holder = toml::from_str(toml).unwrap();
        match h.secret {
            Secret::Inline(s) => assert_eq!(s.expose_secret(), "hello"),
            Secret::File { .. } => panic!("expected inline"),
        }
    }

    #[test]
    fn deserializes_file_table() {
        let toml = r#"secret = { file = "/run/secrets/foo" }"#;
        let h: Holder = toml::from_str(toml).unwrap();
        match h.secret {
            Secret::File { file } => assert_eq!(file, PathBuf::from("/run/secrets/foo")),
            Secret::Inline(_) => panic!("expected file"),
        }
    }

    #[test]
    fn resolve_reads_file_and_trims() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "p4ssw0rd\n\n").unwrap();
        let mut secret = Secret::File {
            file: file.path().to_path_buf(),
        };
        secret.resolve().unwrap();
        assert_eq!(secret.expose().expose_secret(), "p4ssw0rd");
    }

    #[test]
    fn resolve_missing_file_errors() {
        let mut secret = Secret::File {
            file: PathBuf::from("/nonexistent/secret/file"),
        };
        let err = secret.resolve().unwrap_err();
        assert!(err.to_string().contains("failed to read secret file"));
    }

    #[test]
    fn resolve_is_idempotent() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "abc").unwrap();
        let mut secret = Secret::File {
            file: file.path().to_path_buf(),
        };
        secret.resolve().unwrap();
        secret.resolve().unwrap();
        assert_eq!(secret.expose().expose_secret(), "abc");
    }

    #[test]
    fn resolve_inline_is_noop() {
        let mut secret = Secret::Inline(SecretString::from("inline".to_string()));
        secret.resolve().unwrap();
        assert_eq!(secret.expose().expose_secret(), "inline");
    }

    #[test]
    #[should_panic(expected = "Secret::expose called on unresolved File")]
    fn expose_panics_on_unresolved_file() {
        let secret = Secret::File {
            file: PathBuf::from("/some/path"),
        };
        let _ = secret.expose();
    }
}
