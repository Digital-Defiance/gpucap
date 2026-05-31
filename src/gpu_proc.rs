use core_foundation::base::TCFType;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use crate::cf_utils::{cfarray_count, cfarray_get, CVoidRef};
use std::ffi::c_void;

type IOReturn = i32;
type MachPort = u32;

const K_IO_MAIN_PORT_DEFAULT: MachPort = 0;
const K_IO_SERVICE_PLANE: *const i8 = c"IOService".as_ptr();

extern "C" {
    fn IOServiceMatching(name: *const i8) -> *mut c_void;
    fn IOServiceGetMatchingServices(main_port: MachPort, matching: *mut c_void, iter: *mut u32)
        -> IOReturn;
    fn IOIteratorNext(iterator: u32) -> u32;
    fn IORegistryEntryGetChildIterator(entry: u32, plane: *const i8, iterator: *mut u32) -> IOReturn;
    fn IORegistryEntryCreateCFProperty(
        entry: u32,
        key: *const c_void,
        allocator: *const c_void,
        options: u32,
    ) -> *const c_void;
    fn IOObjectRelease(object: u32) -> IOReturn;
    fn CFGetTypeID(cf: *const c_void) -> usize;
    fn CFArrayGetTypeID() -> usize;
    fn CFDictionaryGetValue(dict: *const c_void, key: *const c_void) -> *const c_void;
}

/// Cumulative GPU time (nanoseconds) for all AGX clients owned by `pid`.
pub fn gpu_time_ns_for_pid(pid: i32) -> u64 {
    if pid <= 0 {
        return 0;
    }
    unsafe {
        let mut total = 0u64;
        let matching = IOServiceMatching(c"AGXAccelerator".as_ptr());
        if matching.is_null() {
            return 0;
        }
        let mut iter: u32 = 0;
        if IOServiceGetMatchingServices(K_IO_MAIN_PORT_DEFAULT, matching, &mut iter) != 0 {
            return 0;
        }

        loop {
            let accel = IOIteratorNext(iter);
            if accel == 0 {
                break;
            }
            total += gpu_time_ns_under_accel(accel, pid);
            IOObjectRelease(accel);
        }
        IOObjectRelease(iter);
        total
    }
}

/// Whether the AGX GPU accelerator is present in IORegistry.
pub fn agx_accelerator_available() -> bool {
    unsafe {
        let matching = IOServiceMatching(c"AGXAccelerator".as_ptr());
        if matching.is_null() {
            return false;
        }
        let mut iter: u32 = 0;
        if IOServiceGetMatchingServices(K_IO_MAIN_PORT_DEFAULT, matching, &mut iter) != 0 {
            return false;
        }
        let service = IOIteratorNext(iter);
        let found = service != 0;
        if service != 0 {
            IOObjectRelease(service);
        }
        IOObjectRelease(iter);
        found
    }
}

unsafe fn gpu_time_ns_under_accel(accel: u32, pid: i32) -> u64 {
    let mut child_iter: u32 = 0;
    if IORegistryEntryGetChildIterator(accel, K_IO_SERVICE_PLANE, &mut child_iter) != 0 {
        return 0;
    }

    let mut total = 0u64;
    loop {
        let child = IOIteratorNext(child_iter);
        if child == 0 {
            break;
        }
        if client_pid(child) == Some(pid) {
            total += app_usage_gpu_time_ns(child);
        }
        IOObjectRelease(child);
    }
    IOObjectRelease(child_iter);
    total
}

unsafe fn client_pid(service: u32) -> Option<i32> {
    let key = CFString::new("IOUserClientCreator");
    let prop = IORegistryEntryCreateCFProperty(
        service,
        key.as_CFTypeRef(),
        std::ptr::null(),
        0,
    );
    if prop.is_null() {
        return None;
    }
    let creator = CFString::wrap_under_create_rule(prop as *const _);
    parse_creator_pid(&creator.to_string())
}

fn parse_creator_pid(text: &str) -> Option<i32> {
    let rest = text.strip_prefix("pid ")?;
    let pid_str = rest.split(',').next()?.trim();
    pid_str.parse().ok()
}

unsafe fn app_usage_gpu_time_ns(service: u32) -> u64 {
    let key = CFString::new("AppUsage");
    let prop = IORegistryEntryCreateCFProperty(
        service,
        key.as_CFTypeRef(),
        std::ptr::null(),
        0,
    ) as CVoidRef;
    if prop.is_null() || CFGetTypeID(prop) != CFArrayGetTypeID() {
        return 0;
    }

    let mut total = 0u64;
    let count = cfarray_count(prop);
    for i in 0..count {
        let entry = cfarray_get(prop, i);
        if entry.is_null() {
            continue;
        }
        total += read_accumulated_gpu_time_ns(entry);
    }
    total
}

unsafe fn read_accumulated_gpu_time_ns(entry: CVoidRef) -> u64 {
    let key = CFString::new("accumulatedGPUTime");
    let value = CFDictionaryGetValue(entry, key.as_CFTypeRef());
    if value.is_null() {
        return 0;
    }
    let number = CFNumber::wrap_under_get_rule(value as *const _);
    number.to_i64().unwrap_or(0).max(0) as u64
}

pub struct GpuProcessTracker {
    pid: i32,
    prev_ns: u64,
    primed: bool,
}

impl GpuProcessTracker {
    pub fn new(pid: i32) -> Option<Self> {
        if pid <= 0 || !agx_accelerator_available() {
            return None;
        }
        Some(Self {
            pid,
            prev_ns: gpu_time_ns_for_pid(pid),
            primed: false,
        })
    }

    pub fn sample(&mut self, dt_secs: f64) -> Option<f64> {
        if self.pid <= 0 || dt_secs <= 0.0 {
            return None;
        }
        let current = gpu_time_ns_for_pid(self.pid);
        if !self.primed {
            self.prev_ns = current;
            self.primed = true;
            return Some(0.0);
        }
        let delta = current.saturating_sub(self.prev_ns);
        self.prev_ns = current;
        Some((delta as f64 / (dt_secs * 1_000_000_000.0)) * 100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_creator_pid_from_ioregistry_string() {
        assert_eq!(parse_creator_pid("pid 4242, bgpucap"), Some(4242));
        assert_eq!(parse_creator_pid("pid 42"), Some(42));
        assert_eq!(parse_creator_pid("invalid"), None);
    }

    #[test]
    #[ignore = "requires AGX IORegistry clients on Apple Silicon"]
    fn gpu_time_ns_for_self_on_host() {
        if std::env::consts::OS != "macos" {
            return;
        }
        let pid = std::process::id() as i32;
        let _ = gpu_time_ns_for_pid(pid);
    }
}
