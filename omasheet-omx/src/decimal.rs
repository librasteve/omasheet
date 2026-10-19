// Copyright (c) 2026 Stephen Roe

//! A fixed-point decimal, held the way Polars holds one: an `i128` of
//! unscaled digits and a scale saying where the point sits, so that
//! `unscaled / 10^scale` is the number. Thirty-eight digits is the most a
//! `Decimal128` carries, and every operation here keeps inside that or says
//! it cannot.
//!
//! Arithmetic is exact wherever the result fits: adding and subtracting line
//! the scales up, multiplying adds them. Division is the one operation that
//! need not terminate, so it rounds half away from zero at the widest scale
//! the result can hold, which is why `1/3 * 3` is all nines rather than one.

use std::cmp::Ordering;
use std::fmt;

/// The most digits a `Decimal128` holds.
pub const MAX_PRECISION: u32 = 38;

/// `10^MAX_PRECISION`, the first value too wide to store.
const LIMIT: i128 = 10i128.pow(MAX_PRECISION);

/// A number as `unscaled / 10^scale`.
#[derive(Clone, Copy, Debug)]
pub struct Dec {
    unscaled: i128,
    scale: u32,
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
    pub fn new(unscaled: i128, scale: u32) -> Option<Dec> {
        if scale > MAX_PRECISION || unscaled.unsigned_abs() >= LIMIT as u128 {
            None
        } else {
            Some(Dec { unscaled, scale })
        }
    }

    pub fn unscaled(self) -> i128 {
        self.unscaled
    }

    pub fn scale(self) -> u32 {
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
        self.trimmed().scale == 0
    }

    /// The same number with no needless trailing zeros.
    pub fn trimmed(self) -> Dec {
        let mut d = self;
        while d.scale > 0 && d.unscaled % 10 == 0 {
            d.unscaled /= 10;
            d.scale -= 1;
        }
        if d.unscaled == 0 {
            d.scale = 0;
        }
        d
    }

    /// This number as a whole number, when it is one and it fits.
    pub fn to_i64(self) -> Option<i64> {
        let d = self.trimmed();
        if d.scale == 0 {
            i64::try_from(d.unscaled).ok()
        } else {
            None
        }
    }

    pub fn to_f64(self) -> f64 {
        self.unscaled as f64 / 10f64.powi(self.scale as i32)
    }

    /// The same number at `scale`, or `None` if widening it overflows. A
    /// narrower scale rounds half away from zero.
    pub fn rescale(self, scale: u32) -> Option<Dec> {
        if scale == self.scale {
            return Some(self);
        }
        if scale > self.scale {
            let factor = pow10(scale - self.scale)?;
            let unscaled = self.unscaled.checked_mul(factor)?;
            Dec::new(unscaled, scale)
        } else {
            let factor = pow10(self.scale - scale)?;
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

    /// Both numbers at one scale, as wide as will hold them.
    fn align(a: Dec, b: Dec) -> Option<(i128, i128, u32)> {
        let scale = a.scale.max(b.scale);
        match (a.rescale(scale), b.rescale(scale)) {
            (Some(x), Some(y)) => Some((x.unscaled, y.unscaled, scale)),
            // Lining them up overflowed, so trim what we can and try again.
            _ => {
                let (a, b) = (a.trimmed(), b.trimmed());
                let scale = a.scale.max(b.scale);
                Some((
                    a.rescale(scale)?.unscaled,
                    b.rescale(scale)?.unscaled,
                    scale,
                ))
            }
        }
    }

    pub fn checked_add(self, other: Dec) -> Option<Dec> {
        let (x, y, scale) = Dec::align(self, other)?;
        Dec::new(x.checked_add(y)?, scale).or_else(|| {
            // Too wide at this scale: give up digits after the point.
            let wide = Dec {
                unscaled: x.checked_add(y)?,
                scale,
            };
            narrow(wide)
        })
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
        let scale = a.scale + b.scale;
        match a.unscaled.checked_mul(b.unscaled) {
            Some(unscaled) if scale <= MAX_PRECISION => {
                Dec::new(unscaled, scale).or_else(|| narrow(Dec { unscaled, scale }))
            }
            Some(unscaled) => narrow(Dec { unscaled, scale }),
            // The product of the digits does not fit; round an operand down
            // and multiply again so a wide result degrades rather than fails.
            None => {
                let room = MAX_PRECISION.saturating_sub(digits(b.unscaled));
                let a = a.rescale(a.scale.min(room))?;
                let scale = a.scale + b.scale;
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
        let whole = (a.unscaled / b.unscaled).unsigned_abs();
        let room = if whole == 0 {
            MAX_PRECISION
        } else {
            MAX_PRECISION.saturating_sub(digits(whole as i128))
        };

        // scale of a/b before rounding is a.scale - b.scale; lift the
        // numerator so the quotient lands at `want` places.
        for want in (0..=room).rev() {
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
                return Some(d);
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

    pub fn cmp_dec(self, other: Dec) -> Ordering {
        match Dec::align(self, other) {
            Some((x, y, _)) => x.cmp(&y),
            // Beyond a common scale, the wider magnitude decides.
            None => self.to_f64().total_cmp(&other.to_f64()),
        }
    }

    /// The digits with the point put in, no grouping and no sign dance:
    /// `-1.5`, `0.001`, `12`.
    pub fn to_plain_string(self) -> String {
        let d = self.trimmed();
        let negative = d.unscaled < 0;
        let digits = d.unscaled.unsigned_abs().to_string();
        let scale = d.scale as usize;
        let body = if scale == 0 {
            digits
        } else if digits.len() > scale {
            let point = digits.len() - scale;
            format!("{}.{}", &digits[..point], &digits[point..])
        } else {
            format!("0.{}{}", "0".repeat(scale - digits.len()), digits)
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
        let scale = frac.len() as i64 - exp;
        let d = if scale < 0 {
            Dec::new(unscaled, 0)?.rescale(0)?;
            let factor = pow10((-scale) as u32)?;
            Dec::new(unscaled.checked_mul(factor)?, 0)?
        } else {
            Dec::new(unscaled, u32::try_from(scale).ok()?)?
        };
        if negative { d.checked_neg() } else { Some(d) }
    }
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

/// A value too wide to hold, rounded back by giving up decimal places.
fn narrow(wide: Dec) -> Option<Dec> {
    for scale in (0..wide.scale).rev() {
        let factor = pow10(wide.scale - scale)?;
        let whole = wide.unscaled / factor;
        let rest = wide.unscaled % factor;
        let bump = if rest.unsigned_abs() * 2 >= factor.unsigned_abs() {
            if wide.unscaled < 0 { -1 } else { 1 }
        } else {
            0
        };
        if let Some(d) = Dec::new(whole + bump, scale) {
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
        assert_eq!(third.scale(), MAX_PRECISION);
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
        // Adding past the ceiling fails rather than wrapping.
        assert!(wide.checked_add(wide).is_none());
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
    fn percentages_are_ordinary_numbers() {
        // 20% is a fifth: the display is elsewhere, the number is this.
        let fifth = d("20").checked_div(d("100")).unwrap();
        assert_eq!(fifth.to_plain_string(), "0.2");
        assert_eq!(
            d("10000").checked_mul(fifth).unwrap().to_plain_string(),
            "2000"
        );
    }
}
