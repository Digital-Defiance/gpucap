#![allow(deprecated)]

use libc::{
    c_int, host_processor_info, host_statistics64, mach_host_self, mach_msg_type_number_t,
    natural_t, processor_info_array_t, vm_statistics64, HOST_VM_INFO64, HOST_VM_INFO64_COUNT,
    PROCESSOR_CPU_LOAD_INFO,
};
use std::mem;

use crate::iokit::gpu_utilization_iokit;
use crate::ioreport::{self, IOReportSubscription};

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

pub fn memory_usage_percent() -> f64 {
    let total = total_memory();
    if total == 0 {
        return 0.0;
    }

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

    if kr != 0 {
        return 0.0;
    }

    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) as u64 };
    let internal = vm_stat.internal_page_count as u64;
    let purgeable = vm_stat.purgeable_count as u64;
    let wired = vm_stat.wire_count as u64;
    let compressor = vm_stat.compressor_page_count as u64;

    let app_memory = internal.saturating_sub(purgeable);
    let used = (app_memory + wired + compressor) * page_size;
    (used.min(total) as f64 / total as f64) * 100.0
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
}

#[derive(Debug, Clone, Default)]
pub struct MetricSnapshot {
    pub gpu: f64,
    pub cpu: f64,
    pub memory: f64,
}

pub struct Sampler {
    cpu: CpuTracker,
    gpu_report: Option<IOReportSubscription>,
    prev_gpu_sample: crate::cf_utils::CVoidRef,
    gpu_source: GpuSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GpuSource {
    None,
    IoReport,
    IoKit,
}

impl Sampler {
    pub fn new() -> Self {
        let gpu_report = ioreport::create_gpu_subscription();
        let gpu_source = if gpu_utilization_iokit().is_some() {
            GpuSource::IoKit
        } else if gpu_report.is_some() {
            GpuSource::IoReport
        } else {
            GpuSource::None
        };

        let mut sampler = Self {
            cpu: CpuTracker::new(),
            gpu_report,
            prev_gpu_sample: std::ptr::null(),
            gpu_source,
        };

        if sampler.gpu_source == GpuSource::IoReport {
            if let Some(sub) = sampler.gpu_report.as_ref() {
                sampler.prev_gpu_sample = ioreport::create_sample(sub);
            }
        }

        sampler
    }

    pub fn sample(&mut self) -> MetricSnapshot {
        let cpu = self.cpu.sample();
        let memory = memory_usage_percent();
        let gpu = self.sample_gpu();

        MetricSnapshot { gpu, cpu, memory }
    }
}

impl Drop for Sampler {
    fn drop(&mut self) {
        ioreport::release_sample(self.prev_gpu_sample);
    }
}

impl Sampler {
    fn sample_gpu(&mut self) -> f64 {
        match self.gpu_source {
            GpuSource::IoKit => gpu_utilization_iokit().unwrap_or(0.0),
            GpuSource::IoReport => {
                let Some(sub) = self.gpu_report.as_ref() else {
                    return 0.0;
                };
                let current = ioreport::create_sample(sub);
                if self.prev_gpu_sample.is_null() {
                    self.prev_gpu_sample = current;
                    return 0.0;
                }

                let delta = ioreport::create_delta(self.prev_gpu_sample, current);
                ioreport::release_sample(self.prev_gpu_sample);
                self.prev_gpu_sample = current;

                let util = ioreport::parse_gpu_utilization(delta).unwrap_or(0.0);
                ioreport::release_sample(delta);
                util
            }
            GpuSource::None => 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PercentStats;

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
}
