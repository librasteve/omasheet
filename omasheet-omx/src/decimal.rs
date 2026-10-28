// Copyright (c) 2026 Stephen Roe

//! A fixed-point decimal: an `i128` of unscaled digits and a scale saying
//! where the point sits, so that `unscaled / 10^scale` is the number.
//! Thirty-eight digits is the most an `i128` carries, and every operation
//! here keeps inside that or says it cannot.
//!
//! The digits are the ones a Polars `Decimal128` holds, and so is a scale of
//! `0..=38`; the scale here is wider than that one, and signed. A negative
//! scale puts the point to the right of the digits, which is what lets a
//! number far past 38 digits — `1e80`, one digit at a scale of `-80` — be
//! held exactly to the digits it was given. Such a number has no Polars
//! decimal to be, so a column of them stays on the engine and a Parquet
//! write puts a null in its place and says so.
//!
//! Arithmetic is exact wherever the result fits: adding and subtracting line
//! the scales up, multiplying adds them. Division is the one operation that
//! need not terminate, so it rounds half away from zero at the widest scale
//! the result can hold.
//!
//! No fixed-point decimal is closed under division, so the last digits of a
//! long one are where the rounding of every operation before it has
//! collected: `1/7 + 1/7` and `2/7` are both a seventh of two, and differ in
//! the thirty-eighth digit. The last [`GUARD_DIGITS`] digits are therefore
//! working room: they are held, so that a long sum of long numbers has room
//! to be right, but they are not compared and not shown. That is what makes
//! `2/7 + 3/7 == 5/7`, and `1/3 * 3` one rather than all nines.

use std::cmp::Ordering;
use std::fmt;

/// The most digits a `Decimal128` holds.
pub const MAX_PRECISION: u32 = 38;

/// The furthest the point can sit to the left of the digits, and so the
/// smallest number there is: `1e-2147483647`. The bound is symmetric with
/// [`MIN_SCALE`] because `i32::MIN` has no positive counterpart, and
/// negating a scale must always hold.
pub const MAX_SCALE: i32 = i32::MAX;

/// The furthest the point can sit to the right of the digits, and so the
/// largest number there is: `(10^38 - 1) * 10^2147483647`.
pub const MIN_SCALE: i32 = -i32::MAX;

/// How many of the digits a `Decimal128` carries are working room: held, but
/// neither compared nor shown, so that the rounding a long calculation
/// collects in them does not reach the number anybody reads. See the module
/// documentation.
pub const GUARD_DIGITS: u32 = 8;

/// How many significant digits a number is compared and shown to.
pub const SHOWN_PRECISION: u32 = MAX_PRECISION - GUARD_DIGITS;

/// The most characters [`Dec::to_plain_string`] will write out before it
/// gives the number in e notation instead. A scale reaches into the
/// billions, and a number is read by somebody.
const PLAIN_WIDTH: usize = 64;

/// The largest denominator [`Dec::to_fraction`] will answer with.
pub const MAX_DENOMINATOR: i64 = 10_000;

/// `10^MAX_PRECISION`, the first value too wide to store.
const LIMIT: i128 = 10i128.pow(MAX_PRECISION);

/// A number as `unscaled / 10^scale`.
#[derive(Clone, Copy, Debug)]
pub struct Dec {
    unscaled: i128,
    scale: i32,
}

/// Ten to the `n`, or `None` when that will not fit.
fn pow10(n: u32) -> Option<i128> {
    if n > MAX_PRECISION {
        None
    } else {
        Some(10i128.pow(n))
    }
}

impl Dec {
    pub const ZERO: Dec = Dec {
        unscaled: 0,
        scale: 0,
    };
    pub const ONE: Dec = Dec {
        unscaled: 1,
        scale: 0,
    };

    /// A whole number.
    pub fn from_i64(n: i64) -> Dec {
        Dec {
            unscaled: n as i128,
            scale: 0,
        }
    }

    /// `unscaled / 10^scale`, or `None` if it is too wide to hold.
    pub fn new(unscaled: i128, scale: i32) -> Option<Dec> {
        if !(MIN_SCALE..=MAX_SCALE).contains(&scale) || unscaled.unsigned_abs() >= LIMIT as u128 {
            None
        } else {
            Some(Dec { unscaled, scale })
        }
    }

    pub fn unscaled(self) -> i128 {
        self.unscaled
    }

    pub fn scale(self) -> i32 {
        self.scale
    }

    pub fn is_zero(self) -> bool {
        self.unscaled == 0
    }

    pub fn is_negative(self) -> bool {
        self.unscaled < 0
    }

    /// Whether this is a whole number.
    pub fn is_integer(self) -> bool {
        self.trimmed().scale <= 0
    }

    /// The same number with no needless trailing zeros.
    pub fn trimmed(self) -> Dec {
        let mut d = self;
        // Nothing has no digits to give up, and a scale to walk down from
        // that reaches into the billions.
        if d.unscaled == 0 {
            d.scale = 0;
            return d;
        }
        while d.scale > 0 && d.unscaled % 10 == 0 {
            d.unscaled /= 10;
            d.scale -= 1;
        }
        d
    }

    /// This number as a whole number, when it is one and it fits.
    pub fn to_i64(self) -> Option<i64> {
        let d = self.trimmed();
        match d.scale {
            0 => i64::try_from(d.unscaled).ok(),
            // A negative scale is a whole number with zeros after its
            // digits: it is one an `i64` holds only when writing them out
            // fits, which for anything past `1e18` it does not.
            s if s < 0 => i64::try_from(d.rescale(0)?.unscaled).ok(),
            _ => None,
        }
    }

    pub fn to_f64(self) -> f64 {
        self.unscaled as f64 / 10f64.powi(self.scale)
    }

    /// The shortest decimal that reads back as the same `f64`. `None` for an
    /// infinity or a `NaN`, which no decimal stands for.
    pub fn from_f64(f: f64) -> Option<Dec> {
        if !f.is_finite() {
            return None;
        }
        Dec::parse(&format!("{f:e}"))
    }

    /// How many significant digits this number holds: `19.99` holds four and
    /// `1e-38` one.
    pub fn precision(self) -> u32 {
        let d = self.trimmed();
        if d.unscaled == 0 {
            0
        } else {
            digits(d.unscaled)
        }
    }

    /// This number without its guard digits: kept to [`SHOWN_PRECISION`]
    /// significant digits, which is the number as it is compared and shown.
    /// One of fewer digits than that is itself, so money, any other decimal
    /// that ends, and the smallest numbers there are, whose every digit is
    /// significant, are all untouched.
    pub fn guarded(self) -> Dec {
        let d = self.trimmed();
        let held = d.precision();
        if held <= SHOWN_PRECISION {
            return d;
        }
        // Only digits after the point are given up: the guard digits are
        // where the rounding of a long calculation collects, and a whole
        // number has none of them. `2**100` is exact in 31 digits, so all
        // 31 of them are shown.
        let scale = (d.scale - (held - SHOWN_PRECISION) as i32).max(d.scale.min(0));
        d.rescale(scale).unwrap_or(d).trimmed()
    }

    /// Whether this number holds more digits than it shows, so that what is
    /// held is a rounding of it: a quotient keeps every digit it can, so one
    /// that reaches into the guard digits is one that did not end.
    pub fn is_long(self) -> bool {
        self.precision() > SHOWN_PRECISION
    }

    /// The simplest fraction that comes to this number, with a denominator of
    /// at most `max_den`: `0.142857…` gives `(1, 7)` and `0.125` gives
    /// `(1, 8)`. `None` when no such fraction comes to it.
    ///
    /// The convergents of the continued fraction of the number are the
    /// simplest fractions there are at each size of denominator, so the
    /// first of them that comes back to the number is the answer. "Comes
    /// back to" is this type's own equality, which ignores the guard digits,
    /// and that is what lets a seventh be read back out of a number that was
    /// rounded to hold it.
    pub fn to_fraction(self, max_den: i64) -> Option<(i64, i64)> {
        let whole = self.guarded();
        let size = whole.checked_abs()?;
        // `n / d` is the number exactly, as two whole numbers.
        let mut n = size.unscaled;
        // No fraction of a number past the digits there are: its point is
        // right of them, so it is whole and vastly beyond any denominator.
        let mut d = pow10(u32::try_from(size.scale).ok()?)?;
        let (mut h_prev, mut h) = (0i128, 1i128);
        let (mut k_prev, mut k) = (1i128, 0i128);
        while d != 0 {
            let q = n / d;
            (n, d) = (d, n - q * d);
            let h_next = q.checked_mul(h)?.checked_add(h_prev)?;
            let k_next = q.checked_mul(k)?.checked_add(k_prev)?;
            (h_prev, h) = (h, h_next);
            (k_prev, k) = (k, k_next);
            if k > max_den as i128 {
                return None;
            }
            let (num, den) = (i64::try_from(h).ok()?, i64::try_from(k).ok()?);
            if den > 0 && Dec::from_i64(num).checked_div(Dec::from_i64(den))? == size {
                let num = if self.is_negative() { -num } else { num };
                return Some((num, den));
            }
        }
        None
    }

    /// The significant digits, with no leading or trailing zeros, and the
    /// power of ten of the first of them: `19.99` gives `("1999", 1)` and
    /// `1000` gives `("1", 3)`. Zero gives `("0", 0)`.
    pub fn digits_and_exponent(self) -> (String, i64) {
        if self.unscaled == 0 {
            return ("0".to_string(), 0);
        }
        let digits = self.unscaled.unsigned_abs().to_string();
        let exponent = digits.len() as i64 - 1 - self.scale as i64;
        let trimmed = digits.trim_end_matches('0');
        let digits = if trimmed.is_empty() { "0" } else { trimmed };
        (digits.to_string(), exponent)
    }

    /// The power of ten of this number: `e` where `10^e <= |self| < 10^(e+1)`.
    pub fn magnitude(self) -> i64 {
        self.digits_and_exponent().1
    }

    /// `-1`, `0` or `1` by sign.
    pub fn signum(self) -> i64 {
        self.unscaled.signum() as i64
    }

    /// Whether anything of this number sits after the point.
    fn has_fraction(self) -> bool {
        match self.scale {
            s if s <= 0 => false,
            // Past 38 places every digit is after the point, since there are
            // at most 38 of them: the number is below one.
            s if s > MAX_PRECISION as i32 => self.unscaled != 0,
            s => self.unscaled % pow10(s as u32).unwrap_or(1) != 0,
        }
    }

    /// Towards zero.
    pub fn trunc(self) -> Dec {
        match self.scale {
            // Already whole: the point is at or right of the digits.
            s if s <= 0 => self,
            s if s > MAX_PRECISION as i32 => Dec::ZERO,
            s => Dec {
                unscaled: self.unscaled / pow10(s as u32).unwrap_or(1),
                scale: 0,
            },
        }
    }

    /// Towards negative infinity.
    pub fn floor(self) -> Dec {
        let whole = self.trunc();
        if self.unscaled < 0 && self.has_fraction() {
            Dec {
                unscaled: whole.unscaled - 1,
                scale: 0,
            }
        } else {
            whole
        }
    }

    /// Towards positive infinity.
    pub fn ceil(self) -> Dec {
        let whole = self.trunc();
        if self.unscaled > 0 && self.has_fraction() {
            Dec {
                unscaled: whole.unscaled + 1,
                scale: 0,
            }
        } else {
            whole
        }
    }

    /// To the nearest whole number, half away from zero.
    pub fn round(self) -> Dec {
        self.rescale(0).unwrap_or_else(|| self.trunc())
    }

    /// The same number at `scale`, or `None` if widening it overflows. A
    /// narrower scale rounds half away from zero.
    pub fn rescale(self, scale: i32) -> Option<Dec> {
        if scale == self.scale {
            return Some(self);
        }
        let step = (scale as i64 - self.scale as i64).unsigned_abs();
        if scale > self.scale {
            // Widening multiplies the digits, so a step past the 38 there
            // are is one no number can take.
            let factor = pow10(u32::try_from(step).ok()?)?;
            let unscaled = self.unscaled.checked_mul(factor)?;
            Dec::new(unscaled, scale)
        } else if step > MAX_PRECISION as u64 {
            // Narrowing by more than the digits there are leaves nothing of
            // them: every one lands after the point, and the first is a
            // zero, so the number rounds away.
            Dec::new(0, scale)
        } else {
            let factor = pow10(step as u32)?;
            let whole = self.unscaled / factor;
            let rest = self.unscaled % factor;
            // Half away from zero.
            let bump = if rest.unsigned_abs() * 2 >= factor.unsigned_abs() {
                if self.unscaled < 0 { -1 } else { 1 }
            } else {
                0
            };
            Dec::new(whole + bump, scale)
        }
    }

    /// The widest scale this number can be written at and still fit: it needs
    /// `magnitude + 1` digits before the point, and the rest are free. `1`
    /// reaches 37 places where `0.3` reaches 38, and `1e80` reaches `-43`,
    /// its point still 43 places right of its one digit. A column written at
    /// one scale, as a Parquet decimal is, can use no more than the least of
    /// these over its values, and no less than nothing.
    pub fn max_scale(self) -> i32 {
        if self.unscaled == 0 {
            return MAX_SCALE;
        }
        let places = MAX_PRECISION as i64 - 1 - self.magnitude();
        places.clamp(MIN_SCALE as i64, MAX_SCALE as i64) as i32
    }

    /// Both numbers at one scale, as wide as will hold them.
    fn align(a: Dec, b: Dec) -> Option<(i128, i128, i32)> {
        let (a, b) = (a.trimmed(), b.trimmed());
        // The wider of the two scales, brought down to one that holds them
        // both. Bringing it down rounds, which is the only way two numbers of
        // very different sizes can meet at a single scale at all.
        let scale = a.scale.max(b.scale).min(a.max_scale()).min(b.max_scale());
        Some((
            a.rescale(scale)?.unscaled,
            b.rescale(scale)?.unscaled,
            scale,
        ))
    }

    pub fn checked_add(self, other: Dec) -> Option<Dec> {
        let (x, y, scale) = Dec::align(self, other)?;
        match x.checked_add(y) {
            // Too wide at this scale: give up digits after the point.
            Some(sum) => Dec::new(sum, scale).or_else(|| {
                narrow(Dec {
                    unscaled: sum,
                    scale,
                })
            }),
            // The digits themselves do not add up: an `i128` holds a little
            // over `1.7e38`, where two 38-digit numbers reach `2e38`. Each
            // side gives up its last digit first, so the sum has the room
            // and the loss is the one `narrow` would have taken anyway.
            None => {
                let back = |n: i128| {
                    let (whole, rest) = (n / 10, n % 10);
                    whole
                        + if rest.unsigned_abs() * 2 >= 10 {
                            n.signum()
                        } else {
                            0
                        }
                };
                Dec::new(back(x).checked_add(back(y))?, scale.checked_sub(1)?)
            }
        }
    }

    pub fn checked_sub(self, other: Dec) -> Option<Dec> {
        self.checked_add(other.checked_neg()?)
    }

    pub fn checked_neg(self) -> Option<Dec> {
        Dec::new(self.unscaled.checked_neg()?, self.scale)
    }

    pub fn checked_abs(self) -> Option<Dec> {
        Dec::new(self.unscaled.checked_abs()?, self.scale)
    }

    pub fn checked_mul(self, other: Dec) -> Option<Dec> {
        let (a, b) = (self.trimmed(), other.trimmed());
        let scale = a.scale.checked_add(b.scale)?;
        match a.unscaled.checked_mul(b.unscaled) {
            Some(unscaled) => Dec::new(unscaled, scale).or_else(|| narrow(Dec { unscaled, scale })),
            // The product of the digits does not fit. Give up decimal places
            // on both sides, evenly, so that a wide result loses its last
            // digits rather than an operand losing all of them.
            None => {
                let excess =
                    digits(a.unscaled) as i64 + digits(b.unscaled) as i64 - MAX_PRECISION as i64;
                let half = excess.div_euclid(2).max(0) as i32;
                let a = a.rescale(a.scale - half - excess.rem_euclid(2) as i32)?;
                let b = b.rescale(b.scale - half)?;
                let scale = a.scale.checked_add(b.scale)?;
                let unscaled = a.unscaled.checked_mul(b.unscaled)?;
                narrow(Dec { unscaled, scale })
            }
        }
    }

    /// `self / other`, rounded half away from zero at the widest scale the
    /// result will hold. `None` when dividing by zero or it will not fit.
    pub fn checked_div(self, other: Dec) -> Option<Dec> {
        if other.is_zero() {
            return None;
        }
        let (a, b) = (self.trimmed(), other.trimmed());
        if a.is_zero() {
            return Some(Dec::ZERO);
        }

        // Room for the whole part, then as many decimal places as are left.
        // A quotient below one spends no digits on its whole part, so it gets
        // the full width; the loop below drops back if that does not fit.
        // How big the quotient is, and so how many decimal places are left
        // for it. Taken from the two magnitudes, not from dividing the
        // unscaled digits, which ignores where the points are.
        let e = a.magnitude() - b.magnitude();
        // The widest scale a quotient of this size can be written at, and
        // the narrowest worth trying: 38 steps below it every digit is
        // gone. Both may be negative, which is a quotient past 38 digits.
        let top = (MAX_PRECISION as i64 - e).clamp(MIN_SCALE as i64, MAX_SCALE as i64) as i32;
        let bottom = top.saturating_sub(MAX_PRECISION as i32).max(MIN_SCALE);

        // scale of a/b before rounding is a.scale - b.scale; lift the
        // numerator so the quotient lands at `want` places.
        for want in (bottom..=top).rev() {
            let lift = want as i64 + b.scale as i64 - a.scale as i64;
            let (num, den) = if lift >= 0 {
                match pow10(lift as u32).and_then(|f| a.unscaled.checked_mul(f)) {
                    Some(num) => (num, b.unscaled),
                    None => continue,
                }
            } else {
                match pow10((-lift) as u32).and_then(|f| b.unscaled.checked_mul(f)) {
                    Some(den) => (a.unscaled, den),
                    None => continue,
                }
            };
            let q = num / den;
            let rest = num % den;
            let bump = if rest.unsigned_abs() * 2 >= den.unsigned_abs() {
                if (num < 0) != (den < 0) { -1 } else { 1 }
            } else {
                0
            };
            if let Some(d) = Dec::new(q + bump, want) {
                // Trimmed, so that a quotient that terminates is not left
                // padded out to the widest scale and mistaken for a long one.
                return Some(d.trimmed());
            }
        }
        None
    }

    /// Whole-number power.
    pub fn powi(self, mut n: u32) -> Option<Dec> {
        let mut acc = Dec::ONE;
        let mut base = self;
        while n > 0 {
            if n & 1 == 1 {
                acc = acc.checked_mul(base)?;
            }
            n >>= 1;
            if n > 0 {
                base = base.checked_mul(base)?;
            }
        }
        Some(acc)
    }

    /// Two numbers compared without their guard digits, so that two ways of
    /// reaching the same number reach the same answer: see the module
    /// documentation.
    pub fn cmp_dec(self, other: Dec) -> Ordering {
        let (a, b) = (self.guarded(), other.guarded());
        match Dec::align(a, b) {
            Some((x, y, _)) => x.cmp(&y),
            // Beyond a common scale, sign and then size decide: two numbers
            // this far apart cannot have their digits meet, and nor do they
            // need to.
            None => a.signum().cmp(&b.signum()).then_with(|| {
                let order = a.magnitude().cmp(&b.magnitude());
                if a.is_negative() {
                    order.reverse()
                } else {
                    order
                }
            }),
        }
    }

    /// The digits with the point put in, no grouping and no sign dance:
    /// `-1.5`, `0.001`, `12`.
    pub fn to_plain_string(self) -> String {
        let d = self.trimmed();
        let negative = d.unscaled < 0;
        let digits = d.unscaled.unsigned_abs().to_string();
        if d.scale < 0 {
            // The point sits right of the digits: zeros fill the gap, and
            // past a readable width e notation stands in for them.
            let zeros = -d.scale as usize;
            let body = if digits.len() + zeros <= PLAIN_WIDTH {
                format!("{digits}{}", "0".repeat(zeros))
            } else {
                exponential(d)
            };
            return if negative { format!("-{body}") } else { body };
        }
        let scale = d.scale as usize;
        let body = if scale == 0 {
            digits
        } else if digits.len() > scale {
            let point = digits.len() - scale;
            format!("{}.{}", &digits[..point], &digits[point..])
        } else if scale - digits.len() + 2 <= PLAIN_WIDTH {
            format!("0.{}{}", "0".repeat(scale - digits.len()), digits)
        } else {
            // Too small to write out: as above, e notation stands in.
            exponential(d)
        };
        if negative { format!("-{body}") } else { body }
    }

    /// Reads `-12.34`, `12`, `.5` and `1e3`. `None` when it is not a number.
    pub fn parse(text: &str) -> Option<Dec> {
        let text = text.trim();
        let (mantissa, exp) = match text.find(['e', 'E']) {
            Some(i) => (&text[..i], text[i + 1..].parse::<i64>().ok()?),
            None => (text, 0),
        };
        let (negative, digits) = match mantissa.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, mantissa.strip_prefix('+').unwrap_or(mantissa)),
        };
        let (whole, frac) = match digits.split_once('.') {
            Some((w, f)) => (w, f),
            None => (digits, ""),
        };
        if whole.is_empty() && frac.is_empty() {
            return None;
        }
        if !whole
            .bytes()
            .chain(frac.bytes())
            .all(|b| b.is_ascii_digit())
        {
            return None;
        }
        let unscaled: i128 = format!("{whole}{frac}").parse().ok()?;
        let scale = i32::try_from(frac.len() as i64 - exp).ok()?;
        let d = if scale < 0 {
            // The digits written out, as a number read from text has always
            // been; where they will not fit, the point stays right of them.
            u32::try_from(-(scale as i64))
                .ok()
                .and_then(pow10)
                .and_then(|factor| unscaled.checked_mul(factor))
                .and_then(|unscaled| Dec::new(unscaled, 0))
                .or_else(|| Dec::new(unscaled, scale))?
        } else {
            Dec::new(unscaled, scale)?
        };
        if negative { d.checked_neg() } else { Some(d) }
    }
}

/// A number in e notation, signed as the app writes one: `1e+1000`.
fn exponential(d: Dec) -> String {
    let (digits, e) = d.digits_and_exponent();
    let mantissa = match digits.split_at(1) {
        (first, "") => first.to_string(),
        (first, rest) => format!("{first}.{rest}"),
    };
    format!("{mantissa}e{}{}", if e < 0 { '-' } else { '+' }, e.abs())
}

/// How many digits a magnitude is written with.
fn digits(n: i128) -> u32 {
    let mut n = n.unsigned_abs();
    let mut d = 1;
    while n >= 10 {
        n /= 10;
        d += 1;
    }
    d
}

/// A value too wide to hold, rounded back by giving up decimal places: the
/// point moves left, below zero where it must, so that a sum past 38 digits
/// keeps its significant ones and loses only its last. A second step is
/// there for the rounding that carries into a new digit, `999…9` to `100…0`.
fn narrow(wide: Dec) -> Option<Dec> {
    let excess = digits(wide.unscaled).saturating_sub(MAX_PRECISION).max(1);
    for back in excess..=excess + 1 {
        let scale = wide.scale.checked_sub(back as i32)?;
        if let Some(d) = wide.rescale(scale) {
            return Some(d);
        }
    }
    None
}

impl PartialEq for Dec {
    fn eq(&self, other: &Self) -> bool {
        self.cmp_dec(*other) == Ordering::Equal
    }
}

impl Eq for Dec {}

impl PartialOrd for Dec {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Dec {
    fn cmp(&self, other: &Self) -> Ordering {
        self.cmp_dec(*other)
    }
}

impl fmt::Display for Dec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_plain_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Dec {
        Dec::parse(s).expect("parses")
    }

    #[test]
    fn parses_and_shows() {
        for s in ["0", "1", "-1", "12.34", "-0.001", "100"] {
            assert_eq!(d(s).to_plain_string(), s, "{s}");
        }
        assert_eq!(d(".5").to_plain_string(), "0.5");
        assert_eq!(d("1e3").to_plain_string(), "1000");
        assert_eq!(d("1.5e2").to_plain_string(), "150");
        assert_eq!(d("12.3400").to_plain_string(), "12.34");
        assert!(Dec::parse("hello").is_none());
        assert!(Dec::parse("").is_none());
    }

    #[test]
    fn the_readme_case_is_exact() {
        // The floating-point headline: 0.1 + 0.2 is 0.3, not 0.30000000000000004.
        assert_eq!(
            d("0.1").checked_add(d("0.2")).unwrap().to_plain_string(),
            "0.3"
        );
    }

    #[test]
    fn adds_subtracts_and_multiplies_exactly() {
        assert_eq!(
            d("1.5").checked_add(d("2.25")).unwrap().to_plain_string(),
            "3.75"
        );
        assert_eq!(
            d("1").checked_sub(d("0.001")).unwrap().to_plain_string(),
            "0.999"
        );
        assert_eq!(d("1.5").checked_mul(d("4")).unwrap().to_plain_string(), "6");
        assert_eq!(
            d("0.2").checked_mul(d("0.1")).unwrap().to_plain_string(),
            "0.02"
        );
        assert_eq!(
            d("-2.5").checked_mul(d("2")).unwrap().to_plain_string(),
            "-5"
        );
    }

    #[test]
    fn divides_exactly_where_it_can() {
        assert_eq!(d("1").checked_div(d("2")).unwrap().to_plain_string(), "0.5");
        assert_eq!(
            d("1").checked_div(d("8")).unwrap().to_plain_string(),
            "0.125"
        );
        assert_eq!(d("10").checked_div(d("5")).unwrap().to_plain_string(), "2");
        assert_eq!(
            d("-1").checked_div(d("4")).unwrap().to_plain_string(),
            "-0.25"
        );
        assert!(d("1").checked_div(Dec::ZERO).is_none());
    }

    #[test]
    fn thirds_round_at_the_widest_scale() {
        let third = d("1").checked_div(d("3")).unwrap();
        assert_eq!(third.scale(), MAX_PRECISION as i32);
        assert_eq!(
            third.to_plain_string(),
            "0.33333333333333333333333333333333333333"
        );
        // Documented loss: a third times three is all nines, not one.
        assert_eq!(
            third.checked_mul(d("3")).unwrap().to_plain_string(),
            "0.99999999999999999999999999999999999999"
        );
    }

    #[test]
    fn a_third_plus_a_sixth_is_a_half() {
        // The README's `omasheet eval '1/3 + 1/6'`: the rounding cancels.
        let third = d("1").checked_div(d("3")).unwrap();
        let sixth = d("1").checked_div(d("6")).unwrap();
        assert_eq!(third.checked_add(sixth).unwrap().to_plain_string(), "0.5");
    }

    #[test]
    fn the_guard_digits_are_neither_compared_nor_shown() {
        let seventh = |n: i64| Dec::from_i64(n).checked_div(d("7")).unwrap();
        // Held, a seventh of one doubled and a seventh of two differ in
        // their last digit, since the two round opposite ways.
        assert_ne!(
            seventh(1)
                .checked_add(seventh(1))
                .unwrap()
                .to_plain_string(),
            seventh(2).to_plain_string()
        );
        // Without their guard digits they are one number.
        assert_eq!(seventh(1).checked_add(seventh(1)).unwrap(), seventh(2));
        assert_eq!(
            seventh(2).checked_add(seventh(3)).unwrap(),
            seventh(5),
            "2/7 + 3/7 == 5/7"
        );
        let third = d("1").checked_div(d("3")).unwrap();
        assert_eq!(third.checked_mul(d("3")).unwrap(), Dec::ONE);
        assert_eq!(
            third
                .checked_mul(d("3"))
                .unwrap()
                .guarded()
                .to_plain_string(),
            "1"
        );
        // Thirty significant digits are shown, of the thirty-eight held.
        assert_eq!(third.precision(), MAX_PRECISION);
        assert_eq!(third.guarded().precision(), SHOWN_PRECISION);
        assert!(third.is_long());
        // A number that ends is untouched, and so is the smallest there is,
        // whose every digit is significant.
        for text in ["19.99", "0.125", "0", "-7", "1e-38"] {
            assert_eq!(d(text).guarded(), d(text), "{text}");
            assert!(!d(text).is_long(), "{text}");
        }
    }

    #[test]
    fn fractions_are_read_back_out() {
        let frac = |text: &str| d(text).to_fraction(MAX_DENOMINATOR);
        assert_eq!(frac("0.125"), Some((1, 8)));
        assert_eq!(frac("2.75"), Some((11, 4)));
        assert_eq!(frac("-2.75"), Some((-11, 4)));
        assert_eq!(frac("3"), Some((3, 1)));
        assert_eq!(frac("0"), Some((0, 1)));
        // A seventh is rounded to be held, and comes back a seventh.
        let seventh = d("1").checked_div(d("7")).unwrap();
        assert_eq!(seventh.to_fraction(MAX_DENOMINATOR), Some((1, 7)));
        let five_sevenths = d("5").checked_div(d("7")).unwrap();
        assert_eq!(five_sevenths.to_fraction(MAX_DENOMINATOR), Some((5, 7)));
        // Past the denominator asked for there is no answer.
        assert_eq!(
            d("1").checked_div(d("10001")).unwrap().to_fraction(10_000),
            None
        );
        assert_eq!(
            d("1").checked_div(d("10000")).unwrap().to_fraction(10_000),
            Some((1, 10_000))
        );
    }

    #[test]
    fn compares_across_scales() {
        assert_eq!(d("1.50"), d("1.5"));
        assert_eq!(d("0"), d("-0"));
        assert!(d("1.5") < d("1.75"));
        assert!(d("-1") < d("0"));
        assert!(d("10") > d("9.99999"));
    }

    #[test]
    fn whole_numbers_know_themselves() {
        assert_eq!(d("12.00").to_i64(), Some(12));
        assert_eq!(d("12.5").to_i64(), None);
        assert!(d("12.000").is_integer());
        assert!(!d("12.5").is_integer());
        assert_eq!(Dec::from_i64(-7).to_plain_string(), "-7");
    }

    #[test]
    fn powers_multiply_out() {
        assert_eq!(d("2").powi(10).unwrap().to_plain_string(), "1024");
        assert_eq!(d("1.5").powi(2).unwrap().to_plain_string(), "2.25");
        assert_eq!(d("10").powi(0).unwrap().to_plain_string(), "1");
    }

    #[test]
    fn the_precision_ceiling_is_respected() {
        // 38 nines is the widest value there is.
        let wide = Dec::parse(&"9".repeat(38)).expect("38 digits fit");
        assert_eq!(wide.scale(), 0);
        assert!(Dec::parse(&"9".repeat(39)).is_none());
        // Adding past the ceiling moves the point right of the digits
        // rather than wrapping: the answer keeps the 38 digits it holds and
        // its last one is the loss.
        let sum = wide.checked_add(wide).expect("the point has room");
        assert_eq!(sum.scale(), -1);
        assert_eq!(sum.magnitude(), 38);
        assert!(Dec::new(LIMIT, 0).is_none());
    }

    #[test]
    fn rescaling_rounds_half_away_from_zero() {
        assert_eq!(d("1.25").rescale(1).unwrap().to_plain_string(), "1.3");
        assert_eq!(d("-1.25").rescale(1).unwrap().to_plain_string(), "-1.3");
        assert_eq!(d("1.24").rescale(1).unwrap().to_plain_string(), "1.2");
        assert_eq!(d("1.5").rescale(0).unwrap().to_plain_string(), "2");
    }

    #[test]
    fn reads_floats_at_their_shortest() {
        assert_eq!(Dec::from_f64(0.1).unwrap().to_plain_string(), "0.1");
        assert_eq!(Dec::from_f64(1.5e3).unwrap().to_plain_string(), "1500");
        assert_eq!(Dec::from_f64(-0.25).unwrap().to_plain_string(), "-0.25");
        assert_eq!(
            Dec::from_f64(1.0 / 3.0).unwrap().to_plain_string(),
            "0.3333333333333333"
        );
        assert!(Dec::from_f64(f64::INFINITY).is_none());
        assert!(Dec::from_f64(f64::NAN).is_none());
    }

    #[test]
    fn splits_into_digits_and_an_exponent() {
        assert_eq!(d("19.99").digits_and_exponent(), ("1999".into(), 1));
        assert_eq!(d("1000").digits_and_exponent(), ("1".into(), 3));
        assert_eq!(d("0.001").digits_and_exponent(), ("1".into(), -3));
        assert_eq!(d("-7").digits_and_exponent(), ("7".into(), 0));
        assert_eq!(Dec::ZERO.digits_and_exponent(), ("0".into(), 0));
        let third = d("1").checked_div(d("3")).unwrap();
        assert_eq!(third.magnitude(), -1);
        assert_eq!(d("1000").magnitude(), 3);
    }

    #[test]
    fn a_value_rounded_at_the_limit_reads_as_long() {
        assert!(d("1").checked_div(d("3")).unwrap().is_long());
        assert!(d("1").checked_div(d("7")).unwrap().is_long());
        assert!(!d("0.125").is_long());
        assert!(!d("1").checked_div(d("8")).unwrap().is_long());
    }

    #[test]
    fn rounds_to_whole_numbers_each_way() {
        for (s, trunc, floor, ceil, round) in [
            ("2.75", "2", "2", "3", "3"),
            ("-2.75", "-2", "-3", "-2", "-3"),
            ("2.25", "2", "2", "3", "2"),
            ("-7", "-7", "-7", "-7", "-7"),
        ] {
            assert_eq!(d(s).trunc().to_plain_string(), trunc, "trunc {s}");
            assert_eq!(d(s).floor().to_plain_string(), floor, "floor {s}");
            assert_eq!(d(s).ceil().to_plain_string(), ceil, "ceil {s}");
            assert_eq!(d(s).round().to_plain_string(), round, "round {s}");
        }
        assert_eq!(d("-2.75").signum(), -1);
        assert_eq!(Dec::ZERO.signum(), 0);
    }

    #[test]
    fn percentages_are_ordinary_numbers() {
        // 20% is a fifth: the display is elsewhere, the number is this.
        let fifth = d("20").checked_div(d("100")).unwrap();
        assert_eq!(fifth.to_plain_string(), "0.2");
        assert_eq!(
            d("10000").checked_mul(fifth).unwrap().to_plain_string(),
            "2000"
        );
    }

    #[test]
    fn a_number_past_38_digits_keeps_its_point_right_of_them() {
        // `1e80` is one digit at a scale of -80, which no Polars decimal
        // names and this type does.
        let big = d("1e80");
        assert_eq!(big.scale(), -80);
        assert_eq!(big.magnitude(), 80);
        // Avogadro's number, whole and exact.
        let mole = d("6.02214076e23");
        assert_eq!(mole.to_plain_string(), "602214076000000000000000");
        // Added to, a number this size swallows a small one: the two
        // cannot meet at any scale, and the answer is the larger of them.
        assert_eq!(big.checked_add(d("1")).unwrap(), big);
        // Multiplied, the scales add, so the product is further out still.
        let square = big.checked_mul(big).unwrap();
        assert_eq!(square.magnitude(), 160);
        assert_eq!(square.scale(), -160);
        // And divided back down again.
        assert_eq!(square.checked_div(big).unwrap(), big);
        // The atoms in the universe, which 38 digits could not hold. Too
        // wide to write out, so e notation stands in for the zeros.
        assert_eq!(d("1e80").to_plain_string(), "1e+80");
    }

    #[test]
    fn a_sum_past_38_digits_gives_up_its_last_digits_not_its_answer() {
        // Two 38-digit numbers added: the sum needs 39 digits, so the point
        // moves right of them and the last digit is the one given up.
        let wide = d(&"9".repeat(38));
        let sum = wide.checked_add(wide).unwrap();
        assert_eq!(sum.magnitude(), 38);
        assert_eq!(sum.scale(), -1);
        assert_eq!(
            sum.to_plain_string(),
            "200000000000000000000000000000000000000"
        );
    }

    #[test]
    fn the_scale_runs_out_in_the_end() {
        // The largest number there is: 38 digits with the point as far
        // right of them as it reaches. A tenth more has nowhere to go, and
        // it says so rather than wrapping or rounding.
        let largest = Dec::new(LIMIT - 1, MIN_SCALE).expect("fits");
        assert_eq!(largest.magnitude(), MAX_SCALE as i64 + 37);
        assert!(largest.checked_mul(d("10")).is_none());
        assert!(largest.checked_add(largest).is_none());
        // At the other end a quotient rounds away to nothing, as one below
        // the narrowest scale always has.
        let smallest = Dec::new(1, MAX_SCALE).expect("fits");
        assert!(smallest.checked_div(d("10")).expect("rounds").is_zero());
    }
}
