// Copyright (c) 2026 Stephen Roe

//! The app icon, set at run time. An installed app gets its icon from the
//! bundle, but the bare binary (a `cargo run`, say) would show as a generic
//! "exec" in the Dock and the app switcher without this.

#[cxx::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("icon.h");
        unsafe fn set_app_icon(data: *const u8, len: usize);
    }
}

const PNG: &[u8] = include_bytes!("../omasheet.png");

/// Call after the application is made.
pub fn set() {
    // SAFETY: the pointer and length are those of a static slice.
    unsafe { ffi::set_app_icon(PNG.as_ptr(), PNG.len()) };
}
