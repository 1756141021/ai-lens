// DPAPI (CryptProtectData) for the API key at rest. Raw FFI like cursor.rs —
// the `windows` crate conflicts with xcap's pinned version.
// CURRENT_USER scope: the ciphertext is useless off this machine/account, which
// is exactly the threat model (config.json walking away via cloud sync/backup).
use base64::{engine::general_purpose::STANDARD, Engine};
use std::ffi::c_void;
use std::ptr;

#[repr(C)]
struct DataBlob {
    cb_data: u32,
    pb_data: *mut u8,
}

const CRYPTPROTECT_UI_FORBIDDEN: u32 = 0x1;

#[link(name = "crypt32")]
extern "system" {
    fn CryptProtectData(
        data_in: *const DataBlob,
        descr: *const u16,
        entropy: *const DataBlob,
        reserved: *mut c_void,
        prompt: *mut c_void,
        flags: u32,
        data_out: *mut DataBlob,
    ) -> i32;
    fn CryptUnprotectData(
        data_in: *const DataBlob,
        descr: *mut *mut u16,
        entropy: *const DataBlob,
        reserved: *mut c_void,
        prompt: *mut c_void,
        flags: u32,
        data_out: *mut DataBlob,
    ) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn LocalFree(mem: *mut c_void) -> *mut c_void;
}

/// Marks an encrypted value in config.json; absence means legacy plaintext.
pub const PREFIX: &str = "dpapi:";

pub fn protect(plain: &str) -> Option<String> {
    let bytes = plain.as_bytes();
    let input = DataBlob {
        cb_data: bytes.len() as u32,
        pb_data: bytes.as_ptr() as *mut u8,
    };
    let mut output = DataBlob {
        cb_data: 0,
        pb_data: ptr::null_mut(),
    };
    let ok = unsafe {
        CryptProtectData(
            &input,
            ptr::null(),
            ptr::null(),
            ptr::null_mut(),
            ptr::null_mut(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 || output.pb_data.is_null() {
        return None;
    }
    let encrypted =
        unsafe { std::slice::from_raw_parts(output.pb_data, output.cb_data as usize) }.to_vec();
    unsafe { LocalFree(output.pb_data as *mut c_void) };
    Some(format!("{PREFIX}{}", STANDARD.encode(encrypted)))
}

pub fn unprotect(stored: &str) -> Option<String> {
    let b64 = stored.strip_prefix(PREFIX)?;
    let bytes = STANDARD.decode(b64).ok()?;
    let input = DataBlob {
        cb_data: bytes.len() as u32,
        pb_data: bytes.as_ptr() as *mut u8,
    };
    let mut output = DataBlob {
        cb_data: 0,
        pb_data: ptr::null_mut(),
    };
    let ok = unsafe {
        CryptUnprotectData(
            &input,
            ptr::null_mut(),
            ptr::null(),
            ptr::null_mut(),
            ptr::null_mut(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 || output.pb_data.is_null() {
        return None;
    }
    let plain =
        unsafe { std::slice::from_raw_parts(output.pb_data, output.cb_data as usize) }.to_vec();
    unsafe { LocalFree(output.pb_data as *mut c_void) };
    String::from_utf8(plain).ok()
}
