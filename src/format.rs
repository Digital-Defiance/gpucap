use crate::metrics::PercentStats;
use crate::runner::wait_status_to_exit_code;
use std::io::{self, Write};

/// Machine-readable default: GPU/CPU/memory avg+peak, elapsed seconds, BrightDate start/end.
pub const DEFAULT_FORMAT: &str = "%gA,%gP,%uA,%uP,%hA,%hP,%e,%Ws,%Wt\n";

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
    pub exercise_target: Option<f64>,
}

impl FormatContext {
    pub fn elapsed_days(&self) -> f64 {
        self.elapsed_secs / 86_400.0
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
                Some('g') => match chars.next() {
                    Some('A') => write_pct(out, ctx.gpu.avg)?,
                    Some('P') => write_pct(out, ctx.gpu.peak)?,
                    Some('\0') | None => out.write_all(b"g")?,
                    Some(other) => write!(out, "g?={other}")?,
                },
                Some('h') => match chars.next() {
                    Some('A') => write_pct(out, ctx.memory.avg)?,
                    Some('P') => write_pct(out, ctx.memory.peak)?,
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
    use crate::metrics::PercentStats;

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
            exercise_target: None,
        }
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
        assert!(text.contains("N=9645.000000000"));
        assert!(text.contains("n=9645.000010000"));
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
    fn exercise_target_specifier() {
        let mut ctx = sample_context();
        ctx.exercise_target = Some(50.0);
        let mut buf = Vec::new();
        summarize(&mut buf, "target=%tG", &ctx).unwrap();
        assert_eq!(String::from_utf8(buf).unwrap(), "target=50");
    }

    #[test]
    fn literal_percent_and_escape() {
        let mut buf = Vec::new();
        summarize(&mut buf, "100%% done\\t!", &sample_context()).unwrap();
        assert_eq!(String::from_utf8(buf).unwrap(), "100% done\t!");
    }

    #[test]
    fn unknown_specifiers() {
        let mut buf = Vec::new();
        summarize(&mut buf, "bad=%z W=%Wx", &sample_context()).unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(text.contains("bad=?z"));
        assert!(text.contains("W?=x"));
    }

    #[test]
    fn summarize_line_adds_newline_when_missing() {
        let mut buf = Vec::new();
        summarize_line(&mut buf, "x=%x", &sample_context()).unwrap();
        assert_eq!(String::from_utf8(buf).unwrap(), "x=0\n");
    }

    #[test]
    fn summarize_line_preserves_trailing_newline() {
        let mut buf = Vec::new();
        summarize_line(&mut buf, "x=%x\n", &sample_context()).unwrap();
        assert_eq!(String::from_utf8(buf).unwrap(), "x=0\n");
    }

    #[test]
    fn all_gpucap_percent_specifiers() {
        let ctx = sample_context();
        let cases = [
            ("%gA", "12.3"),
            ("%gP", "45.6"),
            ("%uA", "8.1"),
            ("%uP", "23.4"),
            ("%hA", "52.0"),
            ("%hP", "55.1"),
        ];
        for (spec, expected) in cases {
            let mut buf = Vec::new();
            summarize(&mut buf, spec, &ctx).unwrap();
            assert_eq!(String::from_utf8(buf).unwrap(), expected, "spec {spec}");
        }
    }
}
