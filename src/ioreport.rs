use crate::cf_utils::{CVoidRef, *};
use crate::pmgr::DvfsTables;
use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use std::ffi::c_void;

#[link(name = "IOReport")]
extern "C" {
    fn IOReportCopyChannelsInGroup(
        group: CVoidRef,
        subgroup: CVoidRef,
        a: u64,
        b: u64,
        c: u64,
    ) -> CVoidRef;
    fn IOReportCreateSubscription(
        a: CVoidRef,
        channels: CVoidRef,
        out: *mut CVoidRef,
        d: u64,
        e: CVoidRef,
    ) -> CVoidRef;
    fn IOReportCreateSamples(
        subscription: CVoidRef,
        channels: CVoidRef,
        b: CVoidRef,
    ) -> CVoidRef;
    fn IOReportCreateSamplesDelta(s1: CVoidRef, s2: CVoidRef, a: CVoidRef) -> CVoidRef;
    fn IOReportChannelGetGroup(channel: CVoidRef) -> CVoidRef;
    fn IOReportChannelGetSubGroup(channel: CVoidRef) -> CVoidRef;
    fn IOReportChannelGetChannelName(channel: CVoidRef) -> CVoidRef;
    fn IOReportStateGetCount(channel: CVoidRef) -> i32;
    fn IOReportStateGetNameForIndex(channel: CVoidRef, idx: i32) -> CVoidRef;
    fn IOReportStateGetResidency(channel: CVoidRef, idx: i32) -> i64;
    fn IOReportSimpleGetIntegerValue(entry: CVoidRef, a: CVoidRef) -> i64;
    fn IOReportChannelGetFormat(channel: CVoidRef) -> i32;
    fn IOReportChannelGetUnit(channel: CVoidRef) -> u64;
}

extern "C" {
    fn CFRelease(cf: *const c_void);
    fn CFDictionaryGetCount(dict: CVoidRef) -> isize;
    fn CFDictionaryCreateMutableCopy(
        allocator: CVoidRef,
        capacity: isize,
        dict: CVoidRef,
    ) -> CVoidRef;
    fn CFDictionaryGetKeysAndValues(
        dict: CVoidRef,
        keys: *mut CVoidRef,
        values: *mut CVoidRef,
    );
    fn CFDictionaryGetValue(dict: CVoidRef, key: CVoidRef) -> CVoidRef;
    fn CFEqual(a: CVoidRef, b: CVoidRef) -> bool;
    fn CFGetTypeID(cf: CVoidRef) -> usize;
    fn CFArrayGetTypeID() -> usize;
    fn CFDictionaryGetTypeID() -> usize;
}

const CF_ALLOCATOR_DEFAULT: CVoidRef = std::ptr::null();
const IO_REPORT_FORMAT_SIMPLE: i32 = 1;
const IO_REPORT_FORMAT_STATE: i32 = 2;

pub struct IOReportSubscription {
    subscription: CVoidRef,
    channels: CVoidRef,
}

impl Drop for IOReportSubscription {
    fn drop(&mut self) {
        unsafe {
            if !self.channels.is_null() {
                CFRelease(self.channels);
            }
            if !self.subscription.is_null() {
                CFRelease(self.subscription);
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CpuClusterSample {
    pub ecpu_freq_mhz: Option<f64>,
    pub pcpu_freq_mhz: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct IoReportExtendedSample {
    pub gpu_freq_mhz: Option<f64>,
    pub gpu_throttled: bool,
    pub gpu_throttle_sampled: bool,
    pub gpu_temp_c: Option<f64>,
    pub cpu_freq_mhz: Option<f64>,
    pub cpu_clusters: CpuClusterSample,
    pub gpu_power_w: Option<f64>,
    pub cpu_power_w: Option<f64>,
    pub dram_power_w: Option<f64>,
    pub ane_power_w: Option<f64>,
    pub gpu_sram_power_w: Option<f64>,
    pub ecpu_power_w: Option<f64>,
    pub pcpu_power_w: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct EnergyPowerSample {
    pub gpu_power_w: Option<f64>,
    pub cpu_power_w: Option<f64>,
    pub dram_power_w: Option<f64>,
    pub ane_power_w: Option<f64>,
    pub gpu_sram_power_w: Option<f64>,
    pub ecpu_power_w: Option<f64>,
    pub pcpu_power_w: Option<f64>,
}

pub fn create_subscription(group: &str) -> Option<IOReportSubscription> {
    unsafe {
        let cf_group = CFString::new(group);
        let chan = IOReportCopyChannelsInGroup(
            cf_group.as_CFTypeRef(),
            std::ptr::null(),
            0,
            0,
            0,
        );
        if chan.is_null() {
            return None;
        }

        let size = CFDictionaryGetCount(chan);
        let mutable_channels = CFDictionaryCreateMutableCopy(CF_ALLOCATOR_DEFAULT, size, chan);
        CFRelease(chan);

        if mutable_channels.is_null() {
            return None;
        }

        let mut out: CVoidRef = std::ptr::null();
        let subscription = IOReportCreateSubscription(
            std::ptr::null(),
            mutable_channels,
            &mut out,
            0,
            std::ptr::null(),
        );

        if subscription.is_null() {
            CFRelease(mutable_channels);
            return None;
        }

        Some(IOReportSubscription {
            subscription,
            channels: mutable_channels,
        })
    }
}

pub fn create_gpu_subscription() -> Option<IOReportSubscription> {
    create_subscription("GPU Stats")
}

pub fn create_cpu_subscription() -> Option<IOReportSubscription> {
    create_subscription("CPU Stats")
}

pub fn create_energy_subscription() -> Option<IOReportSubscription> {
    create_subscription("Energy Model")
}

pub fn create_sample(sub: &IOReportSubscription) -> CVoidRef {
    unsafe { IOReportCreateSamples(sub.subscription, sub.channels, std::ptr::null()) }
}

pub fn create_delta(s1: CVoidRef, s2: CVoidRef) -> CVoidRef {
    unsafe { IOReportCreateSamplesDelta(s1, s2, std::ptr::null()) }
}

pub fn release_sample(sample: CVoidRef) {
    if !sample.is_null() {
        unsafe { CFRelease(sample) }
    }
}

pub fn parse_gpu_utilization(delta: CVoidRef) -> Option<f64> {
    parse_state_residency_pct(delta, "GPUPH", |name| name != "OFF")
}

pub fn parse_extended_gpu(
    delta: CVoidRef,
    current: CVoidRef,
    dvfs: &DvfsTables,
) -> IoReportExtendedSample {
    let mut out = IoReportExtendedSample::default();
    out.gpu_freq_mhz = parse_weighted_freq_mhz(delta, "GPUPH", &dvfs.gpu_mhz);
    out.gpu_throttled = parse_gpu_cltm_throttled(delta);
    out.gpu_throttle_sampled = true;
    out.gpu_temp_c = parse_gpu_temperature_max(current);
    out
}

pub fn parse_extended_cpu(delta: CVoidRef, dvfs: &DvfsTables) -> CpuClusterSample {
    let ecpu = parse_weighted_freq_mhz(delta, "ECPU", &dvfs.ecpu_mhz);
    let pcpu = parse_weighted_freq_mhz(delta, "PCPU", &dvfs.pcpu_mhz);
    CpuClusterSample {
        ecpu_freq_mhz: ecpu,
        pcpu_freq_mhz: pcpu,
    }
}

pub fn parse_blended_cpu_freq_mhz(clusters: &CpuClusterSample) -> Option<f64> {
    match (clusters.pcpu_freq_mhz, clusters.ecpu_freq_mhz) {
        (Some(p), Some(e)) => Some((p + e) / 2.0),
        (Some(p), None) => Some(p),
        (None, Some(e)) => Some(e),
        (None, None) => None,
    }
}

struct ChannelPair {
    prev: CVoidRef,
    current: CVoidRef,
}

fn extract_channel_pairs(prev: CVoidRef, current: CVoidRef) -> Vec<ChannelPair> {
    if prev.is_null() || current.is_null() {
        return Vec::new();
    }
    unsafe {
        let sn = CFDictionaryGetCount(prev);
        let sn2 = CFDictionaryGetCount(current);
        if sn <= 0 || sn2 <= 0 {
            return Vec::new();
        }

        let mut sk1 = vec![std::ptr::null(); sn as usize];
        let mut sv1 = vec![std::ptr::null(); sn as usize];
        let mut sk2 = vec![std::ptr::null(); sn2 as usize];
        let mut sv2 = vec![std::ptr::null(); sn2 as usize];
        CFDictionaryGetKeysAndValues(prev, sk1.as_mut_ptr(), sv1.as_mut_ptr());
        CFDictionaryGetKeysAndValues(current, sk2.as_mut_ptr(), sv2.as_mut_ptr());

        let array_type = CFArrayGetTypeID();
        let dict_type = CFDictionaryGetTypeID();
        let mut pairs = Vec::new();

        for tv in 0..sn2 as usize {
            let vtype = CFGetTypeID(sv2[tv]);
            if vtype == array_type {
                let arr2 = sv2[tv];
                let mut arr1 = std::ptr::null();
                for j in 0..sn as usize {
                    if CFEqual(sk1[j], sk2[tv]) && CFGetTypeID(sv1[j]) == array_type {
                        arr1 = sv1[j];
                        break;
                    }
                }
                if !arr1.is_null() {
                    append_array_pairs(&mut pairs, arr1, arr2);
                }
            } else if vtype == dict_type {
                let drivers2 = sv2[tv];
                let mut drivers1 = std::ptr::null();
                for j in 0..sn as usize {
                    if CFEqual(sk1[j], sk2[tv]) && CFGetTypeID(sv1[j]) == dict_type {
                        drivers1 = sv1[j];
                        break;
                    }
                }
                if drivers1.is_null() {
                    continue;
                }
                append_nested_driver_pairs(&mut pairs, drivers1, drivers2);
            }
        }

        pairs
    }
}

unsafe fn append_array_pairs(pairs: &mut Vec<ChannelPair>, arr1: CVoidRef, arr2: CVoidRef) {
    let mut nc = cfarray_count(arr2);
    let nc1 = cfarray_count(arr1);
    if nc1 < nc {
        nc = nc1;
    }
    for c in 0..nc {
        pairs.push(ChannelPair {
            prev: cfarray_get(arr1, c),
            current: cfarray_get(arr2, c),
        });
    }
}

unsafe fn append_nested_driver_pairs(
    pairs: &mut Vec<ChannelPair>,
    drivers1: CVoidRef,
    drivers2: CVoidRef,
) {
    let dict_type = CFDictionaryGetTypeID();
    let nd = CFDictionaryGetCount(drivers2);
    if nd <= 0 {
        return;
    }
    let mut dk = vec![std::ptr::null(); nd as usize];
    let mut dv = vec![std::ptr::null(); nd as usize];
    CFDictionaryGetKeysAndValues(drivers2, dk.as_mut_ptr(), dv.as_mut_ptr());

    for d in 0..nd as usize {
        if CFGetTypeID(dv[d]) != dict_type {
            continue;
        }
        let drv2 = dv[d];
        let drv1 = CFDictionaryGetValue(drivers1, dk[d]);
        if drv1.is_null() || CFGetTypeID(drv1) != dict_type {
            continue;
        }

        let dnk = CFDictionaryGetCount(drv2);
        if dnk <= 0 {
            continue;
        }
        let mut ddk = vec![std::ptr::null(); dnk as usize];
        let mut ddv = vec![std::ptr::null(); dnk as usize];
        CFDictionaryGetKeysAndValues(drv2, ddk.as_mut_ptr(), ddv.as_mut_ptr());

        let array_type = CFArrayGetTypeID();
        for k in 0..dnk as usize {
            if CFGetTypeID(ddv[k]) != array_type {
                continue;
            }
            let ch_arr2 = ddv[k];
            let ch_arr1 = CFDictionaryGetValue(drv1, ddk[k]);
            if !ch_arr1.is_null() && CFGetTypeID(ch_arr1) == array_type {
                append_array_pairs(pairs, ch_arr1, ch_arr2);
            }
            break;
        }
    }
}

pub fn parse_energy_power_w(
    prev: CVoidRef,
    current: CVoidRef,
    dt_secs: f64,
) -> EnergyPowerSample {
    if dt_secs <= 0.0 || prev.is_null() || current.is_null() {
        return EnergyPowerSample::default();
    }
    let pairs = extract_channel_pairs(prev, current);
    EnergyPowerSample {
        gpu_power_w: power_from_pairs(&pairs, "GPU Energy", dt_secs),
        cpu_power_w: power_from_pairs(&pairs, "CPU Energy", dt_secs),
        dram_power_w: power_from_pairs(&pairs, "DRAM", dt_secs),
        ane_power_w: power_ane_from_pairs(&pairs, dt_secs),
        gpu_sram_power_w: power_gpu_sram_from_pairs(&pairs, dt_secs),
        ecpu_power_w: power_ecpu_from_pairs(&pairs, dt_secs),
        pcpu_power_w: power_pcpu_from_pairs(&pairs, dt_secs),
    }
}

fn power_from_pairs(pairs: &[ChannelPair], channel: &str, dt_secs: f64) -> Option<f64> {
    let mut best_joules = 0.0f64;
    let mut seen = false;
    for pair in pairs {
        unsafe {
            if pair.current.is_null()
                || pair.prev.is_null()
                || !simple_channel_is(pair.current, channel)
            {
                continue;
            }
            let v2 = IOReportSimpleGetIntegerValue(pair.current, std::ptr::null());
            let v1 = IOReportSimpleGetIntegerValue(pair.prev, std::ptr::null());
            let delta = v2.saturating_sub(v1);
            if delta <= 0 {
                continue;
            }
            let unit = IOReportChannelGetUnit(pair.current);
            let joules = energy_delta_to_joules(delta, unit);
            if joules > best_joules {
                best_joules = joules;
                seen = true;
            }
        }
    }
    if !seen {
        return None;
    }
    let watts = best_joules / dt_secs;
    if !watts.is_finite() || watts < 0.0 {
        return None;
    }
    Some(watts)
}

fn power_ane_from_pairs(pairs: &[ChannelPair], dt_secs: f64) -> Option<f64> {
    if let Some(total) = power_from_pairs(pairs, "ANE", dt_secs) {
        return Some(total);
    }
    power_from_pairs_sum(pairs, is_ane_shard_channel, dt_secs)
}

fn power_gpu_sram_from_pairs(pairs: &[ChannelPair], dt_secs: f64) -> Option<f64> {
    if let Some(total) = power_from_pairs(pairs, "GPU SRAM", dt_secs) {
        return Some(total);
    }
    power_from_pairs_sum(pairs, |name| name.starts_with("GPU SRAM"), dt_secs)
}

fn power_ecpu_from_pairs(pairs: &[ChannelPair], dt_secs: f64) -> Option<f64> {
    power_from_pairs(pairs, "EACC_CPU", dt_secs)
        .or_else(|| power_from_pairs(pairs, "ECPU", dt_secs))
}

fn power_pcpu_from_pairs(pairs: &[ChannelPair], dt_secs: f64) -> Option<f64> {
    if let Some(total) = power_from_pairs(pairs, "PCPU", dt_secs) {
        return Some(total);
    }
    power_from_pairs_sum(pairs, is_pacc_cpu_cluster_channel, dt_secs)
}

fn is_pacc_cpu_cluster_channel(name: &str) -> bool {
    name.starts_with("PACC") && name.ends_with("_CPU")
}

fn is_ane_shard_channel(name: &str) -> bool {
    name.len() > 3
        && name.starts_with("ANE")
        && name.as_bytes()[3..].iter().all(|b| b.is_ascii_digit())
}

fn power_from_pairs_sum(
    pairs: &[ChannelPair],
    matches: impl Fn(&str) -> bool,
    dt_secs: f64,
) -> Option<f64> {
    let mut total_joules = 0.0f64;
    let mut seen = false;
    for pair in pairs {
        unsafe {
            if pair.current.is_null() || pair.prev.is_null() {
                continue;
            }
            if IOReportChannelGetFormat(pair.current) != IO_REPORT_FORMAT_SIMPLE {
                continue;
            }
            let name = from_cfstring(IOReportChannelGetChannelName(pair.current)).unwrap_or_default();
            if !matches(&name) {
                continue;
            }
            let v2 = IOReportSimpleGetIntegerValue(pair.current, std::ptr::null());
            let v1 = IOReportSimpleGetIntegerValue(pair.prev, std::ptr::null());
            let delta = v2.saturating_sub(v1);
            if delta <= 0 {
                continue;
            }
            let unit = IOReportChannelGetUnit(pair.current);
            total_joules += energy_delta_to_joules(delta, unit);
            seen = true;
        }
    }
    if !seen {
        return None;
    }
    let watts = total_joules / dt_secs;
    if !watts.is_finite() || watts < 0.0 {
        return None;
    }
    Some(watts)
}

fn energy_delta_to_joules(delta: i64, unit: u64) -> f64 {
    const K_IO_REPORT_QUANTITY_ENERGY: u64 = 3;
    const K_IO_REPORT_SCALE_SI_SHIFT: u32 = 32;
    const K_IO_REPORT_EXP_ZERO_OFFSET: i32 = 127;

    let quantity = (unit >> 56) & 0xff;
    if quantity != K_IO_REPORT_QUANTITY_ENERGY {
        return delta as f64 / 1_000_000_000.0;
    }
    let scale = unit & 0x00ffffffffffffff;
    let si_encoded = ((scale >> K_IO_REPORT_SCALE_SI_SHIFT) & 0xff) as i32;
    let si_exp = si_encoded - K_IO_REPORT_EXP_ZERO_OFFSET;
    delta as f64 * 10f64.powi(si_exp)
}

unsafe fn simple_channel_is(ch: CVoidRef, channel_name: &str) -> bool {
    IOReportChannelGetFormat(ch) == IO_REPORT_FORMAT_SIMPLE
        && from_cfstring(IOReportChannelGetChannelName(ch)).as_deref() == Some(channel_name)
}

fn parse_gpu_cltm_throttled(delta: CVoidRef) -> bool {
    unsafe {
        let items = cfdict_get_value(delta, "IOReportChannels");
        if items.is_null() {
            return false;
        }
        let count = cfarray_count(items);
        for i in 0..count {
            let ch = cfarray_get(items, i);
            if ch.is_null() || !channel_is(ch, "GPU_CLTM") {
                continue;
            }
            let state_count = IOReportStateGetCount(ch);
            let mut total: i64 = 0;
            let mut no_cltm: i64 = 0;
            for s in 0..state_count {
                let name = from_cfstring(IOReportStateGetNameForIndex(ch, s)).unwrap_or_default();
                let residency = IOReportStateGetResidency(ch, s);
                total += residency;
                if name == "NO_CLTM" {
                    no_cltm = residency;
                }
            }
            return total > 0 && no_cltm < total;
        }
        false
    }
}

fn parse_gpu_temperature_max(current: CVoidRef) -> Option<f64> {
    unsafe {
        let items = cfdict_get_value(current, "IOReportChannels");
        if items.is_null() {
            return None;
        }
        let count = cfarray_count(items);
        let mut max_temp = 0.0f64;
        let mut seen = false;
        for i in 0..count {
            let ch = cfarray_get(items, i);
            if ch.is_null() || IOReportChannelGetFormat(ch) != IO_REPORT_FORMAT_SIMPLE {
                continue;
            }
            let name = from_cfstring(IOReportChannelGetChannelName(ch)).unwrap_or_default();
            if !name.starts_with("Tg") || !name.ends_with(" Max") {
                continue;
            }
            let raw = IOReportSimpleGetIntegerValue(ch, std::ptr::null());
            let temp_c = decode_temperature(raw);
            if temp_c > 0.0 {
                seen = true;
                max_temp = max_temp.max(temp_c);
            }
        }
        if seen {
            Some(max_temp)
        } else {
            None
        }
    }
}

fn decode_temperature(raw: i64) -> f64 {
    if raw > 100_000 {
        raw as f64 / 1000.0
    } else if raw > 10_000 {
        raw as f64 / 100.0
    } else {
        raw as f64
    }
}

fn parse_weighted_freq_mhz(delta: CVoidRef, channel: &str, table_mhz: &[u32]) -> Option<f64> {
    if table_mhz.is_empty() {
        return None;
    }
    unsafe {
        let items = cfdict_get_value(delta, "IOReportChannels");
        if items.is_null() {
            return None;
        }
        let count = cfarray_count(items);
        for i in 0..count {
            let ch = cfarray_get(items, i);
            if ch.is_null() || !channel_is(ch, channel) {
                continue;
            }
            let state_count = IOReportStateGetCount(ch);
            let mut total: i64 = 0;
            let mut weighted = 0.0;
            for s in 0..state_count {
                let name = from_cfstring(IOReportStateGetNameForIndex(ch, s)).unwrap_or_default();
                let residency = IOReportStateGetResidency(ch, s);
                if residency <= 0 || name == "OFF" || name == "IDLE" || name == "DOWN" {
                    continue;
                }
                total += residency;
                if let Some(mhz) = lookup_state_mhz(&name, table_mhz) {
                    weighted += mhz as f64 * residency as f64;
                }
            }
            if total > 0 && weighted > 0.0 {
                return Some(weighted / total as f64);
            }
        }
        None
    }
}

fn lookup_state_mhz(state: &str, table_mhz: &[u32]) -> Option<u32> {
    if let Some(rest) = state.strip_prefix('P') {
        if let Ok(index) = rest.parse::<usize>() {
            if index >= 1 && index <= table_mhz.len() {
                return Some(table_mhz[index - 1]);
            }
        }
    }
    if let Some(rest) = state.strip_prefix('V') {
        if let Ok(index) = rest.parse::<usize>() {
            if index < table_mhz.len() {
                return Some(table_mhz[index]);
            }
        }
    }
    None
}

fn parse_state_residency_pct(
    delta: CVoidRef,
    channel_name: &str,
    is_active: impl Fn(&str) -> bool,
) -> Option<f64> {
    unsafe {
        let items = cfdict_get_value(delta, "IOReportChannels");
        if items.is_null() {
            return None;
        }
        let count = cfarray_count(items);
        for i in 0..count {
            let ch = cfarray_get(items, i);
            if ch.is_null() || !channel_is(ch, channel_name) {
                continue;
            }
            let state_count = IOReportStateGetCount(ch);
            let mut total_active: i64 = 0;
            let mut total: i64 = 0;
            for s in 0..state_count {
                let name = from_cfstring(IOReportStateGetNameForIndex(ch, s)).unwrap_or_default();
                let residency = IOReportStateGetResidency(ch, s);
                total += residency;
                if is_active(&name) {
                    total_active += residency;
                }
            }
            if total > 0 {
                return Some((total_active as f64 / total as f64) * 100.0);
            }
        }
        None
    }
}

unsafe fn channel_is(ch: CVoidRef, channel_name: &str) -> bool {
    IOReportChannelGetFormat(ch) == IO_REPORT_FORMAT_STATE
        && from_cfstring(IOReportChannelGetChannelName(ch)).as_deref() == Some(channel_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pmgr::load_dvfs_tables;
    use std::time::Duration;

    #[test]
    fn is_pacc_cpu_cluster_channel_matches() {
        assert!(is_pacc_cpu_cluster_channel("PACC0_CPU"));
        assert!(is_pacc_cpu_cluster_channel("PACC1_CPU"));
        assert!(!is_pacc_cpu_cluster_channel("PACC0_CPU0"));
        assert!(!is_pacc_cpu_cluster_channel("EACC_CPU"));
    }

    #[test]
    fn is_ane_shard_channel_matches_ane0() {
        assert!(is_ane_shard_channel("ANE0"));
        assert!(is_ane_shard_channel("ANE1"));
        assert!(!is_ane_shard_channel("ANE"));
        assert!(!is_ane_shard_channel("ANE Energy"));
    }

    #[test]
    fn energy_delta_to_joules_respects_unit_scale() {
        const K_IO_REPORT_UNIT_MJ: u64 = (3 << 56) | ((124u64) << 32);
        const K_IO_REPORT_UNIT_NJ: u64 = (3 << 56) | ((118u64) << 32);
        assert!((energy_delta_to_joules(1000, K_IO_REPORT_UNIT_MJ) - 1.0).abs() < f64::EPSILON);
        assert!((energy_delta_to_joules(1_000_000_000, K_IO_REPORT_UNIT_NJ) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn lookup_state_mhz_p_and_v() {
        let table = vec![100, 200, 300];
        assert_eq!(lookup_state_mhz("P1", &table), Some(100));
        assert_eq!(lookup_state_mhz("P3", &table), Some(300));
        assert_eq!(lookup_state_mhz("V1", &table), Some(200));
    }

    #[test]
    #[ignore = "probe energy counters on Apple Silicon"]
    fn probe_energy_counters_on_host() {
        if std::env::consts::OS != "macos" {
            return;
        }
        let Some(sub) = create_energy_subscription() else {
            eprintln!("no energy subscription");
            return;
        };
        let s1 = create_sample(&sub);
        std::thread::sleep(Duration::from_millis(500));
        let s2 = create_sample(&sub);
        let pairs = extract_channel_pairs(s1, s2);
        eprintln!("channel pairs: {}", pairs.len());
        for pair in &pairs {
            unsafe {
                if pair.current.is_null()
                    || IOReportChannelGetFormat(pair.current) != IO_REPORT_FORMAT_SIMPLE
                {
                    continue;
                }
                let name = from_cfstring(IOReportChannelGetChannelName(pair.current))
                    .unwrap_or_default();
                let v1 = IOReportSimpleGetIntegerValue(pair.prev, std::ptr::null());
                let v2 = IOReportSimpleGetIntegerValue(pair.current, std::ptr::null());
                let delta = v2.saturating_sub(v1);
                if name.contains("Energy") || name == "ANE" || name.starts_with("ANE") || name.contains("DRAM") {
                    let unit = IOReportChannelGetUnit(pair.current);
                    eprintln!("  {name}: {v1} -> {v2} (delta {delta}, unit 0x{unit:x})");
                }
            }
        }
        let power = parse_energy_power_w(s1, s2, 0.5);
        eprintln!(
            "power gpu={:?} cpu={:?} dram={:?} ane={:?} e={:?} p={:?}",
            power.gpu_power_w, power.cpu_power_w, power.dram_power_w, power.ane_power_w,
            power.ecpu_power_w, power.pcpu_power_w
        );
        release_sample(s2);
        release_sample(s1);
    }

    #[test]
    #[ignore = "discovery helper — run on Apple Silicon with --ignored --nocapture"]
    fn dump_ioreport_channels_on_host() {
        if std::env::consts::OS != "macos" {
            return;
        }
        let dvfs = load_dvfs_tables();
        eprintln!("dvfs gpu={:?} pcpu={:?} ecpu={:?}", dvfs.gpu_mhz, dvfs.pcpu_mhz, dvfs.ecpu_mhz);
        for group in ["GPU Stats", "CPU Stats"] {
            let Some(sub) = create_subscription(group) else {
                eprintln!("no subscription for {group}");
                continue;
            };
            let s1 = create_sample(&sub);
            std::thread::sleep(Duration::from_millis(300));
            let s2 = create_sample(&sub);
            let delta = create_delta(s1, s2);
            if group == "GPU Stats" {
                let ext = parse_extended_gpu(delta, s2, &dvfs);
                eprintln!("gpu extended: {ext:?}");
            } else {
                let clusters = parse_extended_cpu(delta, &dvfs);
                eprintln!("cpu e={:?} p={:?}", clusters.ecpu_freq_mhz, clusters.pcpu_freq_mhz);
            }
            release_sample(delta);
            release_sample(s2);
            release_sample(s1);
        }
    }
}
