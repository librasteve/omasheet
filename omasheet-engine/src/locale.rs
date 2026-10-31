// Copyright (c) 2026 Stephen Roe

//! How the user's locale writes dates and times, as the operating system
//! has it.

use crate::datefmt::Names;
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

/// What the user's locale (`LC_TIME`) calls the days, the months and the
/// halves of the day, and how it writes `%c`, `%x`, `%X` and `%r`; the `C`
/// locale's where it has none.
#[cfg(unix)]
pub fn os_names() -> Names {
    use std::ffi::CStr;
    let item = |item: libc::nl_item| {
        // SAFETY: `nl_langinfo` returns a valid string for a valid item; it
        // is copied before the next call can overwrite it.
        let text = unsafe { CStr::from_ptr(libc::nl_langinfo(item)) };
        text.to_string_lossy().into_owned()
    };
    // SAFETY: an empty locale name selects the one named by the environment.
    let found = unsafe { !libc::setlocale(libc::LC_TIME, c"".as_ptr()).is_null() };
    let c = Names::default();
    if !found {
        return c;
    }
    fn all<const N: usize>(
        first: libc::nl_item,
        item: impl Fn(libc::nl_item) -> String,
    ) -> [String; N] {
        std::array::from_fn(|k| item(first + k as libc::nl_item))
    }
    // A locale with no form of its own, as many have no `%r`, takes C's.
    let or = |found: String, c: String| if found.is_empty() { c } else { found };
    Names {
        days: all(libc::DAY_1, item),
        short_days: all(libc::ABDAY_1, item),
        months: all(libc::MON_1, item),
        short_months: all(libc::ABMON_1, item),
        halves: [item(libc::AM_STR), item(libc::PM_STR)],
        date_time: or(item(libc::D_T_FMT), c.date_time),
        date: or(item(libc::D_FMT), c.date),
        time: or(item(libc::T_FMT), c.time),
        time12: or(item(libc::T_FMT_AMPM), c.time12),
    }
}

#[cfg(not(unix))]
pub fn os_names() -> Names {
    Names::default()
}

/// The decimal point and the separator of digit groups of the user's locale
/// (`LC_NUMERIC`): `.` and `,` in Britain, `,` and `.` in Germany. A locale
/// with no separator, as the `C` locale has none, groups by `,`, or by a
/// narrow space where its point is `,`.
#[cfg(unix)]
pub fn os_numbers() -> (char, char) {
    use std::ffi::CStr;
    let first = |item: libc::nl_item| {
        // SAFETY: `nl_langinfo` returns a valid string for a valid item; its
        // first character is taken before the next call can overwrite it.
        let text = unsafe { CStr::from_ptr(libc::nl_langinfo(item)) };
        text.to_string_lossy().chars().next()
    };
    // SAFETY: a null locale name only queries the current one, which is
    // copied before it is changed, to be put back after.
    let before = unsafe {
        let name = libc::setlocale(libc::LC_NUMERIC, std::ptr::null());
        (!name.is_null()).then(|| CStr::from_ptr(name).to_owned())
    };
    // SAFETY: an empty locale name selects the one named by the environment.
    let found = unsafe { !libc::setlocale(libc::LC_NUMERIC, c"".as_ptr()).is_null() };
    let (point, sep) = if found {
        (first(libc::RADIXCHAR), first(libc::THOUSEP))
    } else {
        (None, None)
    };
    // SAFETY: the locale is put back as it was, so that nothing else reads
    // numbers differently.
    unsafe { libc::setlocale(libc::LC_NUMERIC, before.as_deref().unwrap_or(c"C").as_ptr()) };
    let point = point.unwrap_or('.');
    let fallback = if point == ',' { '\u{202F}' } else { ',' };
    (point, sep.filter(|&s| s != point).unwrap_or(fallback))
}

#[cfg(not(unix))]
pub fn os_numbers() -> (char, char) {
    ('.', ',')
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_locale_point_is_never_its_separator() {
        let (point, sep) = os_numbers();
        assert!(matches!(point, '.' | ','), "{point:?}");
        assert_ne!(point, sep);
    }
}
