use core_foundation::base::TCFType;
use core_foundation::base::CFType;
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use std::ffi::c_void;

type IOReturn = i32;
type MachPort = u32;

const K_IO_MAIN_PORT_DEFAULT: MachPort = 0;

extern "C" {
    fn IOServiceMatching(name: *const i8) -> *mut c_void;
    fn IOServiceGetMatchingService(main_port: MachPort, matching: *mut c_void) -> u32;
    fn IORegistryEntryCreateCFProperty(
        entry: u32,
        key: *const c_void,
        allocator: *const c_void,
        options: u32,
    ) -> *const c_void;
    fn IOObjectRelease(object: u32) -> IOReturn;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct GpuIoKitStats {
    pub device_util_pct: Option<f64>,
    pub renderer_util_pct: Option<f64>,
    pub tiler_util_pct: Option<f64>,
    pub mem_in_use_bytes: Option<u64>,
    pub mem_allocated_bytes: Option<u64>,
}

pub fn gpu_iokit_stats() -> Option<GpuIoKitStats> {
    for class in ["IOAccelerator", "AGXAccelerator"] {
        if let Some(stats) = read_performance_statistics(class) {
            return Some(stats);
        }
    }
    None
}

pub fn gpu_utilization_iokit() -> Option<f64> {
    gpu_iokit_stats().and_then(|s| s.device_util_pct)
}

fn read_performance_statistics(class_name: &str) -> Option<GpuIoKitStats> {
    unsafe {
        let class_cstr = std::ffi::CString::new(class_name).ok()?;
        let matching = IOServiceMatching(class_cstr.as_ptr());
        if matching.is_null() {
            return None;
        }
        let service = IOServiceGetMatchingService(K_IO_MAIN_PORT_DEFAULT, matching);
        if service == 0 {
            return None;
        }

        let key = CFString::new("PerformanceStatistics");
        let prop = IORegistryEntryCreateCFProperty(
            service,
            key.as_CFTypeRef(),
            std::ptr::null(),
            0,
        );
        IOObjectRelease(service);

        if prop.is_null() {
            return None;
        }

        let dict = CFDictionary::<CFString, CFType>::wrap_under_create_rule(prop as *const _);
        Some(GpuIoKitStats {
            device_util_pct: read_f64(&dict, "Device Utilization %"),
            renderer_util_pct: read_f64(&dict, "Renderer Utilization %"),
            tiler_util_pct: read_f64(&dict, "Tiler Utilization %"),
            mem_in_use_bytes: read_u64(&dict, "In use system memory"),
            mem_allocated_bytes: read_u64(&dict, "Alloc system memory"),
        })
    }
}

fn read_f64(dict: &CFDictionary<CFString, CFType>, key: &str) -> Option<f64> {
    let cf_key = CFString::new(key);
    let value = dict.find(cf_key)?;
    unsafe {
        let number = CFNumber::wrap_under_get_rule(value.as_CFTypeRef() as *const _);
        number.to_f64()
    }
}

fn read_u64(dict: &CFDictionary<CFString, CFType>, key: &str) -> Option<u64> {
    read_f64(dict, key).map(|v| v as u64)
}
