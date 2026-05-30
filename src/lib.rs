mod cf_utils;
mod color;
mod format;
mod gpuexercise;
mod iokit;
mod ioreport;
mod metrics;
mod output;
mod platform;
mod runner;

pub use color::{parse_color_scheme, parse_color_when, ColorScheme, ColorWhen, Colors};
pub use format::{summarize_line, FormatContext, DEFAULT_FORMAT};
pub use metrics::PercentStats;
pub use output::print_report;
pub use platform::check_apple_silicon;
pub use runner::{run_command, wait_status_to_exit_code, RunResult};

use clap::{Arg, ArgAction, Command};
use std::time::Duration;

fn parse_color_setting(value: &str, setting: &str) -> Result<ColorWhen, i32> {
    parse_color_when(value).map_err(|e| {
        eprintln!("gpucap: {e}");
        eprintln!("gpucap: try `gpucap --help` for valid {setting} values");
        2
    })
}

fn parse_scheme_setting(value: &str) -> Result<ColorScheme, i32> {
    parse_color_scheme(value).map_err(|e| {
        eprintln!("gpucap: {e}");
        eprintln!("gpucap: try `gpucap --help` for valid --color-scheme values");
        2
    })
}

pub(crate) fn resolve_color_settings(matches: &clap::ArgMatches) -> Result<Colors, i32> {
    let color_when = if matches.get_flag("no_color") {
        ColorWhen::Never
    } else if let Some(value) = matches.get_one::<String>("color") {
        parse_color_setting(value, "--color values")?
    } else if let Ok(value) = std::env::var("GPUCAP_COLOR") {
        parse_color_setting(&value, "GPUCAP_COLOR values")?
    } else {
        ColorWhen::Auto
    };

    let scheme = if let Some(value) = matches.get_one::<String>("color_scheme") {
        parse_scheme_setting(value)?
    } else if let Ok(value) = std::env::var("GPUCAP_COLOR_SCHEME") {
        parse_scheme_setting(&value)?
    } else {
        ColorScheme::Default
    };

    Ok(Colors::resolve(color_when, scheme))
}

pub fn run(args: &[String]) -> i32 {
    if let Err(code) = platform::ensure_apple_silicon() {
        return code;
    }

    if args.len() > 1 && args[1] == "gpuexercise" {
        return gpuexercise::run(args);
    }

    run_capture(args)
}

fn run_capture(args: &[String]) -> i32 {
    let cmd = Command::new("gpucap")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Run a command and report GPU, CPU, and unified memory usage on Apple Silicon")
        .long_about(
            "Run a command and report GPU, CPU, and unified memory usage.\n\n\
             Examples:\n  \
               gpucap sleep 1\n  \
               gpucap -f '%gA,%uA,%e,%Ws,%Wt' sleep 1\n  \
               gpucap --color=bright -- ffmpeg -i in.mp4 out.mp4\n  \
               gpucap gpuexercise --percent 60 --seconds 5",
        )
        .arg(
            Arg::new("format")
                .short('f')
                .long("format")
                .value_name("FORMAT")
                .help("Print statistics using a FORMAT string (plain text; see --help)")
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
        .arg(
            Arg::new("command")
                .help("Command and arguments to run")
                .num_args(1..)
                .required(true)
                .trailing_var_arg(true),
        );

    let matches = match cmd.try_get_matches_from(args) {
        Ok(m) => m,
        Err(e) => {
            let _ = e.print();
            return e.exit_code();
        }
    };

    let colors = match resolve_color_settings(&matches) {
        Ok(colors) => colors,
        Err(code) => return code,
    };

    let interval_ms: u64 = match matches.get_one::<String>("interval").unwrap().parse() {
        Ok(ms) if ms > 0 => ms,
        _ => {
            eprintln!("gpucap: interval must be a positive integer (milliseconds)");
            return 2;
        }
    };

    let cmd_args: Vec<&str> = matches
        .get_many::<String>("command")
        .unwrap()
        .map(|s| s.as_str())
        .collect();

    let result = match run_command(&cmd_args, Duration::from_millis(interval_ms)) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("gpucap: failed to run '{}': {}", cmd_args[0], e);
            return if e.kind() == std::io::ErrorKind::NotFound {
                127
            } else {
                126
            };
        }
    };

    let format = matches
        .get_one::<String>("format")
        .cloned()
        .or_else(|| std::env::var("GPUCAP_FORMAT").ok());

    if let Some(fmt) = format {
        let ctx = FormatContext {
            command: result.command.clone(),
            wait_status: result.wait_status,
            elapsed_secs: result.elapsed_secs,
            start_bd: result.start_bd,
            end_bd: result.end_bd,
            gpu: result.gpu,
            cpu: result.cpu,
            memory: result.memory,
            exercise_target: None,
        };
        if let Err(e) = summarize_line(&mut std::io::stderr(), &fmt, &ctx) {
            eprintln!("gpucap: {e}");
            return 1;
        }
    } else {
        print_report(&colors, &result);
    }

    wait_status_to_exit_code(result.wait_status)
}
