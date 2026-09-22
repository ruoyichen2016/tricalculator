//! Exact rational arithmetic for rules values.
//!
//! PCGen rules contain half ranks, 3/4 BAB, fractional saves, and similar
//! values that must not be rounded while the engine is still combining
//! sources.  This type keeps those values exact and only rounds at the
//! display/rule boundary.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

/// Exact rational number backed by a normalized `i64` numerator/denominator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rational {
    numerator: i64,
    denominator: i64,
}

impl Rational {
    pub const ZERO: Self = Self {
        numerator: 0,
        denominator: 1,
    };
    pub const ONE: Self = Self {
        numerator: 1,
        denominator: 1,
    };

    /// Create a normalized rational.  A zero denominator is an error.
    pub fn new(numerator: i64, denominator: i64) -> Result<Self, RationalError> {
        if denominator == 0 {
            return Err(RationalError::ZeroDenominator);
        }
        let sign = if denominator < 0 { -1 } else { 1 };
        let mut n = numerator.saturating_mul(sign);
        let mut d = denominator.saturating_mul(sign);
        let divisor = gcd(n.unsigned_abs(), d.unsigned_abs()).max(1) as i64;
        n /= divisor;
        d /= divisor;
        Ok(Self {
            numerator: n,
            denominator: d,
        })
    }

    pub const fn from_integer(value: i64) -> Self {
        Self {
            numerator: value,
            denominator: 1,
        }
    }

    pub const fn numerator(self) -> i64 {
        self.numerator
    }

    pub const fn denominator(self) -> i64 {
        self.denominator
    }

    pub fn is_zero(self) -> bool {
        self.numerator == 0
    }

    pub fn is_integer(self) -> bool {
        self.denominator == 1
    }

    /// Floor toward negative infinity.
    pub fn floor(self) -> i64 {
        if self.numerator >= 0 {
            self.numerator / self.denominator
        } else {
            -((-self.numerator + self.denominator - 1) / self.denominator)
        }
    }

    /// Ceiling toward positive infinity.
    pub fn ceil(self) -> i64 {
        -(-self).floor()
    }

    /// Round half away from zero, matching the common d20 table convention.
    pub fn round(self) -> i64 {
        let sign = if self.numerator < 0 { -1 } else { 1 };
        let absolute = self.numerator.unsigned_abs();
        let denominator = self.denominator.unsigned_abs();
        sign * ((absolute + denominator / 2) / denominator) as i64
    }

    pub fn checked_add(self, other: Self) -> Option<Self> {
        let n = self
            .numerator
            .checked_mul(other.denominator)?
            .checked_add(other.numerator.checked_mul(self.denominator)?)?;
        let d = self.denominator.checked_mul(other.denominator)?;
        Self::new(n, d).ok()
    }

    pub fn checked_sub(self, other: Self) -> Option<Self> {
        self.checked_add(-other)
    }

    pub fn checked_mul(self, other: Self) -> Option<Self> {
        let n = self.numerator.checked_mul(other.numerator)?;
        let d = self.denominator.checked_mul(other.denominator)?;
        Self::new(n, d).ok()
    }

    pub fn checked_div(self, other: Self) -> Option<Self> {
        if other.is_zero() {
            return None;
        }
        let n = self.numerator.checked_mul(other.denominator)?;
        let d = self.denominator.checked_mul(other.numerator)?;
        Self::new(n, d).ok()
    }
}

impl Default for Rational {
    fn default() -> Self {
        Self::ZERO
    }
}

impl fmt::Display for Rational {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.denominator == 1 {
            write!(f, "{}", self.numerator)
        } else {
            write!(f, "{}/{}", self.numerator, self.denominator)
        }
    }
}

impl From<i32> for Rational {
    fn from(value: i32) -> Self {
        Self::from_integer(value as i64)
    }
}

impl From<i64> for Rational {
    fn from(value: i64) -> Self {
        Self::from_integer(value)
    }
}

impl From<Rational> for f64 {
    fn from(value: Rational) -> Self {
        value.numerator as f64 / value.denominator as f64
    }
}

impl Neg for Rational {
    type Output = Self;
    fn neg(self) -> Self::Output {
        Self {
            numerator: -self.numerator,
            denominator: self.denominator,
        }
    }
}

impl Add for Rational {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        self.checked_add(rhs).unwrap_or(Self::ZERO)
    }
}

impl Sub for Rational {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        self.checked_sub(rhs).unwrap_or(Self::ZERO)
    }
}

impl Mul for Rational {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self::Output {
        self.checked_mul(rhs).unwrap_or(Self::ZERO)
    }
}

impl Div for Rational {
    type Output = Self;
    fn div(self, rhs: Self) -> Self::Output {
        self.checked_div(rhs).unwrap_or(Self::ZERO)
    }
}

impl AddAssign for Rational {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl SubAssign for Rational {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl PartialOrd for Rational {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Rational {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let left = i128::from(self.numerator) * i128::from(other.denominator);
        let right = i128::from(other.numerator) * i128::from(self.denominator);
        left.cmp(&right)
    }
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let remainder = a % b;
        a = b;
        b = remainder;
    }
    a
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RationalError {
    #[error("rational denominator must not be zero")]
    ZeroDenominator,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_and_rounds_exactly() {
        let half = Rational::new(1, 2).unwrap();
        assert_eq!(half.to_string(), "1/2");
        assert_eq!((half + half).to_string(), "1");
        assert_eq!(Rational::new(3, 2).unwrap().floor(), 1);
        assert_eq!(Rational::new(3, 2).unwrap().ceil(), 2);
        assert_eq!(Rational::new(-3, 2).unwrap().floor(), -2);
        assert_eq!(Rational::new(5, 2).unwrap().round(), 3);
        assert_eq!(Rational::new(7, 2).unwrap().round(), 4);
    }

    #[test]
    fn rejects_zero_denominator() {
        assert_eq!(Rational::new(1, 0), Err(RationalError::ZeroDenominator));
    }
}
