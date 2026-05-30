use crate::color::Colors;
use crate::metrics::PercentStats;
use crate::RunResult;

pub fn print_report(colors: &Colors, result: &RunResult) {
    eprintln!();
    print_metric(colors, "gpu", colors.gpu, &result.gpu);
    print_metric(colors, "cpu", colors.cpu, &result.cpu);
    print_metric(colors, "memory", colors.memory, &result.memory);
    print_real(colors, result.elapsed_secs);
}

fn print_metric(colors: &Colors, name: &str, label_style: &str, stats: &PercentStats) {
    if colors.enabled() {
        let avg_style = colors.pct_style(stats.avg);
        let peak_style = colors.pct_style(stats.peak);
        eprint!("{}", colors.label(name, label_style));
        eprint!("{}{}avg ", colors.detail, colors.reset);
        eprint!("{avg_style}{avg:.1}%{reset}", avg = stats.avg, reset = colors.reset);
        eprint!("{}{}peak ", colors.detail, colors.reset);
        eprintln!(
            "{peak_style}{peak:.1}%{reset}",
            peak = stats.peak,
            reset = colors.reset
        );
    } else {
        eprintln!(
            "{name:<8} avg {avg:.1}%  peak {peak:.1}%",
            avg = stats.avg,
            peak = stats.peak,
        );
    }
}

fn print_real(colors: &Colors, elapsed_secs: f64) {
    if colors.enabled() {
        eprintln!(
            "{} {}{:.6}{} {}{}{}",
            colors.label("real", colors.real),
            colors.value,
            elapsed_secs,
            colors.reset,
            colors.unit,
            "s",
            colors.reset,
        );
    } else {
        eprintln!("real     {elapsed_secs:.6} s");
    }
}

pub fn print_exercise_report(
    colors: &Colors,
    target: f64,
    seconds: f64,
    stats: &PercentStats,
) {
    eprintln!();
    if colors.enabled() {
        eprint!("{}", colors.label("target", colors.title));
        eprint!("{}{:.0}%{reset}  ", colors.value, target, reset = colors.reset);
        eprint!("{}{}for ", colors.detail, colors.reset);
        eprintln!(
            "{}{:.1} {unit}s{reset}",
            colors.value,
            seconds,
            unit = colors.unit,
            reset = colors.reset
        );
    } else {
        eprintln!("target   {target:.0}%  for {seconds:.1} s");
    }
    print_metric(colors, "gpu", colors.gpu, stats);
}
