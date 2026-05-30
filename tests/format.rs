use assert_cmd::Command;
use predicates::prelude::*;

fn cmd() -> Command {
    Command::cargo_bin("gpucap").expect("gpucap binary not found")
}

fn apple_silicon_host() -> bool {
    std::env::consts::OS == "macos"
        && std::env::consts::ARCH == "aarch64"
        && gpucap::check_apple_silicon().is_ok()
}

#[test]
fn format_flag_in_help() {
    cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("--format"));
}

#[test]
fn format_csv_output() {
    if !apple_silicon_host() {
        eprintln!("skipping format integration test (Apple Silicon required)");
        return;
    }

    cmd()
        .args(["-f", "%gA,%gP,%uA,%uP,%hA,%hP,%e,%Ws,%Wt", "sleep", "0.1"])
        .assert()
        .success()
        .stderr(predicate::str::is_match(
            r"^\d+\.\d+,\d+\.\d+,\d+\.\d+,\d+\.\d+,\d+\.\d+,\d+\.\d+,\d+\.\d+,\d+\.\d{9},\d+\.\d{9}\n$",
        )
        .unwrap());
}

#[test]
fn format_env_var() {
    if !apple_silicon_host() {
        eprintln!("skipping format integration test (Apple Silicon required)");
        return;
    }

    cmd()
        .env("GPUCAP_FORMAT", "cmd=%C x=%x")
        .args(["sleep", "0"])
        .assert()
        .success()
        .stderr(predicate::str::contains("cmd=sleep 0"))
        .stderr(predicate::str::contains("x=0"));
}

#[test]
fn format_plain_no_ansi() {
    if !apple_silicon_host() {
        eprintln!("skipping format integration test (Apple Silicon required)");
        return;
    }

    let output = cmd()
        .args(["--color=always", "-f", "gpu=%gA", "sleep", "0"])
        .output()
        .expect("failed to run gpucap");

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.starts_with("gpu="));
    assert!(
        !stderr.contains("\x1b["),
        "format output must be plain text even with --color=always"
    );
}

#[test]
fn format_brightdate_fields() {
    if !apple_silicon_host() {
        eprintln!("skipping format integration test (Apple Silicon required)");
        return;
    }

    cmd()
        .args(["-f", "start=%Ws end=%Wt elapsed=%dE", "sleep", "0"])
        .assert()
        .success()
        .stderr(predicate::str::is_match(
            r"^start=\d+\.\d{9} end=\d+\.\d{9} elapsed=\d+\.\d{6} md\n$",
        )
        .unwrap());
}

#[test]
fn format_default_constant() {
    if !apple_silicon_host() {
        eprintln!("skipping format integration test (Apple Silicon required)");
        return;
    }

    cmd()
        .env("GPUCAP_FORMAT", gpucap::DEFAULT_FORMAT)
        .args(["sleep", "0"])
        .assert()
        .success()
        .stderr(predicate::str::is_match(
            r"^\d+\.\d+,\d+\.\d+,\d+\.\d+,\d+\.\d+,\d+\.\d+,\d+\.\d+,\d+\.\d+,\d+\.\d{9},\d+\.\d{9}\n$",
        )
        .unwrap());
}
