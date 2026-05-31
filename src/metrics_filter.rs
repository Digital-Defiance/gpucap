use std::collections::HashSet;

/// Human-report metric identifiers (match output row labels).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MetricId {
    Gpu,
    CmdGpu,
    Cpu,
    Memory,
    GpuMem,
    Renderer,
    Tiler,
    GpuMhz,
    CpuMhz,
    EMhz,
    PMhz,
    GpuPwr,
    SramPwr,
    CpuPwr,
    DramPwr,
    AnePwr,
    EPwr,
    PPwr,
    GpuTemp,
    Throttle,
    Wired,
    Compress,
    Swap,
    Pressure,
}

impl MetricId {
    pub fn name(self) -> &'static str {
        match self {
            Self::Gpu => "gpu",
            Self::CmdGpu => "cmd-gpu",
            Self::Cpu => "cpu",
            Self::Memory => "memory",
            Self::GpuMem => "gpu-mem",
            Self::Renderer => "renderer",
            Self::Tiler => "tiler",
            Self::GpuMhz => "gpu-mhz",
            Self::CpuMhz => "cpu-mhz",
            Self::EMhz => "e-mhz",
            Self::PMhz => "p-mhz",
            Self::GpuPwr => "gpu-pwr",
            Self::SramPwr => "sram-pwr",
            Self::CpuPwr => "cpu-pwr",
            Self::DramPwr => "dram-pwr",
            Self::AnePwr => "ane-pwr",
            Self::EPwr => "e-pwr",
            Self::PPwr => "p-pwr",
            Self::GpuTemp => "gpu-temp",
            Self::Throttle => "throttle",
            Self::Wired => "wired",
            Self::Compress => "compress",
            Self::Swap => "swap",
            Self::Pressure => "pressure",
        }
    }

    pub fn all() -> &'static [MetricId] {
        &[
            Self::Gpu,
            Self::CmdGpu,
            Self::Cpu,
            Self::Memory,
            Self::GpuMem,
            Self::Renderer,
            Self::Tiler,
            Self::GpuMhz,
            Self::CpuMhz,
            Self::EMhz,
            Self::PMhz,
            Self::GpuPwr,
            Self::SramPwr,
            Self::CpuPwr,
            Self::DramPwr,
            Self::AnePwr,
            Self::EPwr,
            Self::PPwr,
            Self::GpuTemp,
            Self::Throttle,
            Self::Wired,
            Self::Compress,
            Self::Swap,
            Self::Pressure,
        ]
    }

    fn from_token(token: &str) -> Option<Self> {
        match token {
            "gpu" => Some(Self::Gpu),
            "cmd-gpu" | "cmd" | "cmdgpu" => Some(Self::CmdGpu),
            "cpu" => Some(Self::Cpu),
            "memory" | "mem" => Some(Self::Memory),
            "gpu-mem" | "gpumem" => Some(Self::GpuMem),
            "renderer" => Some(Self::Renderer),
            "tiler" => Some(Self::Tiler),
            "gpu-mhz" | "gpumhz" => Some(Self::GpuMhz),
            "cpu-mhz" | "cpumhz" => Some(Self::CpuMhz),
            "e-mhz" | "emhz" | "ecpu-mhz" => Some(Self::EMhz),
            "p-mhz" | "pmhz" | "pcpu-mhz" => Some(Self::PMhz),
            "gpu-pwr" | "gpupwr" => Some(Self::GpuPwr),
            "sram-pwr" | "srampwr" | "gpu-sram" => Some(Self::SramPwr),
            "cpu-pwr" | "cpupwr" => Some(Self::CpuPwr),
            "dram-pwr" | "drampwr" | "dram" => Some(Self::DramPwr),
            "ane-pwr" | "anepwr" | "ane" => Some(Self::AnePwr),
            "e-pwr" | "epwr" | "ecpu-pwr" => Some(Self::EPwr),
            "p-pwr" | "ppwr" | "pcpu-pwr" => Some(Self::PPwr),
            "gpu-temp" | "gputemp" | "temp" => Some(Self::GpuTemp),
            "throttle" => Some(Self::Throttle),
            "wired" => Some(Self::Wired),
            "compress" | "compressed" => Some(Self::Compress),
            "swap" => Some(Self::Swap),
            "pressure" => Some(Self::Pressure),
            _ => None,
        }
    }

    fn expand_group(token: &str) -> Option<&'static [MetricId]> {
        match token {
            "basic" | "basics" | "core" => Some(&[
                Self::Gpu,
                Self::Cpu,
                Self::Memory,
            ]),
            "extended" => Some(&[
                Self::CmdGpu,
                Self::GpuMem,
                Self::Renderer,
                Self::Tiler,
                Self::GpuMhz,
                Self::CpuMhz,
                Self::EMhz,
                Self::PMhz,
                Self::GpuPwr,
                Self::SramPwr,
                Self::CpuPwr,
                Self::DramPwr,
                Self::AnePwr,
                Self::EPwr,
                Self::PPwr,
                Self::GpuTemp,
                Self::Throttle,
                Self::Wired,
                Self::Compress,
                Self::Swap,
                Self::Pressure,
            ]),
            "power" => Some(&[
                Self::GpuPwr,
                Self::SramPwr,
                Self::CpuPwr,
                Self::DramPwr,
                Self::AnePwr,
                Self::EPwr,
                Self::PPwr,
            ]),
            "freq" | "frequency" | "frequencies" => Some(&[
                Self::GpuMhz,
                Self::CpuMhz,
                Self::EMhz,
                Self::PMhz,
            ]),
            "gpu-detail" | "gpu-extended" => Some(&[
                Self::CmdGpu,
                Self::GpuMem,
                Self::Renderer,
                Self::Tiler,
                Self::GpuMhz,
                Self::GpuPwr,
                Self::SramPwr,
                Self::GpuTemp,
                Self::Throttle,
            ]),
            "memory-detail" | "host-mem" | "host-memory" => Some(&[
                Self::Wired,
                Self::Compress,
                Self::Swap,
                Self::Pressure,
            ]),
            _ => None,
        }
    }
}

pub const METRICS_HELP: &str = "Comma-separated metrics for human output (default: all). \
Names: gpu, cmd-gpu, cpu, memory, gpu-mem, renderer, tiler, gpu-mhz, cpu-mhz, e-mhz, p-mhz, \
gpu-pwr, sram-pwr, cpu-pwr, dram-pwr, ane-pwr, e-pwr, p-pwr, gpu-temp, throttle, wired, \
compress, swap, pressure. Groups: basic, extended, power, freq, gpu-detail, memory-detail. \
Aliases: mem, cmd, dram, ane, temp. Env: BGPUCAP_METRICS";

#[derive(Debug, Clone, Default)]
pub struct MetricFilter {
    show_all: bool,
    selected: HashSet<MetricId>,
}

impl MetricFilter {
    pub fn all() -> Self {
        Self {
            show_all: true,
            selected: HashSet::new(),
        }
    }

    pub fn show(&self, id: MetricId) -> bool {
        self.show_all || self.selected.contains(&id)
    }

    pub fn parse_list(input: &str) -> Result<Self, String> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Ok(Self::all());
        }
        if trimmed.eq_ignore_ascii_case("all") {
            return Ok(Self::all());
        }

        let mut selected = HashSet::new();
        for part in trimmed.split(',') {
            let token = part.trim().to_ascii_lowercase();
            if token.is_empty() {
                continue;
            }
            if token == "all" {
                return Ok(Self::all());
            }
            if let Some(group) = MetricId::expand_group(&token) {
                selected.extend(group.iter().copied());
                continue;
            }
            if let Some(id) = MetricId::from_token(&token) {
                selected.insert(id);
                continue;
            }
            return Err(format!(
                "unknown metric or group '{token}' (try --help for names)"
            ));
        }

        if selected.is_empty() {
            return Err("empty --metrics list".to_string());
        }

        Ok(Self {
            show_all: false,
            selected,
        })
    }

    pub fn needs_extended_sampling(&self) -> bool {
        if self.show_all {
            return true;
        }
        self.selected
            .iter()
            .any(|id| !matches!(id, MetricId::Gpu | MetricId::Cpu | MetricId::Memory))
    }

    pub fn needs_cmd_gpu(&self) -> bool {
        self.show(MetricId::CmdGpu)
    }

    pub fn print_list() {
        use std::io::Write;
        let _ = writeln!(std::io::stdout(), "Metrics (for --metrics / BGPUCAP_METRICS):\n");
        let _ = writeln!(std::io::stdout(), "  Individual:");
        for id in MetricId::all() {
            let _ = writeln!(std::io::stdout(), "    {}", id.name());
        }
        let _ = writeln!(std::io::stdout(), "\n  Groups:");
        for (name, _) in METRIC_GROUPS {
            let _ = writeln!(std::io::stdout(), "    {name}");
        }
        let _ = writeln!(
            std::io::stdout(),
            "\n  Aliases: mem, cmd, dram, ane, temp, gpumhz, cpumhz, …"
        );
        let _ = writeln!(std::io::stdout(), "  Use 'all' (default) for every metric.");
    }
}

const METRIC_GROUPS: &[(&str, &[MetricId])] = &[
    (
        "basic",
        &[MetricId::Gpu, MetricId::Cpu, MetricId::Memory],
    ),
    (
        "extended",
        &[
            MetricId::CmdGpu,
            MetricId::GpuMem,
            MetricId::Renderer,
            MetricId::Tiler,
            MetricId::GpuMhz,
            MetricId::CpuMhz,
            MetricId::EMhz,
            MetricId::PMhz,
            MetricId::GpuPwr,
            MetricId::SramPwr,
            MetricId::CpuPwr,
            MetricId::DramPwr,
            MetricId::AnePwr,
            MetricId::EPwr,
            MetricId::PPwr,
            MetricId::GpuTemp,
            MetricId::Throttle,
            MetricId::Wired,
            MetricId::Compress,
            MetricId::Swap,
            MetricId::Pressure,
        ],
    ),
    (
        "power",
        &[
            MetricId::GpuPwr,
            MetricId::SramPwr,
            MetricId::CpuPwr,
            MetricId::DramPwr,
            MetricId::AnePwr,
            MetricId::EPwr,
            MetricId::PPwr,
        ],
    ),
    (
        "freq",
        &[
            MetricId::GpuMhz,
            MetricId::CpuMhz,
            MetricId::EMhz,
            MetricId::PMhz,
        ],
    ),
    (
        "gpu-detail",
        &[
            MetricId::CmdGpu,
            MetricId::GpuMem,
            MetricId::Renderer,
            MetricId::Tiler,
            MetricId::GpuMhz,
            MetricId::GpuPwr,
            MetricId::SramPwr,
            MetricId::GpuTemp,
            MetricId::Throttle,
        ],
    ),
    (
        "memory-detail",
        &[
            MetricId::Wired,
            MetricId::Compress,
            MetricId::Swap,
            MetricId::Pressure,
        ],
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_shows_all() {
        let f = MetricFilter::all();
        assert!(f.show(MetricId::Gpu));
        assert!(f.show(MetricId::GpuPwr));
    }

    #[test]
    fn basic_group_expands() {
        let f = MetricFilter::parse_list("basic").unwrap();
        assert!(f.show(MetricId::Gpu));
        assert!(f.show(MetricId::Cpu));
        assert!(f.show(MetricId::Memory));
        assert!(!f.show(MetricId::GpuPwr));
    }

    #[test]
    fn comma_list_and_aliases() {
        let f = MetricFilter::parse_list("gpu,mem,cmd").unwrap();
        assert!(f.show(MetricId::Gpu));
        assert!(f.show(MetricId::Memory));
        assert!(f.show(MetricId::CmdGpu));
        assert!(!f.show(MetricId::Cpu));
    }

    #[test]
    fn power_group() {
        let f = MetricFilter::parse_list("power").unwrap();
        assert!(f.show(MetricId::GpuPwr));
        assert!(f.show(MetricId::DramPwr));
        assert!(!f.show(MetricId::Gpu));
    }

    #[test]
    fn basic_skips_extended_sampling() {
        let f = MetricFilter::parse_list("basic").unwrap();
        assert!(!f.needs_extended_sampling());
        assert!(MetricFilter::all().needs_extended_sampling());
    }

    #[test]
    fn unknown_token_errors() {
        assert!(MetricFilter::parse_list("gpu,not-a-metric").is_err());
    }
}
