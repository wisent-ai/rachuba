//! Exact money arithmetic.
//!
//! Every monetary value in this crate is an integer number of cents. Floating
//! point never touches a dollar amount, because a payroll that is off by one
//! cent against the IRS is a payroll that is wrong.
//!
//! Tax rates are integers in parts-per-million. 6.2% is 62_000 ppm; New York's
//! Paid Family Leave rate of 0.388% is 3_880 ppm. Six decimal places is enough
//! for every published federal and state rate, and it keeps the multiply exact
//! up to the final rounding step.

use std::fmt;
use std::iter::Sum;
use std::ops::{Add, AddAssign, Neg, Sub, SubAssign};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// One whole, 100%, in parts-per-million: the largest rate a schedule or election may carry.
pub const WHOLE_PPM: i64 = 1_000_000;
/// Denominator for parts-per-million rates.
pub const PPM: i128 = WHOLE_PPM as i128;
/// Thousands separators group the whole dollars three digits at a time.
const DIGITS_PER_GROUP: usize = 3;

/// A signed amount of money, stored as whole cents.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
pub struct Cents(pub i64);

impl Cents {
    pub const ZERO: Cents = Cents(0);

    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    pub const fn is_positive(self) -> bool {
        self.0 > 0
    }

    /// Clamp negatives to zero. Worksheet 1A says "if zero or less, enter -0-"
    /// in five separate places; this is that instruction.
    pub const fn floor_zero(self) -> Cents {
        if self.0 < 0 {
            Cents::ZERO
        } else {
            self
        }
    }

    pub const fn min(self, other: Cents) -> Cents {
        if self.0 <= other.0 {
            self
        } else {
            other
        }
    }

    /// Multiply by a parts-per-million rate, rounding half away from zero.
    pub fn mul_ppm(self, ppm: i64) -> Cents {
        let num = self.0 as i128 * ppm as i128;
        Cents(div_round_half_up(num, PPM) as i64)
    }

    /// Divide into `n` equal parts, rounding half away from zero.
    ///
    /// Used for per-period amounts on Worksheet 1A lines 2h and 3b, where the
    /// annual figure is divided by the number of pay periods.
    pub fn div_round(self, n: i64) -> Cents {
        assert!(n != 0, "division by zero pay periods");
        Cents(div_round_half_up(self.0 as i128, n as i128) as i64)
    }

    /// Round to a whole dollar, half away from zero.
    ///
    /// Publication 15-T: "Withheld tax amounts should be rounded to the nearest
    /// whole dollar by dropping amounts under 50 cents and increasing amounts
    /// from 50 to 99 cents to the next dollar."
    pub fn round_to_dollar(self) -> Cents {
        Cents(div_round_half_up(self.0 as i128, 100) as i64 * 100)
    }

    /// Bare decimal, no currency symbol and no thousands separators, for
    /// machine-readable output and form line values.
    pub fn plain(self) -> String {
        let neg = self.0 < 0;
        let abs = self.0.unsigned_abs();
        format!(
            "{}{}.{:02}",
            if neg { "-" } else { "" },
            abs / 100,
            abs % 100
        )
    }
}

/// Round `num / den` half away from zero. `den` must be positive.
fn div_round_half_up(num: i128, den: i128) -> i128 {
    debug_assert!(den > 0);
    if num >= 0 {
        (num + den / 2) / den
    } else {
        -((-num + den / 2) / den)
    }
}

impl Add for Cents {
    type Output = Cents;
    fn add(self, rhs: Cents) -> Cents {
        Cents(self.0 + rhs.0)
    }
}

impl Sub for Cents {
    type Output = Cents;
    fn sub(self, rhs: Cents) -> Cents {
        Cents(self.0 - rhs.0)
    }
}

impl Neg for Cents {
    type Output = Cents;
    fn neg(self) -> Cents {
        Cents(-self.0)
    }
}

impl AddAssign for Cents {
    fn add_assign(&mut self, rhs: Cents) {
        self.0 += rhs.0;
    }
}

impl SubAssign for Cents {
    fn sub_assign(&mut self, rhs: Cents) {
        self.0 -= rhs.0;
    }
}

impl Sum for Cents {
    fn sum<I: Iterator<Item = Cents>>(iter: I) -> Cents {
        Cents(iter.map(|c| c.0).sum())
    }
}

impl fmt::Display for Cents {
    /// Human presentation with a currency symbol and thousands separators.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let neg = self.0 < 0;
        let abs = self.0.unsigned_abs();
        let (whole, frac) = (abs / 100, abs % 100);
        let digits = whole.to_string();
        let mut grouped = String::with_capacity(digits.len() + digits.len() / DIGITS_PER_GROUP);
        for (i, ch) in digits.chars().enumerate() {
            if i > 0 && (digits.len() - i) % DIGITS_PER_GROUP == 0 {
                grouped.push(',');
            }
            grouped.push(ch);
        }
        write!(f, "{}${}.{:02}", if neg { "-" } else { "" }, grouped, frac)
    }
}

impl fmt::Debug for Cents {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}

/// Parse a decimal dollar amount: `1234`, `1234.5`, `1234.56`, `-12.34`.
///
/// Rejects more than two decimal places rather than silently truncating, so a
/// typo in a config file surfaces as an error instead of a wrong paycheck.
impl std::str::FromStr for Cents {
    type Err = String;

    fn from_str(s: &str) -> Result<Cents, String> {
        let s = s.trim().replace([',', '$', '_'], "");
        let (neg, digits) = match s.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, s.strip_prefix('+').unwrap_or(&s)),
        };
        if digits.is_empty() {
            return Err("empty amount".into());
        }
        let (whole, frac) = match digits.split_once('.') {
            Some((w, f)) => (w, f),
            None => (digits, ""),
        };
        if frac.len() > 2 {
            return Err(format!("{s}: more than two decimal places"));
        }
        if !whole.chars().all(|c| c.is_ascii_digit()) || !frac.chars().all(|c| c.is_ascii_digit()) {
            return Err(format!("{s}: not a decimal amount"));
        }
        let whole: i64 = if whole.is_empty() {
            0
        } else {
            whole
                .parse()
                .map_err(|_| format!("{s}: amount too large"))?
        };
        let frac: i64 = match frac.len() {
            0 => 0,
            1 => frac.parse::<i64>().unwrap() * 10,
            _ => frac.parse::<i64>().unwrap(),
        };
        let total = whole
            .checked_mul(100)
            .and_then(|w| w.checked_add(frac))
            .ok_or_else(|| format!("{s}: amount too large"))?;
        Ok(Cents(if neg { -total } else { total }))
    }
}

/// Serialized as a decimal string so config files and the ledger stay readable
/// and diffable, and so no JSON/TOML float ever rounds a dollar amount.
impl Serialize for Cents {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.plain())
    }
}

impl<'de> Deserialize<'de> for Cents {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Cents, D::Error> {
        use serde::de::Error;
        String::deserialize(d)?.parse().map_err(D::Error::custom)
    }
}




