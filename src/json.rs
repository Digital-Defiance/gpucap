use crate::metrics::{MetricSnapshot, PercentStats, ScalarStats, ThrottleStats};

/// Version field in ``-f json`` command reports and embed snapshot JSON.
pub const REPORT_SCHEMA: &str = "1";
use crate::metrics_filter::{MetricFilter, MetricId};
use crate::platform::ChipProfile;
use crate::runner::wait_status_to_exit_code;
use crate::RunResult;
use std::collections::HashMap;
use std::io::{self, Write};

pub fn is_json_format(fmt: &str) -> bool {
    fmt.trim().eq_ignore_ascii_case("json")
}

#[derive(Debug, Clone)]
pub struct ParsedMetric {
    pub avg: f64,
    pub peak: f64,
    pub samples: u32,
}

#[derive(Debug, Clone)]
pub struct ParsedReport {
    pub command: String,
    pub elapsed_secs: f64,
    pub metrics: HashMap<String, ParsedMetric>,
}

impl ParsedReport {
    pub fn parse(text: &str) -> Result<Self, String> {
        let command = extract_json_string(text, "command").unwrap_or_default();
        let elapsed_secs = extract_json_number(text, "elapsed_secs").unwrap_or(0.0);
        let metrics = extract_metrics_block(text)?;
        if metrics.is_empty() {
            return Err("no metrics object found".into());
        }
        Ok(Self {
            command,
            elapsed_secs,
            metrics,
        })
    }

    pub fn metric_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.metrics.keys().map(String::as_str).collect();
        names.sort_unstable();
        names
    }

    /// Metric keys present in both reports (sorted).
    pub fn common_metric_names<'a>(
        left: &'a ParsedReport,
        right: &'a ParsedReport,
    ) -> Vec<&'a str> {
        left.metric_names()
            .into_iter()
            .filter(|name| right.metrics.contains_key(*name))
            .collect()
    }
}

/// One NDJSON sample line for `bgpucap watch -f json`.
pub fn write_sample_line(
    out: &mut dyn Write,
    filter: &MetricFilter,
    snapshot: &MetricSnapshot,
    bd: f64,
) -> io::Result<()> {
    write!(out, "{{\"bd\":{}", json_num(bd))?;
    if filter.show(MetricId::Gpu) {
        write!(out, ",\"gpu\":{}", json_num(snapshot.gpu))?;
    }
    if filter.show(MetricId::Cpu) {
        write!(out, ",\"cpu\":{}", json_num(snapshot.cpu))?;
    }
    if filter.show(MetricId::Memory) {
        write!(out, ",\"memory\":{}", json_num(snapshot.memory))?;
    }
    if filter.show(MetricId::CmdGpu) {
        if let Some(v) = snapshot.command_gpu {
            write!(out, ",\"cmd_gpu\":{}", json_num(v))?;
        }
    }
    if filter.show(MetricId::GpuPwr) {
        if let Some(v) = snapshot.gpu_power_w {
            write!(out, ",\"gpu_power_w\":{}", json_num(v))?;
        }
    }
    writeln!(out, "}}")
}

/// Single ``sample_system`` reading (embed / watch); not a wrapped command summary.
pub fn write_snapshot_json(
    out: &mut dyn Write,
    snapshot: &MetricSnapshot,
    chip: &ChipProfile,
    filter: &MetricFilter,
) -> io::Result<()> {
    write!(out, "{{")?;
    write_str_field(out, "schema", REPORT_SCHEMA)?;
    write!(out, ",")?;
    write_str_field(out, "kind", "snapshot")?;
    write!(out, ",")?;
    write_chip(out, chip)?;
    write!(out, ",")?;
    write_snapshot_metrics(out, snapshot, filter)?;
    write!(out, "}}\n")
}

pub fn write_result(
    out: &mut dyn Write,
    result: &RunResult,
    chip: &ChipProfile,
    filter: &MetricFilter,
    exercise_target: Option<f64>,
) -> io::Result<()> {
    write!(out, "{{")?;
    write_str_field(out, "schema", REPORT_SCHEMA)?;
    write!(out, ",")?;
    write_str_field(out, "kind", "run")?;
    write!(out, ",")?;
    write_str_field(out, "command", &format_command(&result.command))?;
    write!(out, ",")?;
    write_i32_field(out, "exit_code", wait_status_to_exit_code(result.wait_status))?;
    write!(out, ",")?;
    write_f64_field(out, "elapsed_secs", result.elapsed_secs)?;
    write!(out, ",")?;
    write_f64_field(out, "start_bd", result.start_bd)?;
    write!(out, ",")?;
    write_f64_field(out, "end_bd", result.end_bd)?;
    if let Some(pid) = result.tracked_gpu_pid {
        write!(out, ",")?;
        write_i32_field(out, "tracked_gpu_pid", pid)?;
    }
    if let Some(target) = exercise_target {
        write!(out, ",")?;
        write_f64_field(out, "exercise_target", target)?;
    }
    write!(out, ",")?;
    write_chip(out, chip)?;
    write!(out, ",")?;
    write_metrics(out, result, filter)?;
    write!(out, "}}\n")
}

fn instant_percent(v: f64) -> PercentStats {
    PercentStats {
        avg: v,
        peak: v,
        samples: 1,
    }
}

fn instant_scalar(v: f64) -> ScalarStats {
    ScalarStats {
        avg: v,
        peak: v,
        samples: 1,
    }
}

fn write_snapshot_metrics(
    out: &mut dyn Write,
    snapshot: &MetricSnapshot,
    filter: &MetricFilter,
) -> io::Result<()> {
    write!(out, "\"metrics\":{{")?;
    let mut first = true;
    let mut emit = |out: &mut dyn Write, name: &str, body: &str| -> io::Result<()> {
        if !first {
            write!(out, ",")?;
        }
        first = false;
        write!(out, "\"{name}\":{body}")
    };

    if filter.show(MetricId::Gpu) {
        emit(out, "gpu", &percent_json(&instant_percent(snapshot.gpu)))?;
    }
    if filter.show(MetricId::Cpu) {
        emit(out, "cpu", &percent_json(&instant_percent(snapshot.cpu)))?;
    }
    if filter.show(MetricId::Memory) {
        emit(out, "memory", &percent_json(&instant_percent(snapshot.memory)))?;
    }
    if filter.show(MetricId::Pressure) {
        emit(
            out,
            "mem_pressure",
            &scalar_json(&instant_scalar(snapshot.mem_pressure as f64)),
        )?;
    }
    if filter.show(MetricId::Swap) && snapshot.mem_swap_bytes > 0 {
        emit(
            out,
            "mem_swap",
            &scalar_json(&instant_scalar(snapshot.mem_swap_bytes as f64)),
        )?;
    }
    if filter.show(MetricId::CmdGpu) {
        if let Some(v) = snapshot.command_gpu {
            emit(out, "cmd_gpu", &percent_json(&instant_percent(v)))?;
        }
    }
    write!(out, "}}")
}

fn write_chip(out: &mut dyn Write, chip: &ChipProfile) -> io::Result<()> {
    write!(out, "\"chip\":{{")?;
    write_str_field(out, "brand", &chip.brand)?;
    write!(out, ",")?;
    write_str_field(out, "family", chip.family.as_str())?;
    write!(out, ",")?;
    write_str_field(out, "validation_tier", chip.validation_tier())?;
    write!(out, ",")?;
    write_bool_field(out, "validated_metrics", chip.validated_metrics)?;
    write!(out, "}}")
}

fn write_metrics(out: &mut dyn Write, result: &RunResult, filter: &MetricFilter) -> io::Result<()> {
    write!(out, "\"metrics\":{{")?;
    let mut first = true;
    let mut emit = |out: &mut dyn Write, name: &str, body: &str| -> io::Result<()> {
        if !first {
            write!(out, ",")?;
        }
        first = false;
        write!(out, "\"{name}\":{body}")
    };

    if filter.show(MetricId::Gpu) {
        emit(out, "gpu", &percent_json(&result.gpu))?;
    }
    if filter.show(MetricId::CmdGpu) && result.command_gpu.samples > 0 {
        emit(out, "cmd_gpu", &percent_json(&result.command_gpu))?;
    }
    if filter.show(MetricId::Cpu) {
        emit(out, "cpu", &percent_json(&result.cpu))?;
    }
    if filter.show(MetricId::Memory) {
        emit(out, "memory", &percent_json(&result.memory))?;
    }
    if filter.show(MetricId::GpuMem) && result.gpu_mem_in_use.samples > 0 {
        emit(out, "gpu_mem_in_use", &scalar_json(&result.gpu_mem_in_use))?;
    }
    if filter.show(MetricId::GpuMem) && result.gpu_mem_allocated.samples > 0 {
        emit(out, "gpu_mem_allocated", &scalar_json(&result.gpu_mem_allocated))?;
    }
    if filter.show(MetricId::Renderer) && result.gpu_renderer.samples > 0 {
        emit(out, "renderer", &percent_json(&result.gpu_renderer))?;
    }
    if filter.show(MetricId::Tiler) && result.gpu_tiler.samples > 0 {
        emit(out, "tiler", &percent_json(&result.gpu_tiler))?;
    }
    if filter.show(MetricId::GpuMhz) && result.gpu_freq_mhz.samples > 0 {
        emit(out, "gpu_freq_mhz", &scalar_json(&result.gpu_freq_mhz))?;
    }
    if filter.show(MetricId::CpuMhz) && result.cpu_freq_mhz.samples > 0 {
        emit(out, "cpu_freq_mhz", &scalar_json(&result.cpu_freq_mhz))?;
    }
    if filter.show(MetricId::EMhz) && result.ecpu_freq_mhz.samples > 0 {
        emit(out, "ecpu_freq_mhz", &scalar_json(&result.ecpu_freq_mhz))?;
    }
    if filter.show(MetricId::PMhz) && result.pcpu_freq_mhz.samples > 0 {
        emit(out, "pcpu_freq_mhz", &scalar_json(&result.pcpu_freq_mhz))?;
    }
    if filter.show(MetricId::GpuPwr) && result.gpu_power_w.samples > 0 {
        emit(out, "gpu_power_w", &scalar_json(&result.gpu_power_w))?;
    }
    if filter.show(MetricId::SramPwr) && result.gpu_sram_power_w.samples > 0 {
        emit(out, "gpu_sram_power_w", &scalar_json(&result.gpu_sram_power_w))?;
    }
    if filter.show(MetricId::CpuPwr) && result.cpu_power_w.samples > 0 {
        emit(out, "cpu_power_w", &scalar_json(&result.cpu_power_w))?;
    }
    if filter.show(MetricId::DramPwr) && result.dram_power_w.samples > 0 {
        emit(out, "dram_power_w", &scalar_json(&result.dram_power_w))?;
    }
    if filter.show(MetricId::AnePwr) && result.ane_power_w.samples > 0 {
        emit(out, "ane_power_w", &scalar_json(&result.ane_power_w))?;
    }
    if filter.show(MetricId::EPwr) && result.ecpu_power_w.samples > 0 {
        emit(out, "ecpu_power_w", &scalar_json(&result.ecpu_power_w))?;
    }
    if filter.show(MetricId::PPwr) && result.pcpu_power_w.samples > 0 {
        emit(out, "pcpu_power_w", &scalar_json(&result.pcpu_power_w))?;
    }
    if filter.show(MetricId::GpuTemp) && result.gpu_temp_c.samples > 0 {
        emit(out, "gpu_temp_c", &scalar_json(&result.gpu_temp_c))?;
    }
    if filter.show(MetricId::Throttle) && result.gpu_throttle.samples > 0 {
        emit(out, "gpu_throttle", &throttle_json(&result.gpu_throttle))?;
    }
    if filter.show(MetricId::Wired) && result.mem_wired.samples > 0 {
        emit(out, "mem_wired", &scalar_json(&result.mem_wired))?;
    }
    if filter.show(MetricId::Compress) && result.mem_compressed.samples > 0 {
        emit(out, "mem_compressed", &scalar_json(&result.mem_compressed))?;
    }
    if filter.show(MetricId::Swap) && result.mem_swap.samples > 0 {
        emit(out, "mem_swap", &scalar_json(&result.mem_swap))?;
    }
    if filter.show(MetricId::Pressure) && result.mem_pressure.samples > 0 {
        emit(out, "mem_pressure", &scalar_json(&result.mem_pressure))?;
    }

    write!(out, "}}")
}

fn percent_json(s: &PercentStats) -> String {
    format!(
        "{{\"avg\":{},\"peak\":{},\"samples\":{}}}",
        json_num(s.avg),
        json_num(s.peak),
        s.samples
    )
}

fn scalar_json(s: &ScalarStats) -> String {
    format!(
        "{{\"avg\":{},\"peak\":{},\"samples\":{}}}",
        json_num(s.avg),
        json_num(s.peak),
        s.samples
    )
}

fn throttle_json(s: &ThrottleStats) -> String {
    format!(
        "{{\"pct\":{},\"throttled_samples\":{},\"samples\":{}}}",
        json_num(s.pct()),
        s.throttled_samples,
        s.samples
    )
}

fn json_num(v: f64) -> String {
    if v.is_finite() {
        format!("{v:.6}")
    } else {
        "null".to_string()
    }
}

fn format_command(command: &[String]) -> String {
    command.join(" ")
}

fn write_str_field(out: &mut dyn Write, key: &str, value: &str) -> io::Result<()> {
    write!(out, "\"{key}\":")?;
    write_json_string(out, value)
}

fn write_f64_field(out: &mut dyn Write, key: &str, value: f64) -> io::Result<()> {
    write!(out, "\"{key}\":{}", json_num(value))
}

fn write_i32_field(out: &mut dyn Write, key: &str, value: i32) -> io::Result<()> {
    write!(out, "\"{key}\":{value}")
}

fn write_bool_field(out: &mut dyn Write, key: &str, value: bool) -> io::Result<()> {
    write!(out, "\"{key}\":{}", if value { "true" } else { "false" })
}

fn write_json_string(out: &mut dyn Write, s: &str) -> io::Result<()> {
    write!(out, "\"")?;
    for ch in s.chars() {
        match ch {
            '\\' => write!(out, "\\\\")?,
            '"' => write!(out, "\\\"")?,
            '\n' => write!(out, "\\n")?,
            '\r' => write!(out, "\\r")?,
            '\t' => write!(out, "\\t")?,
            c if c.is_control() => write!(out, "\\u{:04x}", c as u32)?,
            c => write!(out, "{c}")?,
        }
    }
    write!(out, "\"")
}

fn extract_json_string(text: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\":\"");
    let start = text.find(&needle)? + needle.len();
    let rest = &text[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn extract_json_number(text: &str, key: &str) -> Option<f64> {
    let needle = format!("\"{key}\":");
    let start = text.find(&needle)? + needle.len();
    let rest = text[start..].trim_start();
    let end = rest
        .find(|c: char| c == ',' || c == '}' || c == '\n')
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

fn extract_metrics_block(text: &str) -> Result<HashMap<String, ParsedMetric>, String> {
    let start = text
        .find("\"metrics\":{")
        .ok_or("missing metrics object")?
        + "\"metrics\":{".len();
    let slice = &text[start..];
    let mut depth = 1usize;
    let mut end = slice.len();
    for (j, ch) in slice.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = j;
                    break;
                }
            }
            _ => {}
        }
    }
    let block = &slice[..end];

    let mut metrics = HashMap::new();
    let mut i = 0;
    let bytes = block.as_bytes();
    while i < bytes.len() {
        if bytes[i] != b'"' {
            i += 1;
            continue;
        }
        i += 1;
        let name_start = i;
        while i < bytes.len() && bytes[i] != b'"' {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let name = std::str::from_utf8(&bytes[name_start..i])
            .map_err(|e| e.to_string())?
            .to_string();
        i += 1;
        if i >= bytes.len() || bytes[i] != b':' {
            continue;
        }
        i += 1;
        if i >= bytes.len() || bytes[i] != b'{' {
            continue;
        }
        let obj_start = i;
        let obj_end = block[obj_start..]
            .find('}')
            .map(|p| obj_start + p)
            .unwrap_or(block.len());
        let obj = &block[obj_start..obj_end];
        if let (Some(avg), Some(peak)) = (
            extract_json_number(obj, "avg"),
            extract_json_number(obj, "peak"),
        ) {
            let samples = extract_json_number(obj, "samples")
                .map(|v| v as u32)
                .unwrap_or(1);
            metrics.insert(name, ParsedMetric { avg, peak, samples });
        }
        i = obj_end + 1;
    }
    Ok(metrics)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::{PercentStats, ScalarStats};

    #[test]
    fn json_format_is_detected() {
        assert!(is_json_format("json"));
        assert!(is_json_format(" JSON "));
        assert!(!is_json_format("%gA"));
    }

    #[test]
    fn parses_report_for_compare() {
        let sample = r#"{"command":"sleep 1","exit_code":0,"elapsed_secs":1.5,"metrics":{"gpu":{"avg":12.300000,"peak":45.600000,"samples":10},"cpu":{"avg":8.100000,"peak":23.400000,"samples":10}}}"#;
        let report = ParsedReport::parse(sample).unwrap();
        assert_eq!(report.command, "sleep 1");
        assert!((report.elapsed_secs - 1.5).abs() < f64::EPSILON);
        assert!((report.metrics["gpu"].avg - 12.3).abs() < f64::EPSILON);
        assert_eq!(report.metrics["gpu"].samples, 10);
    }

    #[test]
    fn common_metric_names_is_intersection() {
        let left = ParsedReport::parse(
            r#"{"command":"a","elapsed_secs":1,"metrics":{"gpu":{"avg":1,"peak":2,"samples":1},"cpu":{"avg":1,"peak":2,"samples":1},"dram_power_w":{"avg":1,"peak":2,"samples":1}}}"#,
        )
        .unwrap();
        let right = ParsedReport::parse(
            r#"{"command":"b","elapsed_secs":1,"metrics":{"gpu":{"avg":1,"peak":2,"samples":1},"cpu":{"avg":1,"peak":2,"samples":1},"gpu_power_w":{"avg":1,"peak":2,"samples":1}}}"#,
        )
        .unwrap();
        assert_eq!(
            ParsedReport::common_metric_names(&left, &right),
            vec!["cpu", "gpu"]
        );
    }

    #[test]
    fn writes_minimal_json() {
        let result = RunResult {
            command: vec!["sleep".into(), "1".into()],
            wait_status: 0,
            elapsed_secs: 1.0,
            start_bd: 100.0,
            end_bd: 101.0,
            tracked_gpu_pid: None,
            gpu: PercentStats {
                avg: 1.0,
                peak: 2.0,
                samples: 3,
            },
            cpu: PercentStats::default(),
            memory: PercentStats::default(),
            gpu_renderer: PercentStats::default(),
            gpu_tiler: PercentStats::default(),
            gpu_mem_in_use: ScalarStats::default(),
            gpu_mem_allocated: ScalarStats::default(),
            gpu_freq_mhz: ScalarStats::default(),
            gpu_temp_c: ScalarStats::default(),
            gpu_throttle: ThrottleStats::default(),
            cpu_freq_mhz: ScalarStats::default(),
            ecpu_freq_mhz: ScalarStats::default(),
            pcpu_freq_mhz: ScalarStats::default(),
            gpu_power_w: ScalarStats::default(),
            cpu_power_w: ScalarStats::default(),
            dram_power_w: ScalarStats::default(),
            ane_power_w: ScalarStats::default(),
            ecpu_power_w: ScalarStats::default(),
            pcpu_power_w: ScalarStats::default(),
            command_gpu: PercentStats::default(),
            gpu_sram_power_w: ScalarStats::default(),
            mem_wired: ScalarStats::default(),
            mem_compressed: ScalarStats::default(),
            mem_swap: ScalarStats::default(),
            mem_pressure: ScalarStats::default(),
        };
        let chip = ChipProfile {
            brand: "Apple M4 Max".into(),
            family: crate::platform::ChipFamily::M4,
            validated_metrics: true,
        };
        let filter = MetricFilter::parse_list("gpu").unwrap();
        let mut buf = Vec::new();
        write_result(&mut buf, &result, &chip, &filter, None).unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(text.contains("\"gpu\":{\"avg\":1.000000"));
        assert!(!text.contains("\"cpu\":"));
    }
}
