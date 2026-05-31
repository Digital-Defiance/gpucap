use crate::format::{summarize_line, FormatContext};
use crate::gpu_proc::GpuProcessTracker;
use crate::json;
use crate::metrics::Sampler;
use crate::output::print_report;
use crate::runner::RunStats;
use crate::platform::ChipProfile;
use crate::{resolve_report_style, resolve_sample_tier, MetricFilter};
use brightdate::BrightDate;
use clap::{Arg, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

static WATCH_STOP: AtomicBool = AtomicBool::new(false);

extern "C" fn watch_sigint(_: i32) {
    WATCH_STOP.store(true, Ordering::SeqCst);
}

pub fn run(args: &[String]) -> i32 {
    if let Err(code) = crate::platform::ensure_apple_silicon() {
        return code;
    }

    let cmd = Command::new("watch")
        .about("Sample GPU, CPU, and memory continuously until interrupted")
        .arg(
            Arg::new("interval")
                .long("interval")
                .value_name("MS")
                .help("Sampling interval in milliseconds")
                .default_value("1000"),
        )
        .arg(
            Arg::new("count")
                .long("count")
                .short('n')
                .value_name("N")
                .help("Stop after N samples (default: until Ctrl+C)"),
        )
        .arg(
            Arg::new("format")
                .short('f')
                .long("format")
                .value_name("FORMAT")
                .help("Per-sample output: omit for human lines on stderr, or 'json' for NDJSON on stdout"),
        )
        .args(crate::output_style_args())
        .args(crate::common_args());

    let argv: Vec<&str> = std::iter::once("bgpucap")
        .chain(args.iter().skip(2).map(String::as_str))
        .collect();

    let matches = match cmd.try_get_matches_from(&argv) {
        Ok(m) => m,
        Err(e) => {
            let _ = e.print();
            return e.exit_code();
        }
    };

    let colors = match crate::resolve_color_settings(&matches) {
        Ok(c) => c,
        Err(code) => return code,
    };

    let interval_ms: u64 = match matches.get_one::<String>("interval").unwrap().parse() {
        Ok(ms) if ms > 0 => ms,
        _ => {
            eprintln!("bgpucap watch: interval must be a positive integer (milliseconds)");
            return 2;
        }
    };

    let max_samples = matches
        .get_one::<String>("count")
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|&n| n > 0);

    let style = match resolve_report_style(&matches) {
        Ok(s) => s,
        Err(code) => return code,
    };

    let format = crate::resolve_format(&matches);
    let json_live = format.as_deref().is_some_and(json::is_json_format);
    let tier = resolve_sample_tier(&style.metrics, format.as_deref());
    let gpu_track = crate::resolve_gpu_track(&matches, &style.metrics);

    WATCH_STOP.store(false, Ordering::SeqCst);
    unsafe {
        libc::signal(libc::SIGINT, watch_sigint as *const () as libc::sighandler_t);
    }

    eprintln!("bgpucap watch: sampling every {interval_ms} ms (Ctrl+C to stop)…");

    let chip = ChipProfile::detect();
    let start_bd = BrightDate::now().value;
    let mut sampler = Sampler::with_tier(tier);
    let mut stats = RunStats::default();
    let mut gpu_proc = match gpu_track {
        crate::GpuTrackPid::Pid(pid) => GpuProcessTracker::new(pid),
        _ => None,
    };
    let tracked_pid = match gpu_track {
        crate::GpuTrackPid::Pid(pid) => Some(pid),
        _ => None,
    };

    let mut samples = 0u32;
    let interval = Duration::from_millis(interval_ms);

    while !WATCH_STOP.load(Ordering::SeqCst) {
        if let Some(limit) = max_samples {
            if samples >= limit {
                break;
            }
        }

        let snapshot = sampler.sample();
        if let Some(tracker) = gpu_proc.as_mut() {
            if let Some(cmd_gpu) = tracker.sample(interval.as_secs_f64()) {
                let mut snap = snapshot;
                snap.command_gpu = Some(cmd_gpu);
                stats.record(&snap);
                print_live_sample(
                    json_live,
                    &style.metrics,
                    &snap,
                    BrightDate::now().value,
                );
            } else {
                stats.record(&snapshot);
                print_live_sample(json_live, &style.metrics, &snapshot, BrightDate::now().value);
            }
        } else {
            stats.record(&snapshot);
            print_live_sample(json_live, &style.metrics, &snapshot, BrightDate::now().value);
        }

        samples += 1;
        if max_samples.is_some_and(|n| samples >= n) {
            break;
        }
        if WATCH_STOP.load(Ordering::SeqCst) {
            break;
        }
        std::thread::sleep(interval);
    }

    unsafe {
        libc::signal(libc::SIGINT, libc::SIG_DFL);
    }

    if samples == 0 {
        eprintln!("bgpucap watch: no samples collected");
        return 0;
    }

    let elapsed = interval.as_secs_f64() * samples as f64;
    let result = stats.into_run_result(
        vec!["watch".into()],
        0,
        elapsed,
        start_bd,
        BrightDate::now().value,
        tracked_pid,
    );

    eprintln!();
    if json_live {
        if let Err(e) = json::write_result(
            &mut std::io::stdout(),
            &result,
            &chip,
            &style.metrics,
            None,
        ) {
            eprintln!("bgpucap watch: {e}");
            return 1;
        }
    } else if let Some(fmt) = format.as_deref() {
        let ctx = FormatContext::from_run(&result, None);
        if let Err(e) = summarize_line(&mut std::io::stderr(), fmt, &ctx) {
            eprintln!("bgpucap watch: {e}");
            return 1;
        }
    } else {
        print_report(&colors, &chip, &result, &style);
    }

    0
}

fn print_live_sample(
    json_live: bool,
    filter: &MetricFilter,
    snapshot: &crate::metrics::MetricSnapshot,
    bd: f64,
) {
    if json_live {
        let _ = json::write_sample_line(&mut std::io::stdout(), filter, snapshot, bd);
    } else {
        eprint!(
            "  gpu {:5.1}%  cpu {:5.1}%  mem {:5.1}%",
            snapshot.gpu, snapshot.cpu, snapshot.memory
        );
        if filter.show(crate::MetricId::CmdGpu) {
            if let Some(cmd) = snapshot.command_gpu {
                eprint!("  cmd-gpu {:5.1}%", cmd);
            }
        }
        if filter.show(crate::MetricId::GpuPwr) {
            if let Some(w) = snapshot.gpu_power_w {
                eprint!("  gpu-pwr {:.0}W", w);
            }
        }
        eprintln!();
    }
}
