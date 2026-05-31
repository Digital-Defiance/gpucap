use assert_cmd::Command;
use predicates::prelude::*;

fn cmd() -> Command {
    Command::cargo_bin("bgpucap").expect("bgpucap binary not found")
}

#[test]
fn compare_two_json_reports() {
    let before = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/before.json");
    let after = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/after.json");

    cmd()
        .args(["compare", before, after])
        .assert()
        .success()
        .stdout(predicate::str::contains("gpu"))
        .stdout(predicate::str::contains("+20.0"))
        .stdout(predicate::str::contains("cpu"))
        .stdout(predicate::str::contains("+10.0"));
}

#[test]
fn compare_uses_common_metrics_only() {
    let before = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/before_power.json"
    );
    let after = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/after_basic.json"
    );

    cmd()
        .args(["compare", before, after])
        .assert()
        .success()
        .stdout(predicate::str::contains("gpu"))
        .stdout(predicate::str::contains("cpu"))
        .stdout(predicate::str::contains("gpu_power_w").not())
        .stderr(predicate::str::contains("gpu_power_w"));
}
