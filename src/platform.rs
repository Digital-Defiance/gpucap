pub fn ensure_apple_silicon() -> Result<(), i32> {
    if let Err(reason) = check_apple_silicon() {
        eprintln!("gpucap: {reason}");
        eprintln!();
        eprintln!("gpucap is for Apple Silicon Macs only (M1, M2, M3, M4, and later).");
        eprintln!("It reads AGX GPU metrics and unified memory statistics that are not");
        eprintln!("available on Intel Macs or non-macOS platforms.");
        eprintln!();
        eprintln!("Build and run on an Apple Silicon Mac:");
        eprintln!("  cargo build --release");
        eprintln!("  gpucap -- sleep 1");
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

fn sysctl_string(name: &str) -> Option<String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apple_silicon_check_on_build_host() {
        if std::env::consts::OS == "macos" && std::env::consts::ARCH == "aarch64" {
            assert!(check_apple_silicon().is_ok());
        }
    }
}
