// Copyright (c) 2026 Stephen Roe

//! The bits of the app that Qt's Rust bindings do not reach, in C++:
//!
//! - The app icon, set at run time. An installed app gets its icon from the
//!   bundle, but the bare binary (a `cargo run`, say) would show as a generic
//!   "exec" in the Dock and the app switcher without this.
//! - The files the system asks the app to open. A double click in the Finder
//!   sends an open-file event, not a command line, so the file never reached
//!   the app.

use cxx_qt_lib::QQmlApplicationEngine;
use std::pin::Pin;

#[cxx::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("icon.h");
        unsafe fn set_app_icon(data: *const u8, len: usize);

        type QQmlApplicationEngine = cxx_qt_lib::QQmlApplicationEngine;
        fn forward_open_events(engine: Pin<&mut QQmlApplicationEngine>);
    }
}

const PNG: &[u8] = include_bytes!("../omasheet.png");

/// Call after the application is made.
pub fn set() {
    // SAFETY: the pointer and length are those of a static slice.
    unsafe { ffi::set_app_icon(PNG.as_ptr(), PNG.len()) };
}

/// Hand the files the system asks the app to open to the engine's root
/// object, once its windows are up.
pub fn forward_open(engine: Pin<&mut QQmlApplicationEngine>) {
    ffi::forward_open_events(engine);
}
