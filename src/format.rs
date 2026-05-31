use crate::metrics::{PercentStats, ScalarStats, ThrottleStats};
use crate::runner::wait_status_to_exit_code;
use std::io::{self, Write};

/// Machine-readable default: GPU/CPU/memory avg+peak, elapsed seconds, BrightDate start/end.
pub const DEFAULT_FORMAT: &str = "%gA,%gP,%uA,%uP,%hA,%hP,%e,%Ws,%Wt\n";

/// True when a format string references metrics beyond gpu/cpu/memory basics.
pub fn format_needs_extended(fmt: &str) -> bool {
    if crate::json::is_json_format(fmt) {
        return false;
    }
    let basic_g = ['A', 'P'];
    let basic_u = ['A', 'P'];
    let basic_h = ['A', 'P'];
    let chars: Vec<char> = fmt.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '%' {
            i += 1;
            continue;
        }
        i += 1;
        if i >= chars.len() {
            break;
        }
        if chars[i] == '%' {
            i += 1;
            continue;
        }
        let family = chars[i];
        i += 1;
        if i >= chars.len() {
            break;
        }
        let spec = chars[i];
        match family {
            'g' if !basic_g.contains(&spec) => return true,
            'u' if !basic_u.contains(&spec) => return true,
            'h' if !basic_h.contains(&spec) => return true,
            'a' => return true,
            _ => {}
        }
        i += 1;
    }
    false
}

#[derive(Debug, Clone)]
pub struct FormatContext {
    pub command: Vec<String>,
    pub wait_status: i32,
    pub elapsed_secs: f64,
    pub start_bd: f64,
    pub end_bd: f64,
    pub gpu: PercentStats,
    pub cpu: PercentStats,
    pub memory: PercentStats,
    pub gpu_renderer: PercentStats,
    pub gpu_tiler: PercentStats,
    pub gpu_mem_in_use: ScalarStats,
    pub gpu_mem_allocated: ScalarStats,
    pub gpu_freq_mhz: ScalarStats,
    pub gpu_temp_c: ScalarStats,
    pub gpu_throttle: ThrottleStats,
    pub cpu_freq_mhz: ScalarStats,
    pub ecpu_freq_mhz: ScalarStats,
    pub pcpu_freq_mhz: ScalarStats,
    pub gpu_power_w: ScalarStats,
    pub cpu_power_w: ScalarStats,
    pub dram_power_w: ScalarStats,
    pub ane_power_w: ScalarStats,
    pub ecpu_power_w: ScalarStats,
    pub pcpu_power_w: ScalarStats,
    pub command_gpu: PercentStats,
    pub gpu_sram_power_w: ScalarStats,
    pub mem_wired: ScalarStats,
    pub mem_compressed: ScalarStats,
    pub mem_swap: ScalarStats,
    pub mem_pressure: ScalarStats,
    pub exercise_target: Option<f64>,
}

impl FormatContext {
    pub fn empty() -> Self {
        Self {
            command: Vec::new(),
            wait_status: 0,
            elapsed_secs: 0.0,
            start_bd: 0.0,
            end_bd: 0.0,
            gpu: PercentStats::default(),
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
            exercise_target: None,
        }
    }

    pub fn elapsed_days(&self) -> f64 {
        self.elapsed_secs / 86_400.0
    }

    pub fn from_run(result: &crate::RunResult, exercise_target: Option<f64>) -> Self {
        Self {
            command: result.command.clone(),
            wait_status: result.wait_status,
            elapsed_secs: result.elapsed_secs,
            start_bd: result.start_bd,
            end_bd: result.end_bd,
            gpu: result.gpu,
            cpu: result.cpu,
            memory: result.memory,
            gpu_renderer: result.gpu_renderer,
            gpu_tiler: result.gpu_tiler,
            gpu_mem_in_use: result.gpu_mem_in_use,
            gpu_mem_allocated: result.gpu_mem_allocated,
            gpu_freq_mhz: result.gpu_freq_mhz,
            gpu_temp_c: result.gpu_temp_c,
            gpu_throttle: result.gpu_throttle,
            cpu_freq_mhz: result.cpu_freq_mhz,
            ecpu_freq_mhz: result.ecpu_freq_mhz,
            pcpu_freq_mhz: result.pcpu_freq_mhz,
            gpu_power_w: result.gpu_power_w,
            cpu_power_w: result.cpu_power_w,
            dram_power_w: result.dram_power_w,
            ane_power_w: result.ane_power_w,
            ecpu_power_w: result.ecpu_power_w,
            pcpu_power_w: result.pcpu_power_w,
            command_gpu: result.command_gpu,
            gpu_sram_power_w: result.gpu_sram_power_w,
            mem_wired: result.mem_wired,
            mem_compressed: result.mem_compressed,
            mem_swap: result.mem_swap,
            mem_pressure: result.mem_pressure,
            exercise_target,
        }
    }
}

pub fn summarize(out: &mut dyn Write, fmt: &str, ctx: &FormatContext) -> io::Result<()> {
    let mut chars = fmt.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '%' => match chars.next() {
                Some('%') => out.write_all(b"%")?,
                Some('C') => write_command(out, &ctx.command)?,
                Some('E') => write!(out, "{}", format_elapsed_hms(ctx.elapsed_secs))?,
                Some('W') => match chars.peek().copied() {
                    Some('t') => {
                        chars.next();
                        write!(out, "{:.9}", ctx.end_bd)?;
                    }
                    Some('s') => {
                        chars.next();
                        write!(out, "{:.9}", ctx.start_bd)?;
                    }
                    Some('\0') | None => out.write_all(b"W")?,
                    Some(other) => write!(out, "W?={other}")?,
                },
                Some('B') => write!(out, "{:.9}", ctx.elapsed_days())?,
                Some('N') => write!(out, "{:.9}", ctx.start_bd)?,
                Some('b') => write!(out, "{:.6}", ctx.elapsed_days() * 1_000.0)?,
                Some('d') => match chars.next() {
                    Some('E') => write!(out, "{:.6} md", ctx.elapsed_secs / 86.4)?,
                    Some('\0') | None => out.write_all(b"d")?,
                    Some(other) => write!(out, "d?={other}")?,
                },
                Some('e') => write!(out, "{}", format_elapsed_seconds(ctx.elapsed_secs))?,
                Some('a') => match chars.next() {
                    Some('B') => write_scalar(out, ctx.ane_power_w.avg, 2)?,
                    Some('K') => write_scalar(out, ctx.ane_power_w.peak, 2)?,
                    Some('\0') | None => out.write_all(b"a")?,
                    Some(other) => write!(out, "a?={other}")?,
                },
                Some('g') => match chars.next() {
                    Some('A') => write_pct(out, ctx.gpu.avg)?,
                    Some('P') => write_pct(out, ctx.gpu.peak)?,
                    Some('I') => write_scalar(out, ctx.gpu_mem_in_use.avg, 0)?,
                    Some('J') => write_scalar(out, ctx.gpu_mem_in_use.peak, 0)?,
                    Some('M') => write_scalar(out, ctx.gpu_mem_allocated.avg, 0)?,
                    Some('O') => write_scalar(out, ctx.gpu_mem_allocated.peak, 0)?,
                    Some('R') => write_pct(out, ctx.gpu_renderer.avg)?,
                    Some('S') => write_pct(out, ctx.gpu_renderer.peak)?,
                    Some('L') => write_pct(out, ctx.gpu_tiler.avg)?,
                    Some('Y') => write_pct(out, ctx.gpu_tiler.peak)?,
                    Some('F') => write_scalar(out, ctx.gpu_freq_mhz.avg, 0)?,
                    Some('V') => write_scalar(out, ctx.gpu_freq_mhz.peak, 0)?,
                    Some('U') => write_scalar(out, ctx.gpu_temp_c.avg, 1)?,
                    Some('W') => write_scalar(out, ctx.gpu_temp_c.peak, 1)?,
                    Some('T') => write_pct(out, ctx.gpu_throttle.pct())?,
                    Some('B') => write_scalar(out, ctx.gpu_power_w.avg, 2)?,
                    Some('K') => write_scalar(out, ctx.gpu_power_w.peak, 2)?,
                    Some('C') => write_pct(out, ctx.command_gpu.avg)?,
                    Some('D') => write_pct(out, ctx.command_gpu.peak)?,
                    Some('N') => write_scalar(out, ctx.gpu_sram_power_w.avg, 2)?,
                    Some('Q') => write_scalar(out, ctx.gpu_sram_power_w.peak, 2)?,
                    Some('\0') | None => out.write_all(b"g")?,
                    Some(other) => write!(out, "g?={other}")?,
                },
                Some('h') => match chars.next() {
                    Some('A') => write_pct(out, ctx.memory.avg)?,
                    Some('P') => write_pct(out, ctx.memory.peak)?,
                    Some('W') => write_scalar(out, ctx.mem_wired.avg, 0)?,
                    Some('X') => write_scalar(out, ctx.mem_wired.peak, 0)?,
                    Some('C') => write_scalar(out, ctx.mem_compressed.avg, 0)?,
                    Some('D') => write_scalar(out, ctx.mem_compressed.peak, 0)?,
                    Some('S') => write_scalar(out, ctx.mem_swap.avg, 0)?,
                    Some('O') => write_scalar(out, ctx.mem_swap.peak, 0)?,
                    Some('K') => write_scalar(out, ctx.mem_pressure.avg, 0)?,
                    Some('L') => write_scalar(out, ctx.mem_pressure.peak, 0)?,
                    Some('G') => write_scalar(out, ctx.dram_power_w.avg, 2)?,
                    Some('J') => write_scalar(out, ctx.dram_power_w.peak, 2)?,
                    Some('\0') | None => out.write_all(b"h")?,
                    Some(other) => write!(out, "h?={other}")?,
                },
                Some('n') => write!(out, "{:.9}", ctx.end_bd)?,
                Some('t') => match chars.next() {
                    Some('G') => {
                        if let Some(target) = ctx.exercise_target {
                            write!(out, "{target:.0}")?;
                        }
                    }
                    Some('\0') | None => out.write_all(b"t")?,
                    Some(other) => write!(out, "t?={other}")?,
                },
                Some('u') => match chars.next() {
                    Some('A') => write_pct(out, ctx.cpu.avg)?,
                    Some('P') => write_pct(out, ctx.cpu.peak)?,
                    Some('F') => write_scalar(out, ctx.cpu_freq_mhz.avg, 0)?,
                    Some('V') => write_scalar(out, ctx.cpu_freq_mhz.peak, 0)?,
                    Some('E') => write_scalar(out, ctx.ecpu_freq_mhz.avg, 0)?,
                    Some('Q') => write_scalar(out, ctx.ecpu_freq_mhz.peak, 0)?,
                    Some('H') => write_scalar(out, ctx.pcpu_freq_mhz.avg, 0)?,
                    Some('Z') => write_scalar(out, ctx.pcpu_freq_mhz.peak, 0)?,
                    Some('B') => write_scalar(out, ctx.cpu_power_w.avg, 2)?,
                    Some('K') => write_scalar(out, ctx.cpu_power_w.peak, 2)?,
                    Some('G') => write_scalar(out, ctx.ecpu_power_w.avg, 2)?,
                    Some('R') => write_scalar(out, ctx.ecpu_power_w.peak, 2)?,
                    Some('I') => write_scalar(out, ctx.pcpu_power_w.avg, 2)?,
                    Some('S') => write_scalar(out, ctx.pcpu_power_w.peak, 2)?,
                    Some('\0') | None => out.write_all(b"u")?,
                    Some(other) => write!(out, "u?={other}")?,
                },
                Some('x') => write!(out, "{}", wait_status_to_exit_code(ctx.wait_status))?,
                Some('\0') | None => out.write_all(b"?")?,
                Some(other) => write!(out, "?{other}")?,
            },
            '\\' => match chars.next() {
                Some('t') => out.write_all(b"\t")?,
                Some('n') => out.write_all(b"\n")?,
                Some('\\') => out.write_all(b"\\")?,
                Some('\0') | None => out.write_all(b"?\\")?,
                Some(other) => write!(out, "?\\{other}")?,
            },
            other => write!(out, "{other}")?,
        }
    }
    Ok(())
}

pub fn summarize_line(out: &mut dyn Write, fmt: &str, ctx: &FormatContext) -> io::Result<()> {
    summarize(out, fmt, ctx)?;
    if !fmt.ends_with('\n') {
        writeln!(out)?;
    }
    Ok(())
}

fn write_pct(out: &mut dyn Write, value: f64) -> io::Result<()> {
    write!(out, "{value:.1}")
}

fn write_scalar(out: &mut dyn Write, value: f64, decimals: u32) -> io::Result<()> {
    if value.is_finite() && value > 0.0 {
        write!(out, "{value:.prec$}", prec = decimals as usize)
    } else {
        write!(out, "0")
    }
}

fn write_command(out: &mut dyn Write, command: &[String]) -> io::Result<()> {
    for (i, part) in command.iter().enumerate() {
        if i > 0 {
            out.write_all(b" ")?;
        }
        out.write_all(part.as_bytes())?;
    }
    Ok(())
}

fn format_elapsed_seconds(secs: f64) -> String {
    let sec = secs.trunc() as i64;
    let centis = ((secs - sec as f64) * 100.0).round() as i64;
    format!("{sec}.{centis:02}")
}

fn format_elapsed_hms(secs: f64) -> String {
    let total_sec = secs.trunc() as i64;
    let centis = ((secs - total_sec as f64) * 100.0).round() as i64;
    if total_sec >= 3600 {
        format!(
            "{}:{:02}:{:02}",
            total_sec / 3600,
            (total_sec % 3600) / 60,
            total_sec % 60
        )
    } else {
        format!(
            "{}:{:02}.{:02}",
            total_sec / 60,
            total_sec % 60,
            centis
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::{PercentStats, ScalarStats, ThrottleStats};

    fn sample_context() -> FormatContext {
        FormatContext {
            command: vec!["bgpucap".into(), "sleep".into(), "1".into()],
            wait_status: 0,
            elapsed_secs: 0.5,
            start_bd: 9645.0,
            end_bd: 9645.00001,
            gpu: PercentStats {
                avg: 12.3,
                peak: 45.6,
                samples: 5,
            },
            cpu: PercentStats {
                avg: 8.1,
                peak: 23.4,
                samples: 5,
            },
            memory: PercentStats {
                avg: 52.0,
                peak: 55.1,
                samples: 5,
            },
            gpu_mem_in_use: ScalarStats {
                avg: 1_000_000_000.0,
                peak: 2_000_000_000.0,
                samples: 5,
            },
            gpu_power_w: ScalarStats {
                avg: 15.5,
                peak: 22.0,
                samples: 5,
            },
            cpu_power_w: ScalarStats {
                avg: 3.2,
                peak: 8.0,
                samples: 5,
            },
            ane_power_w: ScalarStats {
                avg: 0.05,
                peak: 0.12,
                samples: 5,
            },
            ecpu_power_w: ScalarStats {
                avg: 1.2,
                peak: 2.5,
                samples: 5,
            },
            pcpu_power_w: ScalarStats {
                avg: 2.0,
                peak: 5.5,
                samples: 5,
            },
            ecpu_freq_mhz: ScalarStats {
                avg: 900.0,
                peak: 1200.0,
                samples: 5,
            },
            pcpu_freq_mhz: ScalarStats {
                avg: 3200.0,
                peak: 4100.0,
                samples: 5,
            },
            gpu_throttle: ThrottleStats {
                throttled_samples: 1,
                samples: 4,
            },
            ..FormatContext::empty()
        }
    }

    #[test]
    fn extended_metric_specifiers() {
        let mut buf = Vec::new();
        summarize(
            &mut buf,
            "pwr=%gB cpu=%uB e=%uE/%uG p=%uH/%uI ane=%aB dram=%hG",
            &sample_context(),
        )
        .unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(text.contains("pwr=15.50"));
        assert!(text.contains("cpu=3.20"));
        assert!(text.contains("ane=0.05"));
        assert!(text.contains("e=900/1.20"));
        assert!(text.contains("p=3200/2.00"));
        assert!(text.contains("dram=0"));
    }

    #[test]
    fn gpucap_metric_specifiers() {
        let mut buf = Vec::new();
        summarize(
            &mut buf,
            "gpu=%gA/%gP cpu=%uA/%uP mem=%hA/%hP",
            &sample_context(),
        )
        .unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(text.contains("gpu=12.3/45.6"));
        assert!(text.contains("cpu=8.1/23.4"));
        assert!(text.contains("mem=52.0/55.1"));
    }

    #[test]
    fn brightdate_and_elapsed_specifiers() {
        let mut buf = Vec::new();
        summarize(
            &mut buf,
            "e=%e E=%E bd=%B md=%b dE=%dE start=%Ws end=%Wt N=%N n=%n",
            &sample_context(),
        )
        .unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(text.contains("e=0.50"));
        assert!(text.contains("E=0:00.50"));
        assert!(text.contains("bd=0.000005787"));
        assert!(text.contains("md=0.005787"));
        assert!(text.contains("dE=0.005787 md"));
        assert!(text.contains("start=9645.000000000"));
        assert!(text.contains("end=9645.000010000"));
    }

    #[test]
    fn command_exit_and_default_format() {
        let ctx = sample_context();
        let mut buf = Vec::new();
        summarize(&mut buf, "cmd=%C x=%x", &ctx).unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(text.contains("cmd=bgpucap sleep 1"));
        assert!(text.contains("x=0"));

        let mut default = Vec::new();
        summarize(&mut default, DEFAULT_FORMAT, &ctx).unwrap();
        let default_text = String::from_utf8(default).unwrap();
        assert!(default_text.starts_with("12.3,45.6,8.1,23.4,52.0,55.1,0.50,"));
    }

    #[test]
    fn default_format_is_basic_tier() {
        assert!(!format_needs_extended(DEFAULT_FORMAT));
        assert!(format_needs_extended("%gB"));
    }

    #[test]
    fn basics_only_context_formats_without_extended_metrics() {
        let mut ctx = FormatContext::empty();
        ctx.command = vec!["sleep".into(), "0".into()];
        ctx.elapsed_secs = 0.1;
        ctx.start_bd = 100.0;
        ctx.end_bd = 100.0001;
        ctx.gpu = PercentStats {
            avg: 1.0,
            peak: 2.0,
            samples: 1,
        };
        ctx.cpu = PercentStats {
            avg: 3.0,
            peak: 4.0,
            samples: 1,
        };
        ctx.memory = PercentStats {
            avg: 50.0,
            peak: 51.0,
            samples: 1,
        };
        let mut buf = Vec::new();
        summarize(&mut buf, "%gA,%gP,%uA,%uP,%hA,%hP", &ctx).unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert_eq!(text, "1.0,2.0,3.0,4.0,50.0,51.0");
    }
}
