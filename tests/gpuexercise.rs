use assert_cmd::Command;
use predicates::prelude::*;
use std::process::Output;

const EXERCISE_SECONDS: &str = "5";
const TARGETS: [u32; 4] = [25, 50, 75, 100];

fn cmd() -> Command {
    Command::cargo_bin("bgpucap").expect("bgpucap binary not found")
}

fn apple_silicon_host() -> bool {
    std::env::consts::OS == "macos"
        && std::env::consts::ARCH == "aarch64"
        && gpucap::check_apple_silicon().is_ok()
}

fn run_exercise(percent: u32) -> Output {
    cmd()
        .args([
            "gpuexercise",
            "--percent",
            &percent.to_string(),
            "--seconds",
            EXERCISE_SECONDS,
            "--no-color",
        ])
        .output()
        .expect("failed to spawn bgpucap gpuexercise")
}

fn is_gpu_util_line(line: &str) -> bool {
    line.starts_with("gpu") && !line.starts_with("gpu-")
}

fn parse_gpu_avg(stderr: &str) -> Option<f64> {
    stderr.lines().find_map(|line| {
        if !is_gpu_util_line(line) {
            return None;
        }
        let after_avg = line.split("avg ").nth(1)?;
        let num = after_avg.split('%').next()?.trim();
        num.parse().ok()
    })
}

fn parse_gpu_peak(stderr: &str) -> Option<f64> {
    stderr.lines().find_map(|line| {
        if !is_gpu_util_line(line) {
            return None;
        }
        let after_peak = line.split("peak ").nth(1)?;
        let num = after_peak.split('%').next()?.trim();
        num.parse().ok()
    })
}

#[test]
fn gpuexercise_help() {
    cmd()
        .args(["gpuexercise", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--percent"))
        .stdout(predicate::str::contains("--seconds"));
}

#[test]
fn gpuexercise_target_levels() {
    if !apple_silicon_host() {
        eprintln!("skipping gpuexercise integration tests (Apple Silicon required)");
        return;
    }

    let mut avgs = Vec::with_capacity(TARGETS.len());

    for &percent in &TARGETS {
        let output = run_exercise(percent);
        let stderr = String::from_utf8_lossy(&output.stderr);

        assert!(
            output.status.success(),
            "gpuexercise --percent {percent} failed:\n{stderr}"
        );

        assert!(
            stderr.contains(&format!("{percent}%")) && stderr.contains("target"),
            "missing target line for {percent}%:\n{stderr}"
        );
        assert!(
            stderr.contains("for 5.0 s"),
            "missing duration line for {percent}%:\n{stderr}"
        );
        assert!(
            stderr.contains("gpu") && stderr.contains("avg") && stderr.contains("peak"),
            "missing gpu stats for {percent}%:\n{stderr}"
        );
        assert!(
            stderr.contains("gpu-pwr") || stderr.contains("gpu-mhz"),
            "missing extended gpu metrics for {percent}%:\n{stderr}"
        );

        let avg = parse_gpu_avg(&stderr).unwrap_or_else(|| {
            panic!("could not parse gpu avg for {percent}%:\n{stderr}");
        });
        let peak = parse_gpu_peak(&stderr).unwrap_or_else(|| {
            panic!("could not parse gpu peak for {percent}%:\n{stderr}");
        });

        assert!(
            (0.0..=100.0).contains(&avg),
            "gpu avg out of range for {percent}%: {avg}"
        );
        assert!(
            (0.0..=100.0).contains(&peak),
            "gpu peak out of range for {percent}%: {peak}"
        );
        assert!(peak >= avg - 0.1, "peak {peak} < avg {avg} for {percent}%");

        avgs.push((percent, avg));
    }

    // Ordering is best-effort: IOKit can report saturated 100% at low targets.
    let avg_25 = avgs.iter().find(|(p, _)| *p == 25).unwrap().1;
    let avg_100 = avgs.iter().find(|(p, _)| *p == 100).unwrap().1;
    if avg_25 < 90.0 && avg_100 < 90.0 {
        assert!(
            avg_100 >= avg_25 - 10.0,
            "expected 100% target avg ({avg_100}) >= 25% target avg ({avg_25}) - 10"
        );
    }
}

#[test]
fn gpuexercise_command_gpu_tracking() {
    if !apple_silicon_host() {
        eprintln!("skipping gpuexercise command gpu test (Apple Silicon required)");
        return;
    }

    let output = cmd()
        .args([
            "gpuexercise",
            "-p",
            "50",
            "-s",
            "3",
            "--no-color",
            "-f",
            "sys=%gA cmd=%gC",
        ])
        .output()
        .expect("failed to run gpuexercise");

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("sys=") && stderr.contains("cmd="),
        "missing command gpu format fields:\n{stderr}"
    );
}

#[test]
fn gpuexercise_format_power_and_freq() {
    if !apple_silicon_host() {
        eprintln!("skipping gpuexercise format test (Apple Silicon required)");
        return;
    }

    let output = cmd()
        .args([
            "gpuexercise",
            "-p",
            "50",
            "-s",
            "3",
            "-f",
            "gpu=%gA pwr=%gB mhz=%gF target=%tG",
        ])
        .output()
        .expect("failed to run gpuexercise with format");

    assert!(output.status.success(), "gpuexercise failed");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("gpu=") && stderr.contains("pwr=") && stderr.contains("mhz="),
        "missing format fields:\n{stderr}"
    );
    assert!(stderr.contains("target=50"), "missing target field:\n{stderr}");
}
