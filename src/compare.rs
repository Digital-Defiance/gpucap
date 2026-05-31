use crate::json::ParsedReport;
use clap::Command;
use std::io::{self, Write};

pub fn run(args: &[String]) -> i32 {
    let cmd = Command::new("compare")
        .about("Compare metrics from two bgpucap JSON reports")
        .arg(
            clap::Arg::new("left")
                .value_name("FILE")
                .required(true)
                .help("First JSON report (e.g. before.json)"),
        )
        .arg(
            clap::Arg::new("right")
                .value_name("FILE")
                .required(true)
                .help("Second JSON report (e.g. after.json)"),
        );

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

    let left_path = matches.get_one::<String>("left").unwrap();
    let right_path = matches.get_one::<String>("right").unwrap();

    let left = match read_report(left_path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("bgpucap compare: {left_path}: {e}");
            return 2;
        }
    };
    let right = match read_report(right_path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("bgpucap compare: {right_path}: {e}");
            return 2;
        }
    };

    if let Err(e) = print_diff(&left, &right, left_path, right_path) {
        eprintln!("bgpucap compare: {e}");
        return 1;
    }
    0
}

fn read_report(path: &str) -> Result<ParsedReport, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    ParsedReport::parse(&text)
}

fn print_diff(
    left: &ParsedReport,
    right: &ParsedReport,
    left_label: &str,
    right_label: &str,
) -> io::Result<()> {
    let names = ParsedReport::common_metric_names(left, right);

    if names.is_empty() {
        if left.metrics.is_empty() || right.metrics.is_empty() {
            writeln!(io::stdout(), "bgpucap compare: no metrics found in either file")?;
        } else {
            writeln!(
                io::stdout(),
                "bgpucap compare: no common metrics between reports"
            )?;
        }
        return Ok(());
    }

    let only_left: Vec<&str> = left
        .metric_names()
        .into_iter()
        .filter(|name| !right.metrics.contains_key(*name))
        .collect();
    let only_right: Vec<&str> = right
        .metric_names()
        .into_iter()
        .filter(|name| !left.metrics.contains_key(*name))
        .collect();
    if !only_left.is_empty() {
        eprintln!(
            "bgpucap compare: skipping metrics only in left: {}",
            only_left.join(", ")
        );
    }
    if !only_right.is_empty() {
        eprintln!(
            "bgpucap compare: skipping metrics only in right: {}",
            only_right.join(", ")
        );
    }

    writeln!(io::stdout(), "compare  {left_label}  vs  {right_label}")?;
    if !left.command.is_empty() || !right.command.is_empty() {
        writeln!(
            io::stdout(),
            "  command: {}  |  {}",
            empty_dash(&left.command),
            empty_dash(&right.command)
        )?;
    }
    writeln!(
        io::stdout(),
        "  elapsed: {:.3}s  |  {:.3}s",
        left.elapsed_secs, right.elapsed_secs
    )?;
    writeln!(io::stdout())?;
    writeln!(
        io::stdout(),
        "{:<22} {:>10} {:>10} {:>10}",
        "metric", "left avg", "right avg", "delta"
    )?;

    for name in names {
        let lm = &left.metrics[name];
        let rm = &right.metrics[name];
        let delta = rm.avg - lm.avg;
        writeln!(
            io::stdout(),
            "{:<22} {:>10.1} {:>10.1} {:>+10.1}",
            name, lm.avg, rm.avg, delta
        )?;
    }
    Ok(())
}

fn empty_dash(s: &str) -> &str {
    if s.is_empty() { "-" } else { s }
}
