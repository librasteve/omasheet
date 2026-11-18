// Copyright (c) 2026 Stephen Roe

//! The exact rational behind the `Fraction` type: a numerator and a
//! denominator of arbitrary-precision integers, always reduced, the
//! denominator positive. It is Raku's `FatRat`: nothing in it is ever
//! rounded, nothing falls back to a float, and a googol and its reciprocal
//! sit in one column. It is only the engine's: no dataframe has a dtype for
//! it, so a column of them is never lowered.

use crate::decimal::Dec;
use num_bigint::{BigInt, Sign};
use num_integer::Integer;
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};
use std::cmp::Ordering;
use std::fmt;

/// A rational number.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Rat(BigRational);

impl Rat {
    pub fn zero() -> Rat {
        Rat(BigRational::zero())
    }

    pub fn from_i64(n: i64) -> Rat {
        Rat(BigRational::from_integer(BigInt::from(n)))
    }

    pub fn from_i128(n: i128) -> Rat {
        Rat(BigRational::from_integer(BigInt::from(n)))
    }

    /// `n / d`, or `None` for a zero denominator.
    pub fn new(n: i128, d: i128) -> Option<Rat> {
        (d != 0).then(|| Rat(BigRational::new(BigInt::from(n), BigInt::from(d))))
    }

    pub fn from_big(n: BigInt, d: BigInt) -> Option<Rat> {
        (!d.is_zero()).then(|| Rat(BigRational::new(n, d)))
    }

    /// The decimal `d`, exactly.
    pub fn from_dec(d: Dec) -> Rat {
        let unscaled = BigInt::from(d.unscaled());
        let ten = BigInt::from(10);
        let scale = d.scale();
        if scale >= 0 {
            Rat(BigRational::new(unscaled, ten.pow(scale as u32)))
        } else {
            Rat(BigRational::from_integer(
                unscaled * ten.pow(scale.unsigned_abs()),
            ))
        }
    }

    /// The float's own value, exactly: `0.1` is the fraction a double holds,
    /// not a tenth. `None` for a `NaN` or an infinity.
    pub fn from_f64(f: f64) -> Option<Rat> {
        BigRational::from_float(f).map(Rat)
    }

    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    pub fn is_integer(&self) -> bool {
        self.0.is_integer()
    }

    pub fn is_negative(&self) -> bool {
        self.0.is_negative()
    }

    pub fn signum(&self) -> i32 {
        match self.0.numer().sign() {
            Sign::Minus => -1,
            Sign::NoSign => 0,
            Sign::Plus => 1,
        }
    }

    pub fn numer(&self) -> &BigInt {
        self.0.numer()
    }

    /// The numerator and denominator, where both are small.
    pub fn parts(&self) -> Option<(i64, i64)> {
        Some((self.0.numer().to_i64()?, self.0.denom().to_i64()?))
    }

    pub fn denom(&self) -> &BigInt {
        self.0.denom()
    }

    pub fn add(&self, other: &Rat) -> Rat {
        Rat(&self.0 + &other.0)
    }

    pub fn sub(&self, other: &Rat) -> Rat {
        Rat(&self.0 - &other.0)
    }

    pub fn mul(&self, other: &Rat) -> Rat {
        Rat(&self.0 * &other.0)
    }

    /// `self / other`, or `None` dividing by zero.
    pub fn div(&self, other: &Rat) -> Option<Rat> {
        (!other.is_zero()).then(|| Rat(&self.0 / &other.0))
    }

    pub fn neg(&self) -> Rat {
        Rat(-&self.0)
    }

    pub fn abs(&self) -> Rat {
        Rat(self.0.abs())
    }

    /// A whole-number power, exactly; a negative one is the reciprocal, and
    /// `None` for zero to a negative power.
    pub fn powi(&self, n: i32) -> Option<Rat> {
        if n >= 0 {
            Some(Rat(num_traits::pow::Pow::pow(&self.0, n as u32)))
        } else if self.is_zero() {
            None
        } else {
            Some(Rat(num_traits::pow::Pow::pow(
                &self.0.recip(),
                n.unsigned_abs(),
            )))
        }
    }

    pub fn floor(&self) -> Rat {
        Rat(self.0.floor())
    }

    pub fn ceil(&self) -> Rat {
        Rat(self.0.ceil())
    }

    pub fn trunc(&self) -> Rat {
        Rat(self.0.trunc())
    }

    /// The nearest whole number, a half away from zero.
    pub fn round(&self) -> Rat {
        Rat(self.0.round())
    }

    pub fn to_i64(&self) -> Option<i64> {
        self.0
            .is_integer()
            .then(|| self.0.numer().to_i64())
            .flatten()
    }

    /// The nearest `f64`.
    pub fn to_f64(&self) -> f64 {
        self.0.to_f64().unwrap_or_else(|| {
            // Past a double either way: the sign of an infinity or a zero.
            let big = self.0.numer().bits() > self.0.denom().bits();
            match (big, self.is_negative()) {
                (true, false) => f64::INFINITY,
                (true, true) => f64::NEG_INFINITY,
                _ => 0.0,
            }
        })
    }

    /// The value as a `Decimal` of `scale` places, rounded half away from
    /// zero; `None` where it needs more than 38 digits there.
    pub fn to_dec(&self, scale: i32) -> Option<Dec> {
        if !(0..=38).contains(&scale) {
            return None;
        }
        let factor = BigInt::from(10).pow(scale as u32);
        let scaled = self.0.numer() * &factor;
        let (q, r) = scaled.div_rem(self.0.denom());
        // Half away from zero: compare twice the remainder with the divisor.
        let twice = r.abs() * 2;
        let bump = if twice >= *self.0.denom() {
            self.0.numer().signum()
        } else {
            BigInt::zero()
        };
        let unscaled: i128 = (q + bump).to_i128()?;
        Dec::new(unscaled, scale)
    }

    /// The value as a `Decimal`, if it ends within 38 places and fits.
    pub fn to_dec_exact(&self) -> Option<Dec> {
        // A denominator of only twos and fives ends; count the places.
        let mut d = self.0.denom().clone();
        let (two, five) = (BigInt::from(2), BigInt::from(5));
        let (mut twos, mut fives) = (0u32, 0u32);
        while (&d % &two).is_zero() {
            d /= &two;
            twos += 1;
        }
        while (&d % &five).is_zero() {
            d /= &five;
            fives += 1;
        }
        if !d.is_one() {
            return None;
        }
        let places = twos.max(fives);
        let dec = self.to_dec(i32::try_from(places).ok()?)?;
        (Rat::from_dec(dec) == *self).then_some(dec)
    }

    /// The fraction as text: `3/4`, `-11/4`, or `7` where it is whole.
    pub fn fraction_text(&self) -> String {
        if self.0.is_integer() {
            self.0.numer().to_string()
        } else {
            format!("{}/{}", self.0.numer(), self.0.denom())
        }
    }

    /// The fraction as a whole number and a part where it is more than one:
    /// `2 3/4` for `11/4`, `-2 3/4`, and a plain `3/4` otherwise.
    pub fn mixed_text(&self) -> String {
        if self.0.is_integer() {
            return self.0.numer().to_string();
        }
        let (whole, part) = (
            self.0.numer().abs() / self.0.denom(),
            self.0.numer().abs() % self.0.denom(),
        );
        let sign = if self.is_negative() { "-" } else { "" };
        if whole.is_zero() {
            format!("{sign}{part}/{}", self.0.denom())
        } else {
            format!("{sign}{whole} {part}/{}", self.0.denom())
        }
    }

    /// Whether the denominator is at most `limit`: a fraction a person reads.
    pub fn small_denominator(&self, limit: u64) -> bool {
        self.0.denom().to_u64().is_some_and(|d| d <= limit)
    }

    /// `n/d` or `a b/c` read back, with an optional sign: `1/7`, `2 3/4`,
    /// `-1/3`. `None` for anything else.
    pub fn parse(text: &str) -> Option<Rat> {
        let text = text.trim();
        let (neg, body) = match text.strip_prefix('-') {
            Some(rest) => (true, rest.trim_start()),
            None => (false, text.strip_prefix('+').unwrap_or(text)),
        };
        let whole_and = body.split_once(' ');
        let (whole, frac) = match whole_and {
            Some((w, f)) if f.contains('/') => (Some(w), f.trim()),
            _ => (None, body),
        };
        let (n, d) = frac.split_once('/')?;
        let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
        if !digits(n.trim()) || !digits(d.trim()) {
            return None;
        }
        let n: BigInt = n.trim().parse().ok()?;
        let d: BigInt = d.trim().parse().ok()?;
        if d.is_zero() {
            return None;
        }
        let mut r = BigRational::new(n, d);
        if let Some(w) = whole {
            if !digits(w) {
                return None;
            }
            r += BigRational::from_integer(w.parse::<BigInt>().ok()?);
        }
        Some(Rat(if neg { -r } else { r }))
    }
}

impl PartialOrd for Rat {
    fn partial_cmp(&self, other: &Rat) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Rat {
    fn cmp(&self, other: &Rat) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl fmt::Display for Rat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.fraction_text())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rat(n: i128, d: i128) -> Rat {
        Rat::new(n, d).unwrap()
    }

    #[test]
    fn a_third_is_exact() {
        let third = rat(1, 3);
        assert_eq!(third.mul(&Rat::from_i64(3)), Rat::from_i64(1));
        assert_eq!(rat(1, 7).add(&rat(1, 7)), rat(2, 7));
        assert_eq!(rat(2, 7).add(&rat(3, 7)), rat(5, 7));
    }

    #[test]
    fn a_googol_and_its_reciprocal_are_held() {
        let ten = Rat::from_i64(10);
        let googol = ten.powi(100).unwrap();
        let tiny = ten.powi(-80).unwrap();
        assert_eq!(googol.mul(&tiny), ten.powi(20).unwrap());
        assert_eq!(googol.fraction_text().len(), 101);
        assert!(tiny.fraction_text().starts_with("1/1000"));
    }

    #[test]
    fn a_decimal_converts_exactly_and_back_rounded() {
        let d = Dec::parse("19.99").unwrap();
        assert_eq!(Rat::from_dec(d), rat(1999, 100));
        assert_eq!(rat(1, 3).to_dec(4).unwrap().to_plain_string(), "0.3333");
        assert_eq!(rat(2, 3).to_dec(4).unwrap().to_plain_string(), "0.6667");
        assert_eq!(rat(-1, 8).to_dec(2).unwrap().to_plain_string(), "-0.13");
        assert_eq!(rat(3, 8).to_dec_exact().unwrap().to_plain_string(), "0.375");
        assert!(rat(1, 3).to_dec_exact().is_none());
    }

    #[test]
    fn it_shows_and_reads_fractions() {
        assert_eq!(rat(11, 4).mixed_text(), "2 3/4");
        assert_eq!(rat(-11, 4).mixed_text(), "-2 3/4");
        assert_eq!(rat(3, 4).mixed_text(), "3/4");
        assert_eq!(rat(8, 4).fraction_text(), "2");
        assert_eq!(Rat::parse("2 3/4"), Some(rat(11, 4)));
        assert_eq!(Rat::parse("-1/3"), Some(rat(-1, 3)));
        assert_eq!(Rat::parse("1/0"), None);
        assert_eq!(Rat::parse("1.5"), None);
    }

    #[test]
    fn it_orders_and_rounds() {
        assert!(rat(1, 3) < rat(1, 2));
        assert_eq!(rat(5, 2).round(), Rat::from_i64(3));
        assert_eq!(rat(-5, 2).round(), Rat::from_i64(-3));
        assert_eq!(rat(7, 2).floor(), Rat::from_i64(3));
        assert_eq!(rat(7, 2).ceil(), Rat::from_i64(4));
    }
}
