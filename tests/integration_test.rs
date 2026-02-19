use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;

#[test]
fn test_help_command() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("pgsense-rs"));
    cmd.arg("--help");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Usage:"));
}

#[test]
fn test_version_command() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("pgsense-rs"));
    cmd.arg("--version");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("0.0.1"));
}

#[test]
fn test_rules_list_command() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("pgsense-rs"));
    cmd.args(["rules", "--rules", "config/rules.toml", "list"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("credit-card"))
        .stdout(predicate::str::contains("ssn"))
        .stdout(predicate::str::contains("rules loaded"));
}

#[test]
fn test_rules_test_detects_card() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("pgsense-rs"));
    cmd.args(["rules", "--rules", "config/rules.toml", "test", "--input", "4111111111111111"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("credit-card"))
        .stdout(predicate::str::contains("41************11"));
}

#[test]
fn test_rules_test_no_match() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("pgsense-rs"));
    cmd.args(["rules", "--rules", "config/rules.toml", "test", "--input", "hello world"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("No rules matched"));
}

#[test]
fn test_rules_list_without_rules_file_fails() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("pgsense-rs"));
    cmd.args(["rules", "list"]);

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("no rules file specified"));
}

#[test]
fn test_scan_appears_in_help() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("pgsense-rs"));
    cmd.arg("--help");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("scan"))
        .stdout(predicate::str::contains("rules"));
}

#[test]
fn test_rules_list_shows_phone() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("pgsense-rs"));
    cmd.args(["rules", "--rules", "config/rules.toml", "list"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("phone"));
}

#[test]
fn test_rules_test_detects_phone() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("pgsense-rs"));
    cmd.args(["rules", "--rules", "config/rules.toml", "test", "--input", "+44 20 7946 0958"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("phone"));
}

#[test]
fn test_invalid_command() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("pgsense-rs"));
    cmd.arg("nonexistent");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("unrecognized subcommand"));
}
