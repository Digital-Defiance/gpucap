#![allow(deprecated)]

use libc::{
    c_int, host_processor_info, host_statistics64, mach_host_self, mach_msg_type_number_t,
    natural_t, processor_info_array_t, vm_statistics64, HOST_VM_INFO64, HOST_VM_INFO64_COUNT,
    PROCESSOR_CPU_LOAD_INFO,
};
use std::mem;

use crate::iokit::{gpu_iokit_stats, gpu_utilization_iokit, GpuIoKitStats};
use crate::cf_utils::CVoidRef;
use crate::ioreport::{
    create_cpu_subscription, create_energy_subscription, create_gpu_subscription, create_delta,
    create_sample, parse_blended_cpu_freq_mhz, parse_energy_power_w, parse_extended_cpu,
    parse_extended_gpu, parse_gpu_utilization, release_sample, IoReportExtendedSample,
    IOReportSubscription,
};
use std::time::Instant;
use crate::platform::memory_pressure_level;
use crate::pmgr::{load_dvfs_tables, DvfsTables};

#[derive(Debug, Clone, Default)]
struct CpuTicks {
    user: u64,
    system: u64,
    idle: u64,
    nice: u64,
}

impl CpuTicks {
    fn total(&self) -> u64 {
        self.user + self.system + self.idle + self.nice
    }

    fn active(&self) -> u64 {
        self.user + self.system + self.nice
    }
}

pub struct CpuTracker {
    prev: Vec<CpuTicks>,
}

impl CpuTracker {
    pub fn new() -> Self {
        Self {
            prev: read_cpu_ticks(),
        }
    }

    pub fn sample(&mut self) -> f64 {
        let current = read_cpu_ticks();
        let mut total_active: u64 = 0;
        let mut total_all: u64 = 0;

        for (curr, prev) in current.iter().zip(self.prev.iter()) {
            total_active += curr.active().saturating_sub(prev.active());
            total_all += curr.total().saturating_sub(prev.total());
        }

        self.prev = current;

        if total_all > 0 {
            (total_active as f64 / total_all as f64) * 100.0
        } else {
            0.0
        }
    }
}

fn read_cpu_ticks() -> Vec<CpuTicks> {
    unsafe {
        let mut num_cpus: natural_t = 0;
        let mut cpu_info: processor_info_array_t = std::ptr::null_mut();
        let mut info_count: mach_msg_type_number_t = 0;

        let kr = host_processor_info(
            mach_host_self(),
            PROCESSOR_CPU_LOAD_INFO as c_int,
            &mut num_cpus,
            &mut cpu_info,
            &mut info_count,
        );

        if kr != 0 || cpu_info.is_null() {
            return Vec::new();
        }

        let mut ticks = Vec::with_capacity(num_cpus as usize);
        for i in 0..num_cpus as isize {
            let base = i * 4;
            ticks.push(CpuTicks {
                user: *cpu_info.offset(base) as u64,
                system: *cpu_info.offset(base + 1) as u64,
                idle: *cpu_info.offset(base + 2) as u64,
                nice: *cpu_info.offset(base + 3) as u64,
            });
        }

        extern "C" {
            fn vm_deallocate(target_task: u32, address: usize, size: usize) -> i32;
            fn mach_task_self() -> u32;
        }
        vm_deallocate(
            mach_task_self(),
            cpu_info as usize,
            info_count as usize * mem::size_of::<i32>(),
        );

        ticks
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct HostMemoryDetail {
    pub used_pct: f64,
    pub wired_bytes: u64,
    pub compressed_bytes: u64,
    pub swap_used_bytes: u64,
    pub pressure: u8,
}

pub fn host_memory_detail() -> HostMemoryDetail {
    let total = total_memory();
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) as u64 };
    let mut vm_stat: vm_statistics64 = unsafe { mem::zeroed() };
    let mut count: mach_msg_type_number_t = HOST_VM_INFO64_COUNT as _;

    let kr = unsafe {
        host_statistics64(
            mach_host_self(),
            HOST_VM_INFO64 as c_int,
            &mut vm_stat as *mut vm_statistics64 as *mut _,
            &mut count,
        )
    };

    if kr != 0 || total == 0 {
        return HostMemoryDetail {
            pressure: memory_pressure_level(),
            ..Default::default()
        };
    }

    let internal = vm_stat.internal_page_count as u64;
    let purgeable = vm_stat.purgeable_count as u64;
    let wired = vm_stat.wire_count as u64 * page_size;
    let compressor = vm_stat.compressor_page_count as u64 * page_size;
    let swap = swap_used_bytes();

    let app_memory = internal.saturating_sub(purgeable);
    let used = (app_memory + vm_stat.wire_count as u64 + vm_stat.compressor_page_count as u64)
        * page_size;

    HostMemoryDetail {
        used_pct: (used.min(total) as f64 / total as f64) * 100.0,
        wired_bytes: wired,
        compressed_bytes: compressor,
        swap_used_bytes: swap,
        pressure: memory_pressure_level(),
    }
}

fn swap_used_bytes() -> u64 {
    #[repr(C)]
    struct XswUsage {
        total: u64,
        avail: u64,
        used: u64,
        pagesize: u32,
        encrypted: u32,
    }

    let mut usage = XswUsage {
        total: 0,
        avail: 0,
        used: 0,
        pagesize: 0,
        encrypted: 0,
    };
    let mut len = std::mem::size_of::<XswUsage>();
    let name = c"vm.swapusage";
    unsafe {
        if libc::sysctlbyname(
            name.as_ptr(),
            &mut usage as *mut XswUsage as *mut _,
            &mut len,
            std::ptr::null_mut(),
            0,
        ) != 0
        {
            return 0;
        }
    }
    usage.used
}

fn total_memory() -> u64 {
    let mut size: u64 = 0;
    let mut len = mem::size_of::<u64>();
    let name = c"hw.memsize";
    unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            &mut size as *mut u64 as *mut _,
            &mut len,
            std::ptr::null_mut(),
            0,
        );
    }
    size
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PercentStats {
    pub avg: f64,
    pub peak: f64,
    pub samples: u32,
}

impl PercentStats {
    pub fn record(&mut self, value: f64) {
        if !value.is_finite() {
            return;
        }
        if self.samples == 0 {
            self.avg = value;
            self.peak = value;
        } else {
            let n = self.samples as f64;
            self.avg = (self.avg * n + value) / (n + 1.0);
            if value > self.peak {
                self.peak = value;
            }
        }
        self.samples += 1;
    }

    pub fn record_option(&mut self, value: Option<f64>) {
        if let Some(v) = value {
            self.record(v);
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ScalarStats {
    pub avg: f64,
    pub peak: f64,
    pub samples: u32,
}

impl ScalarStats {
    pub fn record(&mut self, value: f64) {
        if !value.is_finite() {
            return;
        }
        if self.samples == 0 {
            self.avg = value;
            self.peak = value;
        } else {
            let n = self.samples as f64;
            self.avg = (self.avg * n + value) / (n + 1.0);
            if value > self.peak {
                self.peak = value;
            }
        }
        self.samples += 1;
    }

    pub fn record_option(&mut self, value: Option<f64>) {
        if let Some(v) = value {
            self.record(v);
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ThrottleStats {
    pub throttled_samples: u32,
    pub samples: u32,
}

impl ThrottleStats {
    pub fn record(&mut self, throttled: bool) {
        if throttled {
            self.throttled_samples += 1;
        }
        self.samples += 1;
    }

    pub fn pct(&self) -> f64 {
        if self.samples == 0 {
            0.0
        } else {
            (self.throttled_samples as f64 / self.samples as f64) * 100.0
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct MetricSnapshot {
    pub gpu: f64,
    pub cpu: f64,
    pub memory: f64,
    pub gpu_renderer: Option<f64>,
    pub gpu_tiler: Option<f64>,
    pub gpu_mem_in_use: Option<u64>,
    pub gpu_mem_allocated: Option<u64>,
    pub gpu_freq_mhz: Option<f64>,
    pub gpu_temp_c: Option<f64>,
    pub gpu_throttled: Option<bool>,
    pub cpu_freq_mhz: Option<f64>,
    pub ecpu_freq_mhz: Option<f64>,
    pub pcpu_freq_mhz: Option<f64>,
    pub gpu_power_w: Option<f64>,
    pub cpu_power_w: Option<f64>,
    pub dram_power_w: Option<f64>,
    pub ane_power_w: Option<f64>,
    pub ecpu_power_w: Option<f64>,
    pub pcpu_power_w: Option<f64>,
    pub gpu_sram_power_w: Option<f64>,
    pub command_gpu: Option<f64>,
    pub mem_wired_bytes: u64,
    pub mem_compressed_bytes: u64,
    pub mem_swap_bytes: u64,
    pub mem_pressure: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SampleTier {
    /// GPU/CPU/memory % only; skips IOReport energy/freq and IOKit extended fields.
    Basic,
    #[default]
    Full,
}

pub struct Sampler {
    tier: SampleTier,
    cpu: CpuTracker,
    gpu_report: Option<IOReportSubscription>,
    cpu_report: Option<IOReportSubscription>,
    energy_report: Option<IOReportSubscription>,
    prev_gpu_util_sample: CVoidRef,
    prev_gpu_ext_sample: CVoidRef,
    prev_cpu_sample: CVoidRef,
    prev_energy_sample: CVoidRef,
    last_sample_at: Option<Instant>,
    gpu_source: GpuSource,
    dvfs: DvfsTables,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GpuSource {
    None,
    IoReport,
    IoKit,
}

impl Sampler {
    pub fn new() -> Self {
        Self::with_tier(SampleTier::Full)
    }

    pub fn with_tier(tier: SampleTier) -> Self {
        let gpu_report = if tier == SampleTier::Full || gpu_utilization_iokit().is_none() {
            create_gpu_subscription()
        } else {
            None
        };
        let cpu_report = if tier == SampleTier::Full {
            create_cpu_subscription()
        } else {
            None
        };
        let energy_report = if tier == SampleTier::Full {
            create_energy_subscription()
        } else {
            None
        };
        let gpu_source = if gpu_utilization_iokit().is_some() {
            GpuSource::IoKit
        } else if gpu_report.is_some() {
            GpuSource::IoReport
        } else {
            GpuSource::None
        };

        let dvfs = if tier == SampleTier::Full {
            load_dvfs_tables()
        } else {
            DvfsTables::default()
        };

        let mut sampler = Self {
            tier,
            cpu: CpuTracker::new(),
            gpu_report,
            cpu_report,
            energy_report,
            prev_gpu_util_sample: std::ptr::null(),
            prev_gpu_ext_sample: std::ptr::null(),
            prev_cpu_sample: std::ptr::null(),
            prev_energy_sample: std::ptr::null(),
            last_sample_at: None,
            gpu_source,
            dvfs,
        };

        if sampler.gpu_source == GpuSource::IoReport {
            if let Some(sub) = sampler.gpu_report.as_ref() {
                sampler.prev_gpu_util_sample = create_sample(sub);
            }
        }
        if tier == SampleTier::Full {
            if let Some(sub) = sampler.gpu_report.as_ref() {
                sampler.prev_gpu_ext_sample = create_sample(sub);
            }
            if let Some(sub) = sampler.cpu_report.as_ref() {
                sampler.prev_cpu_sample = create_sample(sub);
            }
            if let Some(sub) = sampler.energy_report.as_ref() {
                sampler.prev_energy_sample = create_sample(sub);
            }
        }

        sampler
    }

    pub fn tier(&self) -> SampleTier {
        self.tier
    }

    pub fn sample(&mut self) -> MetricSnapshot {
        let cpu = self.cpu.sample();
        let mem = host_memory_detail();
        let gpu = self.sample_gpu_util_basic();
        let iokit = if self.tier == SampleTier::Full {
            gpu_iokit_stats().unwrap_or_default()
        } else {
            GpuIoKitStats {
                device_util_pct: gpu_utilization_iokit(),
                ..Default::default()
            }
        };
        let gpu = if iokit.device_util_pct.is_some() {
            iokit.device_util_pct.unwrap_or(gpu)
        } else {
            gpu
        };
        let extended = if self.tier == SampleTier::Full {
            self.sample_ioreport_extended()
        } else {
            IoReportExtendedSample::default()
        };

        MetricSnapshot {
            gpu,
            cpu,
            memory: mem.used_pct,
            gpu_renderer: if self.tier == SampleTier::Full {
                iokit.renderer_util_pct
            } else {
                None
            },
            gpu_tiler: if self.tier == SampleTier::Full {
                iokit.tiler_util_pct
            } else {
                None
            },
            gpu_mem_in_use: if self.tier == SampleTier::Full {
                iokit.mem_in_use_bytes
            } else {
                None
            },
            gpu_mem_allocated: if self.tier == SampleTier::Full {
                iokit.mem_allocated_bytes
            } else {
                None
            },
            gpu_freq_mhz: extended.gpu_freq_mhz,
            gpu_temp_c: extended.gpu_temp_c,
            gpu_throttled: extended
                .gpu_throttle_sampled
                .then_some(extended.gpu_throttled),
            cpu_freq_mhz: extended.cpu_freq_mhz,
            ecpu_freq_mhz: extended.cpu_clusters.ecpu_freq_mhz,
            pcpu_freq_mhz: extended.cpu_clusters.pcpu_freq_mhz,
            gpu_power_w: extended.gpu_power_w,
            cpu_power_w: extended.cpu_power_w,
            dram_power_w: extended.dram_power_w,
            ane_power_w: extended.ane_power_w,
            ecpu_power_w: extended.ecpu_power_w,
            pcpu_power_w: extended.pcpu_power_w,
            gpu_sram_power_w: extended.gpu_sram_power_w,
            command_gpu: None,
            mem_wired_bytes: mem.wired_bytes,
            mem_compressed_bytes: mem.compressed_bytes,
            mem_swap_bytes: mem.swap_used_bytes,
            mem_pressure: mem.pressure,
        }
    }
}

impl Drop for Sampler {
    fn drop(&mut self) {
        release_sample(self.prev_gpu_util_sample);
        release_sample(self.prev_gpu_ext_sample);
        release_sample(self.prev_cpu_sample);
        release_sample(self.prev_energy_sample);
    }
}

impl Sampler {
    fn sample_gpu_util_basic(&mut self) -> f64 {
        if let Some(util) = gpu_utilization_iokit() {
            return util;
        }
        match self.gpu_source {
            GpuSource::IoReport => self.sample_gpu_ioreport_util(),
            GpuSource::IoKit | GpuSource::None => 0.0,
        }
    }

    fn sample_gpu_ioreport_util(&mut self) -> f64 {
        let Some(sub) = self.gpu_report.as_ref() else {
            return 0.0;
        };
        let current = create_sample(sub);
        if current.is_null() {
            return 0.0;
        }
        if self.prev_gpu_util_sample.is_null() {
            self.prev_gpu_util_sample = current;
            return 0.0;
        }
        let delta = create_delta(self.prev_gpu_util_sample, current);
        release_sample(self.prev_gpu_util_sample);
        self.prev_gpu_util_sample = current;
        if delta.is_null() {
            return 0.0;
        }
        let util = parse_gpu_utilization(delta).unwrap_or(0.0);
        release_sample(delta);
        util
    }

    fn sample_ioreport_extended(&mut self) -> IoReportExtendedSample {
        let mut out = IoReportExtendedSample::default();
        let now = Instant::now();
        let dt_secs = self
            .last_sample_at
            .map(|t| now.duration_since(t).as_secs_f64())
            .unwrap_or(0.0);
        self.last_sample_at = Some(now);

        if let Some(sub) = self.gpu_report.as_ref() {
            let current = create_sample(sub);
            if !current.is_null() {
                if !self.prev_gpu_ext_sample.is_null() {
                    let delta = create_delta(self.prev_gpu_ext_sample, current);
                    if !delta.is_null() {
                        let gpu_ext = parse_extended_gpu(delta, current, &self.dvfs);
                        out.gpu_freq_mhz = gpu_ext.gpu_freq_mhz;
                        out.gpu_throttled = gpu_ext.gpu_throttled;
                        out.gpu_throttle_sampled = gpu_ext.gpu_throttle_sampled;
                        out.gpu_temp_c = gpu_ext.gpu_temp_c;
                        release_sample(delta);
                    }
                }
                release_sample(self.prev_gpu_ext_sample);
                self.prev_gpu_ext_sample = current;
            }
        }

        if let Some(sub) = self.cpu_report.as_ref() {
            let current = create_sample(sub);
            if !current.is_null() {
                if !self.prev_cpu_sample.is_null() {
                    let delta = create_delta(self.prev_cpu_sample, current);
                    if !delta.is_null() {
                        let clusters = parse_extended_cpu(delta, &self.dvfs);
                        out.cpu_clusters = clusters;
                        out.cpu_freq_mhz = parse_blended_cpu_freq_mhz(&out.cpu_clusters);
                        release_sample(delta);
                    }
                }
                release_sample(self.prev_cpu_sample);
                self.prev_cpu_sample = current;
            }
        }

        if let Some(sub) = self.energy_report.as_ref() {
            let current = create_sample(sub);
            if !current.is_null() {
                if !self.prev_energy_sample.is_null() && dt_secs > 0.0 {
                    let power =
                        parse_energy_power_w(self.prev_energy_sample, current, dt_secs);
                    out.gpu_power_w = power.gpu_power_w;
                    out.cpu_power_w = power.cpu_power_w;
                    out.dram_power_w = power.dram_power_w;
                    out.ane_power_w = power.ane_power_w;
                    out.ecpu_power_w = power.ecpu_power_w;
                    out.pcpu_power_w = power.pcpu_power_w;
                    out.gpu_sram_power_w = power.gpu_sram_power_w;
                }
                release_sample(self.prev_energy_sample);
                self.prev_energy_sample = current;
            }
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_stats_tracks_avg_and_peak() {
        let mut stats = PercentStats::default();
        stats.record(10.0);
        stats.record(30.0);
        stats.record(20.0);
        assert!((stats.avg - 20.0).abs() < f64::EPSILON);
        assert!((stats.peak - 30.0).abs() < f64::EPSILON);
        assert_eq!(stats.samples, 3);
    }

    #[test]
    fn percent_stats_record_option_skips_none() {
        let mut stats = PercentStats::default();
        stats.record_option(None);
        assert_eq!(stats.samples, 0);
        stats.record_option(Some(42.0));
        assert_eq!(stats.samples, 1);
        assert!((stats.avg - 42.0).abs() < f64::EPSILON);
    }
}
