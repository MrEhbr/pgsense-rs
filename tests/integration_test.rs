use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;

fn cmd() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("pgsense-rs"))
}

mod help {
    use super::*;

    #[test]
    fn shows_usage_and_subcommands() {
        cmd()
            .arg("--help")
            .assert()
            .success()
            .stdout(predicate::str::contains("Usage:"))
            .stdout(predicate::str::contains("scan"))
            .stdout(predicate::str::contains("rules"))
            .stdout(predicate::str::contains("validate"));
    }

    #[test]
    fn rejects_invalid_command() {
        cmd()
            .arg("nonexistent")
            .assert()
            .failure()
            .stderr(predicate::str::contains("unrecognized subcommand"));
    }
}

mod rules {
    mod list {
        use super::super::*;

        #[test]
        fn shows_builtin_rules() {
            cmd()
                .args(["rules", "--rules", "config/rules.toml", "list"])
                .assert()
                .success()
                .stdout(predicate::str::contains("credit-card"))
                .stdout(predicate::str::contains("ssn"))
                .stdout(predicate::str::contains("phone"))
                .stdout(predicate::str::contains("rules loaded"));
        }

        #[test]
        fn without_rules_file_fails() {
            cmd()
                .args(["rules", "list"])
                .assert()
                .failure()
                .stderr(predicate::str::contains("no rules file specified"));
        }
    }

    mod test_cmd {
        use super::super::*;

        #[test]
        fn detects_credit_card() {
            cmd()
                .args(["rules", "--rules", "config/rules.toml", "test", "--input", "4111111111111111"])
                .assert()
                .success()
                .stdout(predicate::str::contains("credit-card"))
                .stdout(predicate::str::contains("41************11"));
        }

        #[test]
        fn detects_phone() {
            cmd()
                .args(["rules", "--rules", "config/rules.toml", "test", "--input", "+44 20 7946 0958"])
                .assert()
                .success()
                .stdout(predicate::str::contains("phone"));
        }

        #[test]
        fn reports_no_match() {
            cmd()
                .args(["rules", "--rules", "config/rules.toml", "test", "--input", "hello world"])
                .assert()
                .success()
                .stdout(predicate::str::contains("No rules matched"));
        }
    }

    mod bench {
        use super::super::*;

        #[test]
        fn with_generate_prints_table() {
            cmd()
                .args(["rules", "--rules", "config/rules.toml", "bench", "--generate", "10", "--iterations", "10"])
                .assert()
                .success()
                .stdout(predicate::str::contains("RULE ID"))
                .stdout(predicate::str::contains("MEAN"))
                .stdout(predicate::str::contains("P95"))
                .stdout(predicate::str::contains("Engine scan_value()"));
        }

        #[test]
        fn with_input_runs() {
            cmd()
                .args([
                    "rules",
                    "--rules",
                    "config/rules.toml",
                    "bench",
                    "--input",
                    "4111111111111111",
                    "--iterations",
                    "5",
                ])
                .assert()
                .success()
                .stdout(predicate::str::contains("credit-card"));
        }

        #[test]
        fn json_format_is_valid() {
            let out = cmd()
                .args([
                    "rules",
                    "--rules",
                    "config/rules.toml",
                    "bench",
                    "--generate",
                    "5",
                    "--iterations",
                    "5",
                    "--format",
                    "json",
                ])
                .assert()
                .success()
                .get_output()
                .stdout
                .clone();
            let parsed: serde_json::Value = serde_json::from_slice(&out).expect("output must be valid JSON");
            assert!(parsed["rules"].is_array());
            assert!(parsed["engine"]["mean"].is_string());
            assert_eq!(parsed["config"]["iterations"], 5);
            assert_eq!(parsed["config"]["values"], 5);
        }

        #[test]
        fn no_input_uses_default_generate() {
            cmd()
                .args(["rules", "--rules", "config/rules.toml", "bench", "--iterations", "5"])
                .assert()
                .success()
                .stdout(predicate::str::contains("100 values"));
        }

        #[test]
        fn input_flags_are_mutually_exclusive() {
            cmd()
                .args(["rules", "--rules", "config/rules.toml", "bench", "--input", "x", "--generate", "10"])
                .assert()
                .failure()
                .stderr(predicate::str::contains("cannot be used with"));
        }
    }
}

mod validate {
    use super::*;

    #[test]
    fn help_lists_connect_flag() {
        cmd()
            .args(["validate", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("--connect"));
    }

    #[test]
    fn missing_config_arg_fails() {
        cmd().arg("validate").assert().failure();
    }

    #[test]
    fn nonexistent_config_fails() {
        cmd()
            .args(["validate", "-c", "/this/path/does/not/exist.toml"])
            .assert()
            .failure()
            .stdout(predicate::str::contains("[ERROR]"));
    }

    #[test]
    fn valid_config_succeeds() {
        cmd()
            .args(["validate", "-c", "config/config.toml", "-r", "config/rules.toml"])
            .assert()
            .success()
            .stdout(predicate::str::contains("0 errors"));
    }

    #[test]
    fn invalid_rules_regex_fails() {
        let dir = tempfile::tempdir().unwrap();
        let rules_path = dir.path().join("bad-rules.toml");
        std::fs::write(
            &rules_path,
            r#"[[rules]]
id = "bad"
description = "x"
category = "TEST"
severity = "medium"
type = "regex"
pattern = "[invalid"
"#,
        )
        .unwrap();

        cmd()
            .args(["validate", "-c", "config/config.toml", "-r", rules_path.to_str().unwrap()])
            .assert()
            .failure()
            .stdout(predicate::str::contains("invalid regex"));
    }

    #[test]
    fn invalid_postgres_schema_fails() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        std::fs::write(
            &config_path,
            r#"
[[databases]]
host = "localhost"
dbname = "mydb"
username = "postgres"
password = "x"

[alerts.postgres]
host = "localhost"
dbname = "postgres"
username = "postgres"
password = "x"
schema = "bad-schema"
table = "findings"
"#,
        )
        .unwrap();

        cmd()
            .args(["validate", "-c", config_path.to_str().unwrap()])
            .assert()
            .failure()
            .stdout(predicate::str::contains("invalid schema name"));
    }
}
