//! Stable embed API for BrightVision Test Lab and other Rust/Python consumers.
//!
//! Full step timing on non–Apple Silicon hosts should use **`btime`** only (no bgpucap).

use crate::json::write_snapshot_json;
use crate::metrics::{MetricSnapshot, SampleTier, Sampler};
use crate::platform::{check_apple_silicon, ChipProfile};

/// Whether this process can sample GPU/CPU/memory via bgpucap (macOS arm64 Apple CPU).
pub fn platform_supported() -> bool {
    check_apple_silicon().is_ok()
}

/// One system snapshot (no subprocess). Errors on unsupported platforms.
pub fn sample_system(tier: SampleTier) -> Result<MetricSnapshot, String> {
    check_apple_silicon()?;
    let mut sampler = Sampler::with_tier(tier);
    Ok(sampler.sample())
}

/// JSON object for a single snapshot (`schema` + `chip` + `metrics`), stdout-friendly.
pub fn snapshot_json(
    snapshot: &MetricSnapshot,
    filter: &crate::metrics_filter::MetricFilter,
) -> Result<String, String> {
    check_apple_silicon()?;
    let chip = ChipProfile::detect();
    let mut buf = Vec::new();
    write_snapshot_json(&mut buf, snapshot, &chip, filter)
        .map_err(|e| e.to_string())?;
    Ok(String::from_utf8(buf).map_err(|e| e.to_string())?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_schema_is_documented() {
        assert_eq!(crate::json::REPORT_SCHEMA, "1");
    }

    #[test]
    fn platform_supported_matches_check() {
        assert_eq!(platform_supported(), check_apple_silicon().is_ok());
    }

    #[test]
    fn sample_system_on_supported_host() {
        if !platform_supported() {
            return;
        }
        let snap = sample_system(SampleTier::Basic).expect("sample");
        assert!(snap.memory >= 0.0 && snap.memory <= 100.0);
    }
}
