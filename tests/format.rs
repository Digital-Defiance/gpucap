use assert_cmd::Command;
use predicates::prelude::*;

fn cmd() -> Command {
    Command::cargo_bin("bgpucap").expect("bgpucap binary not found")
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
        .env("BGPUCAP_FORMAT", "cmd=%C x=%x")
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
        .expect("failed to run bgpucap");

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
fn format_extended_metrics() {
    if !apple_silicon_host() {
        eprintln!("skipping extended format integration test (Apple Silicon required)");
        return;
    }

    cmd()
        .env(
            "BGPUCAP_FORMAT",
            "mem=%gI freq=%gF cpu=%uF",
        )
        .args(["sleep", "0.3"])
        .assert()
        .success()
        .stderr(predicate::str::is_match(
            r"^mem=\d+ freq=\d+ cpu=\d+\n$",
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
        .env("BGPUCAP_FORMAT", gpucap::DEFAULT_FORMAT)
        .args(["sleep", "0"])
        .assert()
        .success()
        .stderr(predicate::str::is_match(
            r"^\d+\.\d+,\d+\.\d+,\d+\.\d+,\d+\.\d+,\d+\.\d+,\d+\.\d+,\d+\.\d+,\d+\.\d{9},\d+\.\d{9}\n$",
        )
        .unwrap());
}

#[test]
fn metrics_filter_basic_human_output() {
    if !apple_silicon_host() {
        eprintln!("skipping metrics filter test (Apple Silicon required)");
        return;
    }

    cmd()
        .args(["--metrics", "basic", "--no-color", "sleep", "0"])
        .assert()
        .success()
        .stderr(predicate::str::contains("gpu"))
        .stderr(predicate::str::contains("cpu"))
        .stderr(predicate::str::contains("memory"))
        .stderr(predicate::str::contains("real"))
        .stderr(predicate::str::contains("gpu-pwr").not())
        .stderr(predicate::str::contains("renderer").not());
}

#[test]
fn metrics_filter_unknown_name_fails() {
    cmd()
        .args(["--metrics", "not-a-metric", "sleep", "0"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown metric"));
}

#[test]
fn json_format_output_stdout() {
    if !apple_silicon_host() {
        eprintln!("skipping json format test (Apple Silicon required)");
        return;
    }

    let output = cmd()
        .args(["--metrics", "basic", "-f", "json", "sleep", "0"])
        .output()
        .expect("failed to run bgpucap");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"gpu\""));
    assert!(stdout.contains("\"elapsed_secs\""));
    assert!(stdout.contains("\"chip\""));
}

#[test]
fn compare_subcommand_help() {
    cmd()
        .args(["compare", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Compare metrics"));
}

#[test]
fn watch_subcommand_help() {
    cmd()
        .args(["watch", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("continuously"));
}

#[test]
fn list_metrics_flag() {
    cmd()
        .arg("--list-metrics")
        .assert()
        .success()
        .stdout(predicate::str::contains("basic"))
        .stdout(predicate::str::contains("gpu-pwr"));
}

#[test]
fn format_cluster_power_metrics() {
    if !apple_silicon_host() {
        eprintln!("skipping cluster power format test (Apple Silicon required)");
        return;
    }

    cmd()
        .env("BGPUCAP_FORMAT", "cpu=%uB e=%uG p=%uI")
        .args(["sleep", "1"])
        .assert()
        .success()
        .stderr(predicate::str::is_match(
            r"^cpu=\d+(\.\d+)? e=\d+(\.\d+)? p=\d+(\.\d+)?\n$",
        )
        .unwrap());
}

#[test]
fn format_system_power_metrics() {
    if !apple_silicon_host() {
        eprintln!("skipping power format integration test (Apple Silicon required)");
        return;
    }

    cmd()
        .env("BGPUCAP_FORMAT", "gpu=%gB cpu=%uB dram=%hG ane=%aB")
        .args(["sleep", "1"])
        .assert()
        .success()
        .stderr(predicate::str::is_match(
            r"^gpu=\d+(\.\d+)? cpu=\d+(\.\d+)? dram=\d+(\.\d+)? ane=\d+(\.\d+)?\n$",
        )
        .unwrap());
}
