use crate::gpu_proc::GpuProcessTracker;
use crate::metrics::{MetricSnapshot, PercentStats, SampleTier, ScalarStats, Sampler, ThrottleStats};
use brightdate::BrightDate;
use std::io;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuTrackPid {
    Disabled,
    Child,
    Pid(i32),
}

#[derive(Debug, Clone, Copy)]
pub struct RunOptions {
    pub interval: Duration,
    pub tier: SampleTier,
    pub gpu_track: GpuTrackPid,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            interval: Duration::from_millis(100),
            tier: SampleTier::Full,
            gpu_track: GpuTrackPid::Disabled,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RunResult {
    pub command: Vec<String>,
    pub wait_status: i32,
    pub elapsed_secs: f64,
    pub start_bd: f64,
    pub end_bd: f64,
    pub tracked_gpu_pid: Option<i32>,
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
}

pub fn run_command(cmd: &[&str], options: RunOptions) -> io::Result<RunResult> {
    if cmd.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "missing command",
        ));
    }

    let start = Instant::now();
    let start_bd = BrightDate::now().value;
    let mut sampler = Sampler::with_tier(options.tier);
    let mut stats = RunStats::default();
    let mut tracked = match options.gpu_track {
        GpuTrackPid::Disabled => None,
        GpuTrackPid::Child => None,
        GpuTrackPid::Pid(pid) => Some(pid),
    };

    let wait_status = unsafe {
        let interrupt = libc::signal(libc::SIGINT, libc::SIG_IGN);
        let quit = libc::signal(libc::SIGQUIT, libc::SIG_IGN);

        let pid = libc::fork();
        if pid < 0 {
            libc::signal(libc::SIGINT, interrupt);
            libc::signal(libc::SIGQUIT, quit);
            return Err(io::Error::last_os_error());
        }

        if pid == 0 {
            let c_strings: Vec<std::ffi::CString> = cmd
                .iter()
                .map(|arg| std::ffi::CString::new(*arg))
                .collect::<Result<_, _>>()
                .unwrap_or_else(|_| libc::_exit(126));

            let mut argv: Vec<*const libc::c_char> =
                c_strings.iter().map(|s| s.as_ptr()).collect();
            argv.push(std::ptr::null());

            libc::execvp(c_strings[0].as_ptr(), argv.as_ptr());
            let code = if io::Error::last_os_error().kind() == io::ErrorKind::NotFound {
                127
            } else {
                126
            };
            libc::_exit(code);
        }

        let mut status = 0;
        let mut gpu_proc = match options.gpu_track {
            GpuTrackPid::Disabled => None,
            GpuTrackPid::Child => {
                tracked = Some(pid);
                GpuProcessTracker::new(pid)
            }
            GpuTrackPid::Pid(p) => GpuProcessTracker::new(p),
        };
        let mut last_sample = Instant::now();
        loop {
            let now = Instant::now();
            let dt = now.duration_since(last_sample).as_secs_f64();
            last_sample = now;

            let mut snapshot = sampler.sample();
            if let Some(tracker) = gpu_proc.as_mut() {
                if let Some(cmd_gpu) = tracker.sample(dt) {
                    snapshot.command_gpu = Some(cmd_gpu);
                }
            }
            stats.record(&snapshot);

            let waited = libc::waitpid(pid, &mut status, libc::WNOHANG);
            if waited < 0 {
                let err = io::Error::last_os_error();
                libc::signal(libc::SIGINT, interrupt);
                libc::signal(libc::SIGQUIT, quit);
                return Err(err);
            }
            if waited == pid {
                break;
            }

            std::thread::sleep(options.interval);
        }

        libc::signal(libc::SIGINT, interrupt);
        libc::signal(libc::SIGQUIT, quit);
        status
    };

    Ok(stats.into_run_result(
        cmd.iter().map(|s| (*s).to_string()).collect(),
        wait_status,
        start.elapsed().as_secs_f64(),
        start_bd,
        BrightDate::now().value,
        tracked,
    ))
}

#[derive(Default)]
pub(crate) struct RunStats {
    gpu: PercentStats,
    cpu: PercentStats,
    memory: PercentStats,
    gpu_renderer: PercentStats,
    gpu_tiler: PercentStats,
    gpu_mem_in_use: ScalarStats,
    gpu_mem_allocated: ScalarStats,
    gpu_freq_mhz: ScalarStats,
    gpu_temp_c: ScalarStats,
    gpu_throttle: ThrottleStats,
    cpu_freq_mhz: ScalarStats,
    ecpu_freq_mhz: ScalarStats,
    pcpu_freq_mhz: ScalarStats,
    gpu_power_w: ScalarStats,
    cpu_power_w: ScalarStats,
    dram_power_w: ScalarStats,
    ane_power_w: ScalarStats,
    ecpu_power_w: ScalarStats,
    pcpu_power_w: ScalarStats,
    command_gpu: PercentStats,
    gpu_sram_power_w: ScalarStats,
    mem_wired: ScalarStats,
    mem_compressed: ScalarStats,
    mem_swap: ScalarStats,
    mem_pressure: ScalarStats,
}

impl RunStats {
    pub(crate) fn has_gpu_samples(&self) -> bool {
        self.gpu.samples > 0
    }

    pub(crate) fn record(&mut self, snapshot: &MetricSnapshot) {
        self.gpu.record(snapshot.gpu);
        self.cpu.record(snapshot.cpu);
        self.memory.record(snapshot.memory);
        self.gpu_renderer.record_option(snapshot.gpu_renderer);
        self.gpu_tiler.record_option(snapshot.gpu_tiler);
        if let Some(v) = snapshot.gpu_mem_in_use {
            self.gpu_mem_in_use.record(v as f64);
        }
        if let Some(v) = snapshot.gpu_mem_allocated {
            self.gpu_mem_allocated.record(v as f64);
        }
        self.gpu_freq_mhz.record_option(snapshot.gpu_freq_mhz);
        self.gpu_temp_c.record_option(snapshot.gpu_temp_c);
        if let Some(throttled) = snapshot.gpu_throttled {
            self.gpu_throttle.record(throttled);
        }
        self.cpu_freq_mhz.record_option(snapshot.cpu_freq_mhz);
        self.ecpu_freq_mhz.record_option(snapshot.ecpu_freq_mhz);
        self.pcpu_freq_mhz.record_option(snapshot.pcpu_freq_mhz);
        self.gpu_power_w.record_option(snapshot.gpu_power_w);
        self.cpu_power_w.record_option(snapshot.cpu_power_w);
        self.dram_power_w.record_option(snapshot.dram_power_w);
        self.ane_power_w.record_option(snapshot.ane_power_w);
        self.ecpu_power_w.record_option(snapshot.ecpu_power_w);
        self.pcpu_power_w.record_option(snapshot.pcpu_power_w);
        self.gpu_sram_power_w.record_option(snapshot.gpu_sram_power_w);
        if let Some(cmd_gpu) = snapshot.command_gpu {
            self.command_gpu.record(cmd_gpu);
        }
        self.mem_wired.record(snapshot.mem_wired_bytes as f64);
        self.mem_compressed
            .record(snapshot.mem_compressed_bytes as f64);
        self.mem_swap.record(snapshot.mem_swap_bytes as f64);
        self.mem_pressure.record(snapshot.mem_pressure as f64);
    }

    pub(crate) fn into_run_result(
        self,
        command: Vec<String>,
        wait_status: i32,
        elapsed_secs: f64,
        start_bd: f64,
        end_bd: f64,
        tracked_gpu_pid: Option<i32>,
    ) -> RunResult {
        RunResult {
            command,
            wait_status,
            elapsed_secs,
            start_bd,
            end_bd,
            tracked_gpu_pid,
            gpu: self.gpu,
            cpu: self.cpu,
            memory: self.memory,
            gpu_renderer: self.gpu_renderer,
            gpu_tiler: self.gpu_tiler,
            gpu_mem_in_use: self.gpu_mem_in_use,
            gpu_mem_allocated: self.gpu_mem_allocated,
            gpu_freq_mhz: self.gpu_freq_mhz,
            gpu_temp_c: self.gpu_temp_c,
            gpu_throttle: self.gpu_throttle,
            cpu_freq_mhz: self.cpu_freq_mhz,
            ecpu_freq_mhz: self.ecpu_freq_mhz,
            pcpu_freq_mhz: self.pcpu_freq_mhz,
            gpu_power_w: self.gpu_power_w,
            cpu_power_w: self.cpu_power_w,
            dram_power_w: self.dram_power_w,
            ane_power_w: self.ane_power_w,
            ecpu_power_w: self.ecpu_power_w,
            pcpu_power_w: self.pcpu_power_w,
            command_gpu: self.command_gpu,
            gpu_sram_power_w: self.gpu_sram_power_w,
            mem_wired: self.mem_wired,
            mem_compressed: self.mem_compressed,
            mem_swap: self.mem_swap,
            mem_pressure: self.mem_pressure,
        }
    }
}

pub fn wait_status_to_exit_code(status: i32) -> i32 {
    #[cfg(unix)]
    {
        if libc::WIFSTOPPED(status) {
            return libc::WSTOPSIG(status) + 128;
        }
        if libc::WIFSIGNALED(status) {
            return libc::WTERMSIG(status) + 128;
        }
        if libc::WIFEXITED(status) {
            return libc::WEXITSTATUS(status);
        }
        return 1;
    }
    #[cfg(not(unix))]
    {
        status
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_snapshot(gpu: f64, cpu: f64, memory: f64) -> MetricSnapshot {
        MetricSnapshot {
            gpu,
            cpu,
            memory,
            ..Default::default()
        }
    }

    #[test]
    fn basics_only_snapshot_records_core_metrics() {
        let mut stats = RunStats::default();
        stats.record(&minimal_snapshot(10.0, 20.0, 30.0));
        stats.record(&minimal_snapshot(30.0, 40.0, 50.0));
        assert_eq!(stats.gpu.samples, 2);
        assert_eq!(stats.cpu.samples, 2);
        assert_eq!(stats.memory.samples, 2);
        assert_eq!(stats.gpu_renderer.samples, 0);
        assert_eq!(stats.gpu_freq_mhz.samples, 0);
        assert_eq!(stats.command_gpu.samples, 0);
        assert!(stats.has_gpu_samples());
    }

    #[test]
    fn optional_extended_fields_stay_empty_when_unavailable() {
        let mut stats = RunStats::default();
        stats.record(&minimal_snapshot(5.0, 5.0, 5.0));
        assert_eq!(stats.gpu_throttle.samples, 0);
        assert_eq!(stats.gpu_mem_in_use.samples, 0);
        assert_eq!(stats.gpu_power_w.samples, 0);
    }
}
