// Copyright (c) 2026 Stephen Roe

//! A number shown by its column's format, `'.2f`, as C `printf` shows it,
//! with the point and the separator of digit groups of the locale.
//!
//! An exact number is rounded from its own digits rather than through an
//! `f64`, so every digit shown is right; a `Float` is rounded by Rust's own
//! formatting, which rounds the binary value exactly, as C does. Either way
//! a tie goes to the even digit, which is what C does with the value it has.

use omasheet_omx::Dec;
use omasheet_omx::date::Style;
use omasheet_omx::numfmt::{Conv, NumFormat};

use crate::value::Value;

/// The most digits a number is written out to before its point. Past it,
/// `1e200` under `f`, the number is shown as it would be with no format.
const MAX_DIGITS: i64 = 400;

/// A number's size, without its sign.
#[derive(Clone, Copy)]
enum Size {
    Exact(Dec),
    Float(f64),
}

/// `v` shown by `f` in `style`, or `None` where `f` does not show it: a value
/// that is not a number, or one too large to write out.
pub fn format_number(v: &Value, f: &NumFormat, style: &Style) -> Option<String> {
    if f.conv.is_integer() {
        let n = match v {
            Value::Int(n, _) => *n,
            Value::Decimal(r) | Value::Fraction(r) | Value::Percent(r) if r.is_integer() => {
                r.to_i64()?
            }
            _ => return None,
        };
        return Some(whole(n, f, style));
    }
    let (negative, size) = match v {
        Value::Int(n, _) => (*n < 0, Size::Exact(Dec::from_i64(*n).checked_abs()?)),
        Value::Decimal(r) | Value::Fraction(r) | Value::Percent(r) => {
            let r = r.guarded();
            (r.is_negative(), Size::Exact(r.checked_abs()?))
        }
        Value::Float(x) if x.is_nan() || x.is_infinite() => {
            return Some(not_finite(*x, f));
        }
        Value::Float(x) => (x.is_sign_negative(), Size::Float(x.abs())),
        _ => return None,
    };
    let body = places(size, f)?;
    Some(laid_out(f, sign(negative, f), "", &body, style))
}

/// The sign C writes before a number: `-`, and for one that is not negative
/// `+` or a space where the format asks.
fn sign(negative: bool, f: &NumFormat) -> &'static str {
    if negative {
        "-"
    } else if f.plus {
        "+"
    } else if f.space {
        " "
    } else {
        ""
    }
}

/// A whole number by `d`, `x`, `o` or `b`. Hex, octal and binary show a
/// negative number as its 64-bit two's complement, and take no sign.
fn whole(n: i64, f: &NumFormat, style: &Style) -> String {
    let bits = n as u64;
    let (mut digits, sign, prefix) = match f.conv {
        Conv::Dec => (n.unsigned_abs().to_string(), sign(n < 0, f), ""),
        Conv::Hex { upper: false } => (format!("{bits:x}"), "", "0x"),
        Conv::Hex { upper: true } => (format!("{bits:X}"), "", "0X"),
        Conv::Bin { upper } => (format!("{bits:b}"), "", if upper { "0B" } else { "0b" }),
        Conv::Oct => (format!("{bits:o}"), "", ""),
        _ => unreachable!("a conversion of a whole number"),
    };
    // A precision is the fewest digits there are, and a precision of
    // nought shows nought as nothing at all.
    if let Some(p) = f.precision {
        if p == 0 && n == 0 {
            digits.clear();
        } else if digits.len() < p {
            digits.insert_str(0, &"0".repeat(p - digits.len()));
        }
    }
    // `#` puts `0x` or `0b` before a number that is not nought, and makes
    // the first digit of octal a nought.
    let prefix = if f.alt && n != 0 { prefix } else { "" };
    if f.alt && f.conv == Conv::Oct && !digits.starts_with('0') {
        digits.insert(0, '0');
    }
    laid_out(f, sign, prefix, &digits, style)
}

/// `inf` or `nan`, in capitals under `F`, `E` and `G`, never padded with
/// noughts.
fn not_finite(x: f64, f: &NumFormat) -> String {
    let upper = matches!(
        f.conv,
        Conv::Fixed { upper: true } | Conv::Exp { upper: true } | Conv::General { upper: true }
    );
    let word = match (x.is_nan(), upper) {
        (true, false) => "nan",
        (true, true) => "NAN",
        (false, false) => "inf",
        (false, true) => "INF",
    };
    let text = format!("{}{word}", sign(x.is_sign_negative() && !x.is_nan(), f));
    padded(f, text)
}

/// `text` made up to the format's width with spaces, before it or, under
/// `-`, after it.
fn padded(f: &NumFormat, text: String) -> String {
    let width = f.width.unwrap_or(0);
    let len = text.chars().count();
    if len >= width {
        return text;
    }
    let pad = " ".repeat(width - len);
    if f.left { text + &pad } else { pad + &text }
}

/// A number's sign, its prefix and its digits, `body`, laid out: its whole
/// part grouped under `'`, its point the locale's, and made up to the width
/// with noughts after the sign under `0`, or with spaces. The noughts are
/// not grouped, as C does not group them.
fn laid_out(f: &NumFormat, sign: &str, prefix: &str, body: &str, style: &Style) -> String {
    // The point first, so that a separator that is a `.` is not taken for
    // one.
    let body = if f.conv.is_integer() {
        body.to_string()
    } else {
        body.replace('.', &style.point.to_string())
    };
    let fixed = matches!(
        f.conv,
        Conv::Fixed { .. } | Conv::General { .. } | Conv::Dec
    );
    let body = match style.group.filter(|_| f.group && fixed) {
        Some(sep) => grouped(&body, sep),
        None => body,
    };
    // A precision on a whole number says how many digits there are, so
    // `0` has nothing to make up.
    let zero = f.zero && !f.left && !(f.conv.is_integer() && f.precision.is_some());
    let width = f.width.unwrap_or(0);
    let len = sign.chars().count() + prefix.len() + body.chars().count();
    if zero && len < width {
        return format!("{sign}{prefix}{}{body}", "0".repeat(width - len));
    }
    padded(f, format!("{sign}{prefix}{body}"))
}

/// The digits before the point of `body` grouped in threes by `sep`.
fn grouped(body: &str, sep: char) -> String {
    let whole_len = body
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(body.len());
    let (whole, rest) = body.split_at(whole_len);
    let mut out = String::new();
    for (k, digit) in whole.chars().enumerate() {
        if k > 0 && (whole.len() - k) % 3 == 0 {
            out.push(sep);
        }
        out.push(digit);
    }
    out + rest
}

/// A number's size by `f`, `e` or `g`, with `.` for its point.
fn places(size: Size, f: &NumFormat) -> Option<String> {
    let p = f.precision.unwrap_or(6);
    match f.conv {
        Conv::Fixed { .. } => {
            let mut text = fixed(size, p)?;
            if f.alt && p == 0 {
                text.push('.');
            }
            Some(text)
        }
        Conv::Exp { upper } => {
            let (mantissa, e) = scientific(size, p);
            Some(exponent_form(mantissa, e, p == 0 && f.alt, upper))
        }
        Conv::General { upper } => {
            // `g` is `e` where the exponent is below -4 or not below the
            // precision, and `f` otherwise, with as many places as leave
            // the precision in significant digits.
            let p = p.max(1);
            let (_, x) = scientific(size, p - 1);
            let text = if x < -4 || x >= p as i64 {
                let (mantissa, e) = scientific(size, p - 1);
                let mantissa = if f.alt { mantissa } else { unpadded(mantissa) };
                exponent_form(mantissa, e, f.alt && p == 1, upper)
            } else {
                let text = fixed(size, (p as i64 - 1 - x) as usize)?;
                match (f.alt, text.contains('.')) {
                    (false, _) => unpadded(text),
                    (true, false) => text + ".",
                    (true, true) => text,
                }
            };
            Some(text)
        }
        _ => None,
    }
}

/// `text` without the noughts it ends with after its point, or the point.
fn unpadded(text: String) -> String {
    if !text.contains('.') {
        return text;
    }
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// A mantissa and its exponent written as C writes them: `1.50e+03`, with
/// at least two digits of exponent, and the point kept where `point`.
fn exponent_form(mut mantissa: String, e: i64, point: bool, upper: bool) -> String {
    if point && !mantissa.contains('.') {
        mantissa.push('.');
    }
    let letter = if upper { 'E' } else { 'e' };
    let sign = if e < 0 { '-' } else { '+' };
    format!("{mantissa}{letter}{sign}{:02}", e.abs())
}

/// The size with `p` places after the point, or `None` where it has more
/// than [`MAX_DIGITS`] before it.
fn fixed(size: Size, p: usize) -> Option<String> {
    match size {
        Size::Float(x) => {
            if x >= 10f64.powi(MAX_DIGITS as i32) {
                return None;
            }
            Some(format!("{x:.p$}"))
        }
        Size::Exact(r) => {
            if r.magnitude() >= MAX_DIGITS {
                return None;
            }
            let (digits, scale) = (r.unscaled().unsigned_abs().to_string(), r.scale() as i64);
            // The size times ten to the `p`, as a whole number of digits.
            let shift = p as i64 - scale;
            let n = if shift >= 0 {
                digits + &"0".repeat(shift as usize)
            } else {
                rounded(&digits, (-shift) as usize)
            };
            let n = if n.len() <= p {
                "0".repeat(p + 1 - n.len()) + &n
            } else {
                n
            };
            let (whole, part) = n.split_at(n.len() - p);
            Some(if p == 0 {
                whole.to_string()
            } else {
                format!("{whole}.{part}")
            })
        }
    }
}

/// The size as one digit, `p` places, and the power of ten: `(1.50, 3)`
/// for `1500` with `p` of 2. Nought is `0.00` and nought.
fn scientific(size: Size, p: usize) -> (String, i64) {
    match size {
        Size::Float(x) => {
            let text = format!("{x:.p$e}");
            let (mantissa, e) = text.split_once('e').unwrap_or((&text, "0"));
            (mantissa.to_string(), e.parse().unwrap_or(0))
        }
        Size::Exact(r) => {
            let (digits, mut e) = r.digits_and_exponent();
            let want = p + 1;
            let mut kept = if digits.len() <= want {
                let pad = "0".repeat(want - digits.len());
                digits + &pad
            } else {
                rounded(&digits, digits.len() - want)
            };
            // Rounding up `9.99` gives `10.0`: one digit more, and a power
            // of ten higher.
            if kept.len() > want {
                kept.truncate(want);
                e += 1;
            }
            let (first, rest) = kept.split_at(1);
            let mantissa = if rest.is_empty() {
                first.to_string()
            } else {
                format!("{first}.{rest}")
            };
            (mantissa, if r.is_zero() { 0 } else { e })
        }
    }
}

/// The whole number `digits` with its last `drop` digits taken off, rounded
/// half to even: `rounded("125", 1)` is `"12"` and `rounded("135", 1)` is
/// `"14"`. Taking off every digit leaves `"0"`, or `"1"` past a half.
fn rounded(digits: &str, drop: usize) -> String {
    if drop > digits.len() {
        return "0".to_string();
    }
    let (kept, gone) = digits.split_at(digits.len() - drop);
    let mut gone = gone.bytes();
    let first = gone.next().unwrap_or(b'0');
    let rest_zero = gone.all(|b| b == b'0');
    let last_odd = kept.bytes().last().is_some_and(|b| (b - b'0') % 2 == 1);
    let up = first > b'5' || (first == b'5' && (!rest_zero || last_odd));
    let mut kept: Vec<u8> = if kept.is_empty() {
        b"0".to_vec()
    } else {
        kept.bytes().collect()
    };
    if up {
        let mut k = kept.len();
        loop {
            if k == 0 {
                kept.insert(0, b'1');
                break;
            }
            k -= 1;
            if kept[k] == b'9' {
                kept[k] = b'0';
            } else {
                kept[k] += 1;
                break;
            }
        }
    }
    String::from_utf8(kept).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    fn fmt(text: &str) -> NumFormat {
        NumFormat::parse(text).expect("reads")
    }

    fn dec(text: &str) -> Value {
        Value::Decimal(Dec::parse(text).expect("parses"))
    }

    /// What C shows, in the `C` locale, for a format and a double.
    fn c_double(format: &str, x: f64) -> String {
        let spec = CString::new(format!("%{format}")).expect("no nul");
        let mut buf = vec![0u8; 2048];
        // SAFETY: the format is one conversion of a double, which is what
        // is passed, and the buffer is as long as is said.
        let n = unsafe { libc::snprintf(buf.as_mut_ptr().cast(), buf.len(), spec.as_ptr(), x) };
        buf.truncate(n as usize);
        String::from_utf8(buf).expect("ascii")
    }

    /// What C shows, in the `C` locale, for a format and a 64-bit integer.
    fn c_long(format: &str, n: i64) -> String {
        let spec = CString::new(format!(
            "%{}l{}",
            &format[..format.len() - 1],
            &format[format.len() - 1..]
        ))
        .expect("no nul");
        let mut buf = vec![0u8; 2048];
        // SAFETY: the format is one conversion of a long, which is what is
        // passed, and the buffer is as long as is said.
        let n = unsafe {
            libc::snprintf(
                buf.as_mut_ptr().cast(),
                buf.len(),
                spec.as_ptr(),
                n as libc::c_long,
            )
        };
        buf.truncate(n as usize);
        String::from_utf8(buf).expect("ascii")
    }

    const FLAGS: [&str; 8] = ["", "+", " ", "-", "0", "#", "+0", "-+#"];

    #[test]
    fn floats_are_shown_as_c_shows_them() {
        let values = [
            0.0,
            -0.0,
            1.0,
            -1.5,
            0.125,
            2.5,
            3.5,
            0.045,
            12345.678,
            1e-7,
            1e21,
            123456789.0,
            9.9999,
            0.0001234,
            5e-324,
            f64::MAX,
            f64::INFINITY,
            -f64::INFINITY,
            f64::NAN,
        ];
        for flags in FLAGS {
            for size in ["", "12", ".0", ".2", "15.3", ".10", "1.0"] {
                for conv in ["f", "F", "e", "E", "g", "G"] {
                    let format = format!("{flags}{size}{conv}");
                    for &x in &values {
                        let ours = format_number(&Value::Float(x), &fmt(&format), &Style::ISO);
                        let ours = ours.unwrap_or_else(|| "too large".into());
                        if ours == "too large" {
                            continue;
                        }
                        assert_eq!(ours, c_double(&format, x), "%{format} of {x:e}");
                    }
                }
            }
        }
    }

    #[test]
    fn exact_numbers_are_shown_as_c_shows_their_doubles() {
        // Each of these is a double exactly, so C and the exact renderer
        // see the same number.
        let values = [
            "0",
            "1",
            "-1.5",
            "0.125",
            "2.5",
            "3.5",
            "-0.5",
            "12345.75",
            "1000000",
            "0.0009765625",
            "999.5",
            "9.5",
            "123456789",
        ];
        for flags in FLAGS {
            for size in ["", "12", ".0", ".2", "15.3", ".1"] {
                for conv in ["f", "e", "E", "g", "G"] {
                    let format = format!("{flags}{size}{conv}");
                    for v in values {
                        // glibc drops the noughts `#` keeps where rounding
                        // carries into a new digit, `1.e+03` for `%#.3g` of
                        // 999.5; C has `1.00e+03`, which is what is shown.
                        if flags.contains('#') && conv.eq_ignore_ascii_case("g") && v == "999.5" {
                            continue;
                        }
                        let ours =
                            format_number(&dec(v), &fmt(&format), &Style::ISO).expect("shown");
                        let x: f64 = v.parse().expect("a double");
                        assert_eq!(ours, c_double(&format, x), "%{format} of {v}");
                    }
                }
            }
        }
    }

    #[test]
    fn whole_numbers_are_shown_as_c_shows_them() {
        let values = [0, 1, -1, 7, 255, -255, 1234567, i64::MAX, i64::MIN];
        for flags in FLAGS {
            for size in ["", "8", ".0", ".3", "12.5", "-6"] {
                for conv in ["d", "i", "x", "X", "o", "b", "B"] {
                    if flags.contains('#') && matches!(conv, "d" | "i") {
                        continue;
                    }
                    let format = format!("{flags}{size}{conv}");
                    let Ok(f) = NumFormat::parse(&format) else {
                        continue;
                    };
                    for &n in &values {
                        let ours = format_number(&Value::int(n), &f, &Style::ISO).expect("shown");
                        assert_eq!(ours, c_long(&format, n), "%{format} of {n}");
                    }
                }
            }
        }
    }

    #[test]
    fn exact_digits_past_a_double() {
        let shown = |f: &str, v: &str| format_number(&dec(v), &fmt(f), &Style::ISO).expect("shown");
        assert_eq!(shown(".20f", "0.1"), "0.10000000000000000000");
        assert_eq!(
            shown(".2f", "12345678901234567890.125"),
            "12345678901234567890.12"
        );
        assert_eq!(shown(".2f", "0.135"), "0.14");
        assert_eq!(shown(".2f", "0.125"), "0.12");
        assert_eq!(
            shown(".25e", "1234567890.1234567890123456"),
            "1.2345678901234567890123456e+09"
        );
        assert_eq!(shown("f", "1e-30"), "0.000000");
        assert_eq!(shown(".0f", "0.5"), "0");
        assert_eq!(shown(".0f", "0.51"), "1");
    }

    #[test]
    fn the_locale_groups_and_points() {
        let uk = Style::ISO.numbers('.', ',');
        let de = Style::ISO.numbers(',', '.');
        let shown = |f: &str, v: Value, s: &Style| format_number(&v, &fmt(f), s).expect("shown");
        assert_eq!(shown("'.2f", dec("12345.678"), &uk), "12,345.68");
        assert_eq!(shown("'.2f", dec("12345.678"), &de), "12.345,68");
        assert_eq!(shown(".2f", dec("12345.678"), &uk), "12345.68");
        assert_eq!(
            shown("+'015.2f", Value::Float(12345.6), &uk),
            "+0000012,345.60"
        );
        assert_eq!(shown("'d", Value::int(-1234567), &uk), "-1,234,567");
        assert_eq!(shown("'g", Value::Float(1234.5), &uk), "1,234.5");
        assert_eq!(shown(".2f", Value::Float(0.5), &de), "0,50");
        // Without the locale's separator, `'` groups nothing, as in C.
        assert_eq!(shown("'d", Value::int(1234567), &Style::ISO), "1234567");
    }

    #[test]
    fn hex_and_binary_of_a_negative_are_its_twos_complement() {
        let shown =
            |f: &str, n: i64| format_number(&Value::int(n), &fmt(f), &Style::ISO).expect("shown");
        assert_eq!(shown("#x", 255), "0xff");
        assert_eq!(shown("#x", -255), "0xffffffffffffff01");
        assert_eq!(shown("#b", 5), "0b101");
        assert_eq!(shown("08b", 5), "00000101");
    }

    #[test]
    fn shows_only_what_it_takes() {
        assert!(format_number(&Value::text("x"), &fmt("d"), &Style::ISO).is_none());
        assert!(format_number(&dec("1.5"), &fmt("d"), &Style::ISO).is_none());
        assert!(format_number(&Value::Null, &fmt("f"), &Style::ISO).is_none());
        assert!(format_number(&dec("1e500"), &fmt("f"), &Style::ISO).is_none());
        assert_eq!(
            format_number(&dec("1e500"), &fmt("e"), &Style::ISO).as_deref(),
            Some("1.000000e+500")
        );
        assert_eq!(
            format_number(&dec("4"), &fmt("d"), &Style::ISO).as_deref(),
            Some("4")
        );
    }
}
