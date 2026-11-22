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

        type QString = cxx_qt_lib::QString;
        fn hand_over(path: &QString) -> bool;
        fn listen_for_launches(engine: Pin<&mut QQmlApplicationEngine>);
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

/// On Windows and Linux, where an app runs once to a user: if it is running
/// already, give it the file this launch names (the first argument), or
/// none, and say so. The caller then exits. `OMASHEET_NEW_INSTANCE` starts
/// another regardless, as does the snapshot a development run takes.
pub fn hand_over(path: &str) -> bool {
    if cfg!(target_os = "macos")
        || std::env::var_os("OMASHEET_NEW_INSTANCE").is_some()
        || std::env::var_os("OMASHEET_UI_SNAPSHOT").is_some()
    {
        return false;
    }
    ffi::hand_over(&cxx_qt_lib::QString::from(path))
}

/// Listen for the launches that `hand_over` sends, once the windows are up.
pub fn listen_for_launches(engine: Pin<&mut QQmlApplicationEngine>) {
    if !cfg!(target_os = "macos") {
        ffi::listen_for_launches(engine);
    }
}
