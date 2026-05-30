use std::io::{self, IsTerminal};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorWhen {
    Auto,
    Always,
    Never,
    Ansi,
    TrueColor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorScheme {
    #[default]
    Default,
    Bright,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RenderMode {
    Off,
    Ansi,
    TrueColor,
}

#[derive(Debug, Clone, Copy)]
pub struct Colors {
    pub reset: &'static str,
    pub title: &'static str,
    pub label: &'static str,
    pub detail: &'static str,
    pub value: &'static str,
    pub unit: &'static str,
    pub gpu: &'static str,
    pub cpu: &'static str,
    pub memory: &'static str,
    pub real: &'static str,
    pub ok: &'static str,
    pub warn: &'static str,
    pub bad: &'static str,
}

impl Colors {
    pub const OFF: Self = Self {
        reset: "",
        title: "",
        label: "",
        detail: "",
        value: "",
        unit: "",
        gpu: "",
        cpu: "",
        memory: "",
        real: "",
        ok: "",
        warn: "",
        bad: "",
    };

    pub fn enabled(&self) -> bool {
        !self.reset.is_empty()
    }

    pub fn label(&self, name: &str, style: &str) -> String {
        if self.enabled() {
            format!("{}{:<8}{}", style, name, self.reset)
        } else {
            format!("{:<8}", name)
        }
    }

    pub fn pct_style(&self, pct: f64) -> &str {
        if !self.enabled() {
            return "";
        }
        if pct >= 90.0 {
            self.bad
        } else if pct >= 50.0 {
            self.warn
        } else {
            self.ok
        }
    }

    pub fn resolve(when: ColorWhen, scheme: ColorScheme) -> Self {
        if when == ColorWhen::Never {
            return Self::OFF;
        }

        if when == ColorWhen::Auto && no_color_requested() {
            return Self::OFF;
        }

        let mode = match when {
            ColorWhen::Never => RenderMode::Off,
            ColorWhen::Ansi => RenderMode::Ansi,
            ColorWhen::TrueColor => RenderMode::TrueColor,
            ColorWhen::Always => preferred_render_mode(),
            ColorWhen::Auto => {
                if io::stderr().is_terminal() || force_color_enabled() {
                    preferred_render_mode()
                } else {
                    RenderMode::Off
                }
            }
        };

        match mode {
            RenderMode::Off => Self::OFF,
            RenderMode::Ansi => Self::ansi(scheme),
            RenderMode::TrueColor => Self::truecolor(scheme),
        }
    }

    fn ansi(scheme: ColorScheme) -> Self {
        match scheme {
            ColorScheme::Default => Self {
                reset: "\x1b[0m",
                title: "\x1b[1;36m",
                label: "\x1b[36m",
                detail: "\x1b[2m",
                value: "\x1b[1m",
                unit: "\x1b[2m",
                gpu: "\x1b[35m",
                cpu: "\x1b[33m",
                memory: "\x1b[32m",
                real: "\x1b[1;36m",
                ok: "\x1b[32m",
                warn: "\x1b[33m",
                bad: "\x1b[1;31m",
            },
            ColorScheme::Bright => Self {
                reset: "\x1b[0m",
                title: "\x1b[1;96m",
                label: "\x1b[96m",
                detail: "\x1b[2m",
                value: "\x1b[1;97m",
                unit: "\x1b[2m",
                gpu: "\x1b[95m",
                cpu: "\x1b[93m",
                memory: "\x1b[92m",
                real: "\x1b[1;96m",
                ok: "\x1b[92m",
                warn: "\x1b[93m",
                bad: "\x1b[1;91m",
            },
        }
    }

    fn truecolor(scheme: ColorScheme) -> Self {
        match scheme {
            ColorScheme::Default => Self {
                reset: "\x1b[0m",
                title: "\x1b[1;38;2;80;200;220m",
                label: "\x1b[38;2;80;200;220m",
                detail: "\x1b[2;38;2;120;120;120m",
                value: "\x1b[1;38;2;240;240;240m",
                unit: "\x1b[2;38;2;128;128;128m",
                gpu: "\x1b[38;2;200;120;220m",
                cpu: "\x1b[38;2;240;200;80m",
                memory: "\x1b[38;2;120;220;140m",
                real: "\x1b[1;38;2;80;200;220m",
                ok: "\x1b[38;2;100;200;100m",
                warn: "\x1b[38;2;240;200;80m",
                bad: "\x1b[38;2;240;100;100m",
            },
            ColorScheme::Bright => Self {
                reset: "\x1b[0m",
                title: "\x1b[1;38;2;0;220;255m",
                label: "\x1b[38;2;0;220;255m",
                detail: "\x1b[2;38;2;160;160;160m",
                value: "\x1b[1;38;2;255;255;255m",
                unit: "\x1b[2;38;2;160;160;160m",
                gpu: "\x1b[38;2;255;120;255m",
                cpu: "\x1b[38;2;255;220;80m",
                memory: "\x1b[38;2;80;255;120m",
                real: "\x1b[1;38;2;0;220;255m",
                ok: "\x1b[38;2;80;255;120m",
                warn: "\x1b[38;2;255;220;80m",
                bad: "\x1b[38;2;255;80;80m",
            },
        }
    }
}

pub fn parse_color_when(value: &str) -> Result<ColorWhen, String> {
    match value.to_ascii_lowercase().as_str() {
        "auto" => Ok(ColorWhen::Auto),
        "always" | "on" | "yes" => Ok(ColorWhen::Always),
        "never" | "off" | "no" | "plain" => Ok(ColorWhen::Never),
        "ansi" | "16" => Ok(ColorWhen::Ansi),
        "truecolor" | "24bit" | "rgb" => Ok(ColorWhen::TrueColor),
        other => Err(format!(
            "invalid color mode '{other}' (expected auto, always, never, plain, ansi, or truecolor)"
        )),
    }
}

pub fn parse_color_scheme(value: &str) -> Result<ColorScheme, String> {
    match value.to_ascii_lowercase().as_str() {
        "default" => Ok(ColorScheme::Default),
        "bright" => Ok(ColorScheme::Bright),
        other => Err(format!("invalid color scheme '{other}' (expected default or bright)")),
    }
}

fn no_color_requested() -> bool {
    std::env::var_os("NO_COLOR").is_some()
        || std::env::var("CLICOLOR")
            .map(|v| matches!(v.as_str(), "0" | "false"))
            .unwrap_or(false)
}

fn force_color_enabled() -> bool {
    std::env::var("CLICOLOR_FORCE")
        .map(|v| !matches!(v.as_str(), "" | "0" | "false"))
        .unwrap_or(false)
}

fn preferred_render_mode() -> RenderMode {
    if term_supports_truecolor() {
        RenderMode::TrueColor
    } else {
        RenderMode::Ansi
    }
}

fn term_supports_truecolor() -> bool {
    std::env::var("COLORTERM")
        .map(|v| {
            let v = v.to_ascii_lowercase();
            v == "truecolor" || v == "24bit"
        })
        .unwrap_or(false)
        || std::env::var("TERM")
            .map(|v| v.contains("truecolor"))
            .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_color_when_values() {
        assert_eq!(parse_color_when("auto").unwrap(), ColorWhen::Auto);
        assert_eq!(parse_color_when("24bit").unwrap(), ColorWhen::TrueColor);
        assert!(parse_color_when("nope").is_err());
    }
}
