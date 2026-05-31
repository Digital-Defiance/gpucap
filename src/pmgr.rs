use core_foundation::base::TCFType;
use core_foundation::data::CFData;
use std::ffi::c_void;

type IOReturn = i32;
type MachPort = u32;

const K_IO_MAIN_PORT_DEFAULT: MachPort = 0;

extern "C" {
    fn IOServiceMatching(name: *const i8) -> *mut c_void;
    fn IOServiceGetMatchingServices(main_port: MachPort, matching: *mut c_void, iter: *mut u32)
        -> IOReturn;
    fn IOIteratorNext(iterator: u32) -> u32;
    fn IOObjectRelease(object: u32) -> IOReturn;
    fn IORegistryEntryGetName(entry: u32, name: *mut i8) -> IOReturn;
    fn IORegistryEntryCreateCFProperty(
        entry: u32,
        key: *const c_void,
        allocator: *const c_void,
        options: u32,
    ) -> *const c_void;
}

#[derive(Debug, Clone, Default)]
pub struct DvfsTables {
    pub gpu_mhz: Vec<u32>,
    pub pcpu_mhz: Vec<u32>,
    pub ecpu_mhz: Vec<u32>,
}

pub fn load_dvfs_tables() -> DvfsTables {
    let mut tables = DvfsTables::default();
    unsafe {
        let matching = IOServiceMatching(c"AppleARMIODevice".as_ptr());
        if matching.is_null() {
            return tables;
        }
        let mut iter: u32 = 0;
        if IOServiceGetMatchingServices(K_IO_MAIN_PORT_DEFAULT, matching, &mut iter) != 0 {
            return tables;
        }

        loop {
            let service = IOIteratorNext(iter);
            if service == 0 {
                break;
            }

            let mut name = [0i8; 128];
            if IORegistryEntryGetName(service, name.as_mut_ptr()) != 0 {
                IOObjectRelease(service);
                continue;
            }
            let name = std::ffi::CStr::from_ptr(name.as_ptr())
                .to_string_lossy()
                .into_owned();
            if name != "pmgr" {
                IOObjectRelease(service);
                continue;
            }

            tables.gpu_mhz = read_voltage_states(service, "voltage-states9");
            tables.pcpu_mhz = read_voltage_states(service, "voltage-states8");
            tables.ecpu_mhz = discover_ecpu_table(service, tables.pcpu_mhz.first().copied());
            IOObjectRelease(service);
            break;
        }
        IOObjectRelease(iter);
    }
    tables
}

unsafe fn read_voltage_states(service: u32, key: &str) -> Vec<u32> {
    let cf_key = core_foundation::string::CFString::new(key);
    let prop = IORegistryEntryCreateCFProperty(
        service,
        cf_key.as_CFTypeRef(),
        std::ptr::null(),
        0,
    );
    if prop.is_null() {
        return Vec::new();
    }
    let data = CFData::wrap_under_create_rule(prop as *const _);
    parse_voltage_states_data(data.bytes())
}

unsafe fn discover_ecpu_table(service: u32, pcpu_min_mhz: Option<u32>) -> Vec<u32> {
    for idx in 0..32 {
        if idx == 8 || idx == 9 {
            continue;
        }
        let key = format!("voltage-states{idx}");
        let table = read_voltage_states(service, &key);
        if table.is_empty() {
            continue;
        }
        if table.iter().all(|&mhz| mhz >= 100)
            && pcpu_min_mhz.is_none_or(|pmin| table[0] < pmin)
        {
            return table;
        }
    }
    Vec::new()
}

fn parse_voltage_states_data(bytes: &[u8]) -> Vec<u32> {
    let mut out = Vec::new();
    let mut off = 0;
    while off + 7 < bytes.len() {
        let freq_hz = u32::from_le_bytes(bytes[off..off + 4].try_into().unwrap());
        if freq_hz > 0 {
            out.push(freq_hz / 1_000_000);
        }
        off += 8;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_voltage_states_bytes() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1_338_000_000u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&618_000_000u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        assert_eq!(parse_voltage_states_data(&bytes), vec![1338, 618]);
    }

    #[test]
    #[ignore = "requires Apple Silicon pmgr device"]
    fn load_dvfs_on_host() {
        if std::env::consts::OS != "macos" {
            return;
        }
        let tables = load_dvfs_tables();
        assert!(
            !tables.gpu_mhz.is_empty(),
            "expected GPU DVFS table from pmgr"
        );
        eprintln!("gpu_mhz={:?}", tables.gpu_mhz);
        eprintln!("pcpu_mhz={:?}", tables.pcpu_mhz);
        eprintln!("ecpu_mhz={:?}", tables.ecpu_mhz);
    }
}
