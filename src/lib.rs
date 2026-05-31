mod cf_utils;
mod color;
mod compare;
mod format;
mod gpuexercise;
mod gpu_proc;
mod iokit;
mod ioreport;
mod json;
mod metrics;
mod metrics_filter;
mod output;
mod platform;
mod pmgr;
mod runner;
mod watch;

pub use color::{parse_color_scheme, parse_color_when, ColorScheme, ColorWhen, Colors};
pub use format::{format_needs_extended, summarize_line, FormatContext, DEFAULT_FORMAT};
pub use metrics::{PercentStats, SampleTier, ScalarStats, ThrottleStats};
pub use metrics_filter::{MetricFilter, MetricId, METRICS_HELP};
pub use output::{print_report, ReportStyle};
pub use platform::{
    check_apple_silicon, print_metrics_footnote, ChipFamily, ChipProfile, METRICS_FOOTNOTE,
};
pub use runner::{run_command, wait_status_to_exit_code, GpuTrackPid, RunOptions, RunResult};

use clap::{Arg, ArgAction, ArgMatches, Command};
use std::time::Duration;

fn parse_color_setting(value: &str, setting: &str) -> Result<ColorWhen, i32> {
    parse_color_when(value).map_err(|e| {
        eprintln!("bgpucap: {e}");
        eprintln!("bgpucap: try `bgpucap --help` for valid {setting} values");
        2
    })
}

fn parse_scheme_setting(value: &str) -> Result<ColorScheme, i32> {
    parse_color_scheme(value).map_err(|e| {
        eprintln!("bgpucap: {e}");
        eprintln!("bgpucap: try `bgpucap --help` for valid --color-scheme values");
        2
    })
}

pub(crate) fn resolve_color_settings(matches: &clap::ArgMatches) -> Result<Colors, i32> {
    let color_when = if matches.get_flag("no_color") {
        ColorWhen::Never
    } else if let Some(value) = matches.get_one::<String>("color") {
        parse_color_setting(value, "--color values")?
    } else if let Ok(value) = std::env::var("BGPUCAP_COLOR")
        .or_else(|_| std::env::var("GPUCAP_COLOR"))
    {
        parse_color_setting(&value, "BGPUCAP_COLOR values")?
    } else {
        ColorWhen::Auto
    };

    let scheme = if let Some(value) = matches.get_one::<String>("color_scheme") {
        parse_scheme_setting(value)?
    } else if let Ok(value) = std::env::var("BGPUCAP_COLOR_SCHEME")
        .or_else(|_| std::env::var("GPUCAP_COLOR_SCHEME"))
    {
        parse_scheme_setting(&value)?
    } else {
        ColorScheme::Default
    };

    Ok(Colors::resolve(color_when, scheme))
}

pub(crate) fn output_style_args() -> [Arg; 3] {
    [
        Arg::new("separator")
            .long("separator")
            .value_name("TEXT")
            .help("Text between each average value and the word \"peak\" (default: space)")
            .default_value(" "),
        Arg::new("columns")
            .long("columns")
            .help("Align average and peak values in columns")
            .action(ArgAction::SetTrue),
        Arg::new("metrics")
            .long("metrics")
            .value_name("LIST")
            .help(METRICS_HELP),
    ]
}

pub(crate) fn common_args() -> [Arg; 3] {
    [
        Arg::new("list_metrics")
            .long("list-metrics")
            .help("List metric names and groups for --metrics")
            .action(ArgAction::SetTrue),
        Arg::new("pid")
            .long("pid")
            .value_name("PID")
            .help("Track GPU usage for this process ID (default: wrapped child when cmd-gpu is sampled)"),
        Arg::new("no_track_gpu")
            .long("no-track-gpu")
            .help("Do not track per-process GPU usage (IORegistry)")
            .action(ArgAction::SetTrue)
            .conflicts_with("pid"),
    ]
}

pub(crate) fn resolve_report_style(matches: &ArgMatches) -> Result<ReportStyle, i32> {
    let separator = matches
        .get_one::<String>("separator")
        .cloned()
        .or_else(|| std::env::var("BGPUCAP_SEPARATOR").ok())
        .unwrap_or_else(|| " ".to_string());
    let metrics = resolve_metric_filter(matches)?;
    Ok(ReportStyle {
        separator,
        columns: matches.get_flag("columns"),
        metrics,
    })
}

fn resolve_metric_filter(matches: &ArgMatches) -> Result<MetricFilter, i32> {
    if let Some(value) = matches.get_one::<String>("metrics") {
        return parse_metric_filter(value);
    }
    if let Ok(value) = std::env::var("BGPUCAP_METRICS") {
        return parse_metric_filter(&value);
    }
    Ok(MetricFilter::all())
}

fn parse_metric_filter(value: &str) -> Result<MetricFilter, i32> {
    MetricFilter::parse_list(value).map_err(|e| {
        eprintln!("bgpucap: {e}");
        eprintln!("bgpucap: try `bgpucap --list-metrics` or `bgpucap --help`");
        2
    })
}

pub(crate) fn resolve_sample_tier(
    metrics: &MetricFilter,
    format: Option<&str>,
) -> SampleTier {
    if let Some(fmt) = format {
        if json::is_json_format(fmt) {
            return if metrics.needs_extended_sampling() {
                SampleTier::Full
            } else {
                SampleTier::Basic
            };
        }
        if format_needs_extended(fmt) {
            return SampleTier::Full;
        }
    }
    if metrics.needs_extended_sampling() {
        SampleTier::Full
    } else {
        SampleTier::Basic
    }
}

pub(crate) fn resolve_gpu_track(matches: &ArgMatches, metrics: &MetricFilter) -> GpuTrackPid {
    if matches.get_flag("no_track_gpu") {
        return GpuTrackPid::Disabled;
    }
    if let Some(value) = matches.get_one::<String>("pid") {
        if let Ok(pid) = value.parse::<i32>() {
            if pid > 0 {
                return GpuTrackPid::Pid(pid);
            }
        }
        eprintln!("bgpucap: --pid must be a positive integer");
        std::process::exit(2);
    }
    if metrics.needs_cmd_gpu() {
        GpuTrackPid::Child
    } else {
        GpuTrackPid::Disabled
    }
}

pub(crate) fn resolve_format(matches: &ArgMatches) -> Option<String> {
    if let Some(f) = matches.get_one::<String>("format") {
        return Some(f.clone());
    }
    std::env::var("BGPUCAP_FORMAT")
        .or_else(|_| std::env::var("GPUCAP_FORMAT"))
        .ok()
}

pub fn run(args: &[String]) -> i32 {
    if args.iter().any(|a| a == "--list-metrics") {
        MetricFilter::print_list();
        return 0;
    }

    if args.len() > 1 {
        match args[1].as_str() {
            "gpuexercise" => {
                if let Err(code) = platform::ensure_apple_silicon() {
                    return code;
                }
                return gpuexercise::run(args);
            }
            "watch" => return watch::run(args),
            "compare" => return compare::run(args),
            _ => {}
        }
    }

    run_capture(args)
}

fn run_capture(args: &[String]) -> i32 {
    if let Err(code) = platform::ensure_apple_silicon() {
        return code;
    }

    let cmd = Command::new("bgpucap")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Run a command and report GPU, CPU, and unified memory usage on Apple Silicon")
        .long_about(
            "Run a command and report GPU, CPU, and unified memory usage.\n\n\
             Examples:\n  \
               bgpucap sleep 1\n  \
               bgpucap --metrics basic sleep 1\n  \
               bgpucap -f json sleep 1\n  \
               bgpucap -f '%gA,%uA,%e,%Ws,%Wt' sleep 1\n  \
               bgpucap --pid 1234 --metrics cmd-gpu sleep 1\n  \
               bgpucap gpuexercise --percent 60 --seconds 5",
        )
        .arg(
            Arg::new("format")
                .short('f')
                .long("format")
                .value_name("FORMAT")
                .help("Print statistics using FORMAT (specifiers, or 'json' for JSON output)")
        )
        .arg(
            Arg::new("interval")
                .long("interval")
                .value_name("MS")
                .help("Sampling interval in milliseconds")
                .default_value("100"),
        )
        .arg(
            Arg::new("color")
                .long("color")
                .value_name("WHEN")
                .num_args(0..=1)
                .default_missing_value("always")
                .help("Colorize output: auto, always, never, plain, ansi, or truecolor")
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new("no_color")
                .long("no-color")
                .help("Disable color output")
                .action(ArgAction::SetTrue)
                .conflicts_with("color"),
        )
        .arg(
            Arg::new("color_scheme")
                .long("color-scheme")
                .value_name("SCHEME")
                .help("Color palette: default or bright")
                .default_value("default"),
        )
        .args(output_style_args())
        .args(common_args())
        .arg(
            Arg::new("command")
                .help("Command and arguments to run")
                .num_args(1..)
                .trailing_var_arg(true),
        );

    let matches = match cmd.try_get_matches_from(args) {
        Ok(m) => m,
        Err(e) => {
            let _ = e.print();
            return e.exit_code();
        }
    };

    let cmd_args: Vec<&str> = matches
        .get_many::<String>("command")
        .map(|vals| vals.map(|s| s.as_str()).collect())
        .unwrap_or_default();

    if cmd_args.is_empty() {
        eprintln!("bgpucap: missing command (try `bgpucap --help`)");
        return 2;
    }

    let colors = match resolve_color_settings(&matches) {
        Ok(colors) => colors,
        Err(code) => return code,
    };

    let interval_ms: u64 = match matches.get_one::<String>("interval").unwrap().parse() {
        Ok(ms) if ms > 0 => ms,
        _ => {
            eprintln!("bgpucap: interval must be a positive integer (milliseconds)");
            return 2;
        }
    };

    let chip = platform::ChipProfile::detect();
    let style = match resolve_report_style(&matches) {
        Ok(style) => style,
        Err(code) => return code,
    };

    let format = resolve_format(&matches);
    let tier = resolve_sample_tier(&style.metrics, format.as_deref());
    let gpu_track = resolve_gpu_track(&matches, &style.metrics);

    let result = match run_command(
        &cmd_args,
        RunOptions {
            interval: Duration::from_millis(interval_ms),
            tier,
            gpu_track,
        },
    ) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("bgpucap: failed to run '{}': {}", cmd_args[0], e);
            return if e.kind() == std::io::ErrorKind::NotFound {
                127
            } else {
                126
            };
        }
    };

    if let Some(fmt) = format {
        if json::is_json_format(&fmt) {
            if let Err(e) = json::write_result(
                &mut std::io::stdout(),
                &result,
                &chip,
                &style.metrics,
                None,
            ) {
                eprintln!("bgpucap: {e}");
                return 1;
            }
        } else {
            let ctx = FormatContext::from_run(&result, None);
            if let Err(e) = summarize_line(&mut std::io::stderr(), &fmt, &ctx) {
                eprintln!("bgpucap: {e}");
                return 1;
            }
        }
    } else {
        print_report(&colors, &chip, &result, &style);
    }

    wait_status_to_exit_code(result.wait_status)
}
