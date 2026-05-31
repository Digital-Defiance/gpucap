#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChipFamily {
    M1,
    M2,
    M3,
    M4,
    Unknown,
}

impl ChipFamily {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::M1 => "m1",
            Self::M2 => "m2",
            Self::M3 => "m3",
            Self::M4 => "m4",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChipProfile {
    pub brand: String,
    pub family: ChipFamily,
    /// Extended metrics validated on this exact chip variant in CI/hardware tests.
    pub validated_metrics: bool,
}

impl ChipProfile {
    pub fn detect() -> Self {
        let brand = sysctl_string("machdep.cpu.brand_string")
            .unwrap_or_default()
            .trim()
            .to_string();
        let family = detect_family(&brand);
        // Only M4 Max has been hardware-validated by the project maintainer.
        let validated_metrics = brand.contains("M4") && brand.contains("Max");
        Self {
            brand,
            family,
            validated_metrics,
        }
    }

    /// Human-readable validation status for JSON output and docs.
    pub fn validation_tier(&self) -> &'static str {
        if self.validated_metrics {
            "validated"
        } else {
            "best-effort"
        }
    }
}

fn detect_family(brand: &str) -> ChipFamily {
    if brand.contains("M4") {
        ChipFamily::M4
    } else if brand.contains("M3") {
        ChipFamily::M3
    } else if brand.contains("M2") {
        ChipFamily::M2
    } else if brand.contains("M1") {
        ChipFamily::M1
    } else {
        ChipFamily::Unknown
    }
}

pub const METRICS_FOOTNOTE: &str =
    "* Extended metrics (GPU memory, frequency, power, thermal, per-process GPU) \
     validated on Apple M4 Max; on other chips they are best-effort and \
     omitted when unavailable. GPU, CPU, and memory % always report.";

pub fn print_metrics_footnote(profile: &ChipProfile) {
    if !profile.validated_metrics {
        eprintln!("{METRICS_FOOTNOTE}");
    }
}

pub fn ensure_apple_silicon() -> Result<(), i32> {
    if let Err(reason) = check_apple_silicon() {
        eprintln!("bgpucap: {reason}");
        eprintln!();
        eprintln!("bgpucap is for Apple Silicon Macs only (M1, M2, M3, M4, and later).");
        eprintln!("It reads AGX GPU metrics and unified memory statistics that are not");
        eprintln!("available on Intel Macs or non-macOS platforms.");
        eprintln!();
        eprintln!("Build and run on an Apple Silicon Mac:");
        eprintln!("  cargo build --release");
        eprintln!("  bgpucap -- sleep 1");
        return Err(2);
    }
    Ok(())
}

pub fn check_apple_silicon() -> Result<(), String> {
    if std::env::consts::OS != "macos" {
        return Err(format!(
            "unsupported operating system '{}' (macOS required)",
            std::env::consts::OS
        ));
    }

    if std::env::consts::ARCH != "aarch64" {
        return Err(format!(
            "unsupported CPU architecture '{}' (Apple Silicon / arm64 required)",
            std::env::consts::ARCH
        ));
    }

    if let Some(brand) = sysctl_string("machdep.cpu.brand_string") {
        let brand = brand.trim();
        if !brand.contains("Apple") {
            return Err(format!(
                "expected an Apple Silicon CPU, found '{brand}'"
            ));
        }
    }

    Ok(())
}

pub(crate) fn sysctl_string(name: &str) -> Option<String> {
    let cname = std::ffi::CString::new(name).ok()?;
    let mut len = 0usize;
    unsafe {
        if libc::sysctlbyname(
            cname.as_ptr(),
            std::ptr::null_mut(),
            &mut len,
            std::ptr::null_mut(),
            0,
        ) != 0
        {
            return None;
        }
        let mut buf = vec![0u8; len];
        if libc::sysctlbyname(
            cname.as_ptr(),
            buf.as_mut_ptr() as *mut _,
            &mut len,
            std::ptr::null_mut(),
            0,
        ) != 0
        {
            return None;
        }
        if let Some(nul) = buf.iter().position(|&b| b == 0) {
            buf.truncate(nul);
        }
        String::from_utf8(buf).ok()
    }
}

fn sysctl_i32(name: &str) -> Option<i32> {
    let cname = std::ffi::CString::new(name).ok()?;
    let mut value = 0i32;
    let mut len = std::mem::size_of::<i32>();
    unsafe {
        if libc::sysctlbyname(
            cname.as_ptr(),
            &mut value as *mut i32 as *mut _,
            &mut len,
            std::ptr::null_mut(),
            0,
        ) != 0
        {
            return None;
        }
    }
    Some(value)
}

pub fn memory_pressure_level() -> u8 {
    match sysctl_i32("vm.memory_pressure") {
        Some(0) => 0,
        Some(1) => 1,
        Some(n) if n >= 2 => 2,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apple_silicon_check_on_build_host() {
        if std::env::consts::OS == "macos" && std::env::consts::ARCH == "aarch64" {
            assert!(check_apple_silicon().is_ok());
        }
    }

    #[test]
    fn chip_profile_detects_brand() {
        if std::env::consts::OS != "macos" {
            return;
        }
        let profile = ChipProfile::detect();
        if profile.brand.is_empty() {
            eprintln!("skipping chip_profile_detects_brand (sysctl unavailable)");
            return;
        }
        assert!(profile.brand.contains("Apple"));
    }

    #[test]
    fn m4_max_is_validated() {
        let profile = ChipProfile {
            brand: "Apple M4 Max".into(),
            family: ChipFamily::M4,
            validated_metrics: true,
        };
        assert!(profile.validated_metrics);
        assert_eq!(profile.validation_tier(), "validated");
    }
}
