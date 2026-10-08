//! How the user's locale writes dates and times, as the operating system
//! has it.

use omasheet_omx::date::Style;

/// The date and time style of the user's locale (`LC_TIME`), or the ISO style
/// if the locale has none, as the `C` locale does not, or cannot be read.
#[cfg(unix)]
pub fn os_style() -> Style {
    use std::ffi::CStr;
    let item = |item: libc::nl_item| {
        // SAFETY: `nl_langinfo` returns a valid string for a valid item; it
        // is copied before the next call can overwrite it.
        let text = unsafe { CStr::from_ptr(libc::nl_langinfo(item)) };
        text.to_string_lossy().into_owned()
    };
    // SAFETY: an empty locale name selects the one named by the environment.
    // This is called once, before anything else reads the locale.
    let found = unsafe { !libc::setlocale(libc::LC_TIME, c"".as_ptr()).is_null() };
    if !found {
        return Style::ISO;
    }
    // SAFETY: a null locale name only queries the current one.
    let name = unsafe { CStr::from_ptr(libc::setlocale(libc::LC_TIME, std::ptr::null())) };
    if matches!(name.to_bytes(), b"C" | b"POSIX") || name.to_bytes().starts_with(b"C.") {
        return Style::ISO;
    }
    Style::from_strftime(&item(libc::D_FMT), &item(libc::T_FMT)).unwrap_or(Style::ISO)
}

#[cfg(not(unix))]
pub fn os_style() -> Style {
    Style::ISO
}

/// The name of the locale that [`os_style`] read, such as `en_GB`, without
/// its character encoding. Empty if there is none.
#[cfg(unix)]
pub fn os_name() -> String {
    use std::ffi::CStr;
    // SAFETY: a null locale name only queries the current one, and the
    // result is copied at once.
    let name = unsafe {
        let name = libc::setlocale(libc::LC_TIME, std::ptr::null());
        if name.is_null() {
            return String::new();
        }
        CStr::from_ptr(name).to_string_lossy().into_owned()
    };
    name.split(['.', '@']).next().unwrap_or("").to_string()
}

#[cfg(not(unix))]
pub fn os_name() -> String {
    String::new()
}
