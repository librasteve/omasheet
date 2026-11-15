// Copyright (c) 2026 Stephen Roe

//! The Windows build is a GUI program, so that opening the app from the Start
//! menu or by double-clicking a file starts no console window beside it. The
//! `omasheet` command is the same program, so when it is run from a terminal
//! it joins that terminal's console, and its output and input are there.

/// Join the console of the terminal that started this program, if there was
/// one. A standard handle that was redirected (to a file or a pipe) is left as
/// it is.
#[cfg(windows)]
pub fn attach() {
    use std::ffi::c_void;
    use std::fs::OpenOptions;
    use std::os::windows::io::IntoRawHandle;

    unsafe extern "system" {
        fn AttachConsole(process: u32) -> i32;
        fn GetStdHandle(which: u32) -> *mut c_void;
        fn SetStdHandle(which: u32, handle: *mut c_void) -> i32;
    }
    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    // -10, -11 and -12 as the unsigned numbers the API takes.
    const STD_INPUT: u32 = 0xFFFF_FFF6;
    const STD_OUTPUT: u32 = 0xFFFF_FFF5;
    const STD_ERROR: u32 = 0xFFFF_FFF4;

    // SAFETY: plain Win32 calls, made before anything else is running.
    unsafe {
        if AttachConsole(ATTACH_PARENT_PROCESS) == 0 {
            return;
        }
        for (which, name, writes) in [
            (STD_INPUT, "CONIN$", false),
            (STD_OUTPUT, "CONOUT$", true),
            (STD_ERROR, "CONOUT$", true),
        ] {
            let handle = GetStdHandle(which);
            if !handle.is_null() && handle as isize != -1 {
                continue;
            }
            if let Ok(file) = OpenOptions::new().read(!writes).write(writes).open(name) {
                SetStdHandle(which, file.into_raw_handle());
            }
        }
    }
}

#[cfg(not(windows))]
pub fn attach() {}
