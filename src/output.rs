use crate::color::Colors;
use crate::metrics::{PercentStats, ScalarStats, ThrottleStats};
use crate::metrics_filter::{MetricFilter, MetricId};
use crate::platform::ChipProfile;
use crate::RunResult;

const LABEL_WIDTH: usize = 10;

#[derive(Debug, Clone)]
pub struct ReportStyle {
    /// Printed between the average value and the word `peak`.
    pub separator: String,
    /// Align average and peak values into columns across all metric rows.
    pub columns: bool,
    /// Which metrics to include in human-readable output.
    pub metrics: MetricFilter,
}

impl Default for ReportStyle {
    fn default() -> Self {
        Self {
            separator: " ".to_string(),
            columns: false,
            metrics: MetricFilter::all(),
        }
    }
}

#[derive(Clone, Copy)]
enum ValueKind {
    Percent,
    Scalar,
    Bytes,
}

struct ReportRow<'a> {
    name: &'a str,
    style: &'a str,
    qualifier: Option<&'a str>,
    kind: ValueKind,
    avg: f64,
    peak: f64,
    unit: Option<&'a str>,
}

pub fn print_report(
    colors: &Colors,
    profile: &ChipProfile,
    result: &RunResult,
    style: &ReportStyle,
) {
    eprintln!();
    print_report_body(colors, result, style);
    crate::platform::print_metrics_footnote(profile);
}

pub fn print_exercise_target(colors: &Colors, target: f64, seconds: f64) {
    if colors.enabled() {
        eprint!("{}", colors.label_width("target", colors.title, LABEL_WIDTH));
        eprint!("{}{:.0}%{reset}  ", colors.value, target, reset = colors.reset);
        eprint!("{}{}for ", colors.detail, colors.reset);
        eprintln!(
            "{}{:.1} {unit}s{reset}",
            colors.value,
            seconds,
            unit = colors.unit,
            reset = colors.reset,
        );
    } else {
        eprintln!("{:<width$}{target:.0}%  for {seconds:.1} s", "target", width = LABEL_WIDTH);
    }
}

pub fn print_exercise_report(
    colors: &Colors,
    profile: &ChipProfile,
    target: f64,
    seconds: f64,
    result: &RunResult,
    style: &ReportStyle,
) {
    eprintln!();
    print_exercise_target(colors, target, seconds);
    print_report_body(colors, result, style);
    crate::platform::print_metrics_footnote(profile);
}

fn print_report_body(colors: &Colors, result: &RunResult, style: &ReportStyle) {
    let filter = &style.metrics;
    let mut rows = Vec::new();
    let mut owned_labels: Vec<String> = Vec::new();

    if filter.show(MetricId::Gpu) {
        rows.push(percent_row(MetricId::Gpu, colors.gpu, &result.gpu));
    }
    if filter.show(MetricId::CmdGpu) && result.command_gpu.samples > 0 {
        let qual = result.tracked_gpu_pid.map(|pid| {
            let s = format!("pid {pid}");
            owned_labels.push(s);
            owned_labels.last().unwrap().as_str()
        });
        let name = if result.tracked_gpu_pid.is_some() {
            "gpu-pid"
        } else {
            "cmd-gpu"
        };
        rows.push(percent_row_named(
            name,
            colors.gpu,
            qual,
            &result.command_gpu,
        ));
    }
    if filter.show(MetricId::Cpu) {
        rows.push(percent_row(MetricId::Cpu, colors.cpu, &result.cpu));
    }
    if filter.show(MetricId::Memory) {
        rows.push(percent_row(MetricId::Memory, colors.memory, &result.memory));
    }

    if filter.show(MetricId::GpuMem) && result.gpu_mem_in_use.samples > 0 {
        rows.push(bytes_row(
            MetricId::GpuMem,
            colors.gpu,
            Some("in use"),
            &result.gpu_mem_in_use,
        ));
    }
    if filter.show(MetricId::Renderer) && result.gpu_renderer.samples > 0 {
        rows.push(percent_row(
            MetricId::Renderer,
            colors.gpu,
            &result.gpu_renderer,
        ));
    }
    if filter.show(MetricId::Tiler) && result.gpu_tiler.samples > 0 {
        rows.push(percent_row(MetricId::Tiler, colors.gpu, &result.gpu_tiler));
    }
    if filter.show(MetricId::GpuMhz) && result.gpu_freq_mhz.samples > 0 {
        rows.push(scalar_row(
            MetricId::GpuMhz,
            colors.gpu,
            &result.gpu_freq_mhz,
            "MHz",
        ));
    }
    if filter.show(MetricId::CpuMhz) && result.cpu_freq_mhz.samples > 0 {
        rows.push(scalar_row(
            MetricId::CpuMhz,
            colors.cpu,
            &result.cpu_freq_mhz,
            "MHz",
        ));
    }
    if filter.show(MetricId::EMhz) && result.ecpu_freq_mhz.samples > 0 {
        rows.push(scalar_row(
            MetricId::EMhz,
            colors.cpu,
            &result.ecpu_freq_mhz,
            "MHz",
        ));
    }
    if filter.show(MetricId::PMhz) && result.pcpu_freq_mhz.samples > 0 {
        rows.push(scalar_row(
            MetricId::PMhz,
            colors.cpu,
            &result.pcpu_freq_mhz,
            "MHz",
        ));
    }
    if filter.show(MetricId::GpuPwr) && result.gpu_power_w.samples > 0 {
        rows.push(scalar_row(
            MetricId::GpuPwr,
            colors.gpu,
            &result.gpu_power_w,
            "W",
        ));
    }
    if filter.show(MetricId::SramPwr) && result.gpu_sram_power_w.samples > 0 {
        rows.push(scalar_row(
            MetricId::SramPwr,
            colors.gpu,
            &result.gpu_sram_power_w,
            "W",
        ));
    }
    if filter.show(MetricId::CpuPwr) && result.cpu_power_w.samples > 0 {
        rows.push(scalar_row(
            MetricId::CpuPwr,
            colors.cpu,
            &result.cpu_power_w,
            "W",
        ));
    }
    if filter.show(MetricId::DramPwr) && result.dram_power_w.samples > 0 {
        rows.push(scalar_row(
            MetricId::DramPwr,
            colors.memory,
            &result.dram_power_w,
            "W",
        ));
    }
    if filter.show(MetricId::AnePwr) && result.ane_power_w.samples > 0 {
        rows.push(scalar_row(
            MetricId::AnePwr,
            colors.gpu,
            &result.ane_power_w,
            "W",
        ));
    }
    if filter.show(MetricId::EPwr) && result.ecpu_power_w.samples > 0 {
        rows.push(scalar_row(
            MetricId::EPwr,
            colors.cpu,
            &result.ecpu_power_w,
            "W",
        ));
    }
    if filter.show(MetricId::PPwr) && result.pcpu_power_w.samples > 0 {
        rows.push(scalar_row(
            MetricId::PPwr,
            colors.cpu,
            &result.pcpu_power_w,
            "W",
        ));
    }
    if filter.show(MetricId::GpuTemp) && result.gpu_temp_c.samples > 0 {
        rows.push(scalar_row(
            MetricId::GpuTemp,
            colors.gpu,
            &result.gpu_temp_c,
            "°C",
        ));
    }
    if filter.show(MetricId::Wired) && result.mem_wired.samples > 0 {
        rows.push(bytes_row(
            MetricId::Wired,
            colors.memory,
            None,
            &result.mem_wired,
        ));
    }
    if filter.show(MetricId::Compress)
        && result.mem_compressed.samples > 0
        && result.mem_compressed.peak > 0.0
    {
        rows.push(bytes_row(
            MetricId::Compress,
            colors.memory,
            None,
            &result.mem_compressed,
        ));
    }
    if filter.show(MetricId::Swap)
        && result.mem_swap.samples > 0
        && result.mem_swap.peak > 0.0
    {
        rows.push(bytes_row(
            MetricId::Swap,
            colors.memory,
            None,
            &result.mem_swap,
        ));
    }

    render_metric_rows(colors, &rows, style);

    if filter.show(MetricId::Throttle)
        && result.gpu_throttle.samples > 0
        && result.gpu_throttle.throttled_samples > 0
    {
        print_throttle(colors, &result.gpu_throttle);
    }
    if filter.show(MetricId::Pressure) && result.mem_pressure.peak > 0.0 {
        print_pressure(colors, &result.mem_pressure, style);
    }

    print_real(colors, result.elapsed_secs);
}

fn percent_row<'a>(id: MetricId, style: &'a str, stats: &'a PercentStats) -> ReportRow<'a> {
    percent_row_named(id.name(), style, None, stats)
}

fn percent_row_named<'a>(
    name: &'a str,
    style: &'a str,
    qualifier: Option<&'a str>,
    stats: &'a PercentStats,
) -> ReportRow<'a> {
    ReportRow {
        name,
        style,
        qualifier,
        kind: ValueKind::Percent,
        avg: stats.avg,
        peak: stats.peak,
        unit: None,
    }
}

fn scalar_row<'a>(
    id: MetricId,
    style: &'a str,
    stats: &'a ScalarStats,
    unit: &'a str,
) -> ReportRow<'a> {
    ReportRow {
        name: id.name(),
        style,
        qualifier: None,
        kind: ValueKind::Scalar,
        avg: stats.avg,
        peak: stats.peak,
        unit: Some(unit),
    }
}

fn bytes_row<'a>(
    id: MetricId,
    style: &'a str,
    qualifier: Option<&'a str>,
    stats: &'a ScalarStats,
) -> ReportRow<'a> {
    ReportRow {
        name: id.name(),
        style,
        qualifier,
        kind: ValueKind::Bytes,
        avg: stats.avg,
        peak: stats.peak,
        unit: None,
    }
}

fn render_metric_rows(colors: &Colors, rows: &[ReportRow<'_>], style: &ReportStyle) {
    if rows.is_empty() {
        return;
    }

    let (max_avg_w, max_peak_w) = if style.columns {
        rows.iter().fold((0usize, 0usize), |(max_a, max_p), row| {
            (
                max_a.max(plain_value(row, row.avg).len()),
                max_p.max(plain_value(row, row.peak).len()),
            )
        })
    } else {
        (0, 0)
    };

    for row in rows {
        render_metric_row(colors, row, style, max_avg_w, max_peak_w);
    }
}

fn render_metric_row(
    colors: &Colors,
    row: &ReportRow<'_>,
    style: &ReportStyle,
    max_avg_w: usize,
    max_peak_w: usize,
) {
    let avg_plain = plain_value(row, row.avg);
    let peak_plain = plain_value(row, row.peak);
    let avg_col = pad_plain(&avg_plain, max_avg_w);
    let peak_col = pad_plain(&peak_plain, max_peak_w);

    if colors.enabled() {
        let avg_style = match row.kind {
            ValueKind::Percent => colors.pct_style(row.avg),
            ValueKind::Scalar | ValueKind::Bytes => colors.value,
        };
        let peak_style = match row.kind {
            ValueKind::Percent => colors.pct_style(row.peak),
            ValueKind::Scalar | ValueKind::Bytes => colors.value,
        };

        eprint!("{}", colors.label_width(row.name, row.style, LABEL_WIDTH));
        if let Some(q) = row.qualifier {
            eprint!("{}{q} ", colors.detail, q = q);
        }
        eprint!("{}{}avg ", colors.detail, colors.reset);
        eprint!("{avg_style}{avg_col}{reset}", reset = colors.reset);
        eprint!("{}{}{}peak ", style.separator, colors.detail, colors.reset);
        eprintln!("{peak_style}{peak_col}{reset}", reset = colors.reset);
    } else {
        let mut line = format!("{:<width$}", row.name, width = LABEL_WIDTH);
        if let Some(q) = row.qualifier {
            line.push(' ');
            line.push_str(q);
        }
        line.push_str(" avg ");
        line.push_str(&avg_col);
        line.push_str(&style.separator);
        line.push_str("peak ");
        line.push_str(&peak_col);
        eprintln!("{line}");
    }
}

fn plain_value(row: &ReportRow<'_>, value: f64) -> String {
    match row.kind {
        ValueKind::Percent => format!("{value:.1}%"),
        ValueKind::Scalar => format!("{value:.0} {}", row.unit.unwrap_or("")),
        ValueKind::Bytes => format_bytes(value),
    }
}

fn pad_plain(text: &str, width: usize) -> String {
    if width == 0 || text.len() >= width {
        return text.to_string();
    }
    format!("{text}{}", " ".repeat(width - text.len()))
}

fn print_throttle(colors: &Colors, stats: &ThrottleStats) {
    let pct = stats.pct();
    if colors.enabled() {
        eprint!("{}", colors.label_width("throttle", colors.gpu, LABEL_WIDTH));
        eprintln!(
            "{}{pct:.0}% of samples{reset}",
            colors.value,
            pct = pct,
            reset = colors.reset,
        );
    } else {
        eprintln!("throttle   {pct:.0}% of samples");
    }
}

fn print_pressure(colors: &Colors, stats: &ScalarStats, style: &ReportStyle) {
    let avg = pressure_label(stats.avg);
    let peak = pressure_label(stats.peak);
    if colors.enabled() {
        eprint!("{}", colors.label_width("pressure", colors.memory, LABEL_WIDTH));
        eprintln!(
            "{}{avg} avg{sep}peak {peak}{reset}",
            colors.value,
            avg = avg,
            peak = peak,
            sep = style.separator,
            reset = colors.reset,
        );
    } else {
        eprintln!(
            "pressure   {avg} avg{sep}peak {peak}",
            sep = style.separator,
        );
    }
}

fn pressure_label(level: f64) -> &'static str {
    match level.round() as u8 {
        2 => "critical",
        1 => "warn",
        _ => "normal",
    }
}

fn format_bytes(bytes: f64) -> String {
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    if bytes >= GIB {
        format!("{:.1} GiB", bytes / GIB)
    } else if bytes >= MIB {
        format!("{:.0} MiB", bytes / MIB)
    } else if bytes > 0.0 {
        format!("{:.0} B", bytes)
    } else {
        "0 B".to_string()
    }
}

fn print_real(colors: &Colors, elapsed_secs: f64) {
    if colors.enabled() {
        eprintln!(
            "{} {}{:.6}{} {}{}{}",
            colors.label_width("real", colors.real, LABEL_WIDTH),
            colors.value,
            elapsed_secs,
            colors.reset,
            colors.unit,
            "s",
            colors.reset,
        );
    } else {
        eprintln!("real       {elapsed_secs:.6} s");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separator_inserts_between_avg_and_peak() {
        let style = ReportStyle {
            separator: " / ".to_string(),
            columns: false,
            metrics: MetricFilter::all(),
        };
        let row = percent_row(MetricId::Gpu, "", &PercentStats {
            avg: 55.1,
            peak: 100.0,
            samples: 1,
        });
        let mut buf = Vec::new();
        // Capture via plain (no color) path
        let avg_plain = plain_value(&row, row.avg);
        let peak_plain = plain_value(&row, row.peak);
        let line = format!(
            "{:<10} avg {avg_plain}{sep}peak {peak_plain}",
            row.name,
            sep = style.separator,
        );
        writeln_plain(&mut buf, &line).unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(text.contains("55.1% / peak 100.0%"));
    }

    #[test]
    fn columns_pad_values_to_common_width() {
        let rows = [
            percent_row(MetricId::Gpu, "", &PercentStats {
                avg: 5.0,
                peak: 10.0,
                samples: 1,
            }),
            percent_row(MetricId::Memory, "", &PercentStats {
                avg: 62.3,
                peak: 62.4,
                samples: 1,
            }),
        ];
        let max_a = rows
            .iter()
            .map(|row| plain_value(row, row.avg).len())
            .max()
            .unwrap_or(0);
        assert_eq!(max_a, 5);
        assert_eq!(pad_plain("5.0%", max_a), "5.0% ");
        assert_eq!(pad_plain("62.3%", max_a), "62.3%");
    }

    fn writeln_plain(buf: &mut Vec<u8>, line: &str) -> std::io::Result<()> {
        use std::io::Write;
        writeln!(buf, "{line}")
    }
}
