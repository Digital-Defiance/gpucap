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

pub fn gpu_utilization_iokit() -> Option<f64> {
    for class in ["IOAccelerator", "AGXAccelerator"] {
        if let Some(util) = read_device_utilization(class) {
            return Some(util);
        }
    }
    None
}

fn read_device_utilization(class_name: &str) -> Option<f64> {
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
        let util_key = CFString::new("Device Utilization %");
        let value = dict.find(util_key)?;
        let number = CFNumber::wrap_under_get_rule(value.as_CFTypeRef() as *const _);
        number.to_f64()
    }
}
