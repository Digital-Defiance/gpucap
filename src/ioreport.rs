use crate::cf_utils::{CVoidRef, *};
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
}

extern "C" {
    fn CFRelease(cf: *const c_void);
    fn CFDictionaryGetCount(dict: CVoidRef) -> isize;
    fn CFDictionaryCreateMutableCopy(
        allocator: CVoidRef,
        capacity: isize,
        dict: CVoidRef,
    ) -> CVoidRef;
}

const CF_ALLOCATOR_DEFAULT: CVoidRef = std::ptr::null();

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

pub fn create_gpu_subscription() -> Option<IOReportSubscription> {
    unsafe {
        let cf_group = CFString::new("GPU Stats");
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
    unsafe {
        let items = cfdict_get_value(delta, "IOReportChannels");
        if items.is_null() {
            return None;
        }

        let count = cfarray_count(items);
        let mut total_active: i64 = 0;
        let mut total: i64 = 0;

        for i in 0..count {
            let ch = cfarray_get(items, i);
            if ch.is_null() {
                continue;
            }

            let group = from_cfstring(IOReportChannelGetGroup(ch));
            let subgroup = from_cfstring(IOReportChannelGetSubGroup(ch));
            let channel_name = from_cfstring(IOReportChannelGetChannelName(ch));

            let is_gpuph = group.as_deref() == Some("GPU Stats")
                && subgroup.as_deref() == Some("GPU Performance States")
                && channel_name.as_deref() == Some("GPUPH");

            if !is_gpuph {
                continue;
            }

            let state_count = IOReportStateGetCount(ch);
            for s in 0..state_count {
                let name = from_cfstring(IOReportStateGetNameForIndex(ch, s)).unwrap_or_default();
                let residency = IOReportStateGetResidency(ch, s);
                total += residency;
                if name != "OFF" {
                    total_active += residency;
                }
            }
        }

        if total > 0 {
            Some((total_active as f64 / total as f64) * 100.0)
        } else {
            Some(0.0)
        }
    }
}
