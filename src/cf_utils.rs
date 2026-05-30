pub type CVoidRef = *const std::ffi::c_void;

use core_foundation::base::TCFType;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;

pub fn cfstr(s: &str) -> CFString {
    CFString::new(s)
}

pub unsafe fn from_cfstring(ptr: CVoidRef) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    let cf = CFString::wrap_under_get_rule(ptr as *const _);
    Some(cf.to_string())
}

pub unsafe fn cfdict_get_value(dict: CVoidRef, key: &str) -> CVoidRef {
    if dict.is_null() {
        return std::ptr::null();
    }
    extern "C" {
        fn CFDictionaryGetValue(dict: CVoidRef, key: CVoidRef) -> CVoidRef;
    }
    let cf_key = cfstr(key);
    CFDictionaryGetValue(dict, cf_key.as_CFTypeRef())
}

pub unsafe fn cfnum_to_f64(ptr: CVoidRef) -> Option<f64> {
    if ptr.is_null() {
        return None;
    }
    let cf_num = CFNumber::wrap_under_get_rule(ptr as *const _);
    cf_num.to_f64()
}

pub unsafe fn cfarray_count(arr: CVoidRef) -> isize {
    if arr.is_null() {
        return 0;
    }
    extern "C" {
        fn CFArrayGetCount(arr: CVoidRef) -> isize;
    }
    CFArrayGetCount(arr)
}

pub unsafe fn cfarray_get(arr: CVoidRef, idx: isize) -> CVoidRef {
    if arr.is_null() {
        return std::ptr::null();
    }
    extern "C" {
        fn CFArrayGetValueAtIndex(arr: CVoidRef, idx: isize) -> CVoidRef;
    }
    CFArrayGetValueAtIndex(arr, idx)
}
