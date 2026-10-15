//! Runtime values of the formula language.
//!
//! Numbers are exact rationals so that "half your level" stays ½ until the
//! final 5e round-down. Dice are values in their own right (`2d6+3`), never
//! rolled.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;

/// An exact fraction, always normalised (denominator > 0, lowest terms).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Num {
    n: i64,
    d: i64,
}

fn gcd(a: i64, b: i64) -> i64 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a.max(1)
}

impl Num {
    pub const ZERO: Num = Num { n: 0, d: 1 };

    pub fn to_f64(self) -> f64 {
        self.n as f64 / self.d as f64
    }

    pub fn int(n: i64) -> Num {
        Num { n, d: 1 }
    }

    /// `n / d`; `None` when `d` is zero.
    pub fn ratio(n: i64, d: i64) -> Option<Num> {
        if d == 0 {
            return None;
        }
        let g = gcd(n, d) * d.signum();
        Some(Num { n: n / g, d: d / g })
    }

    pub fn is_int(self) -> bool {
        self.d == 1
    }

    pub fn floor(self) -> i64 {
        self.n.div_euclid(self.d)
    }

    pub fn ceil(self) -> i64 {
        -(-self.n).div_euclid(self.d)
    }

    pub fn checked_div(self, o: Num) -> Option<Num> {
        Num::ratio(self.n * o.d, self.d * o.n)
    }
}

impl std::ops::Add for Num {
    type Output = Num;
    fn add(self, o: Num) -> Num {
        Num::ratio(self.n * o.d + o.n * self.d, self.d * o.d).unwrap()
    }
}

impl std::ops::Sub for Num {
    type Output = Num;
    fn sub(self, o: Num) -> Num {
        self + -o
    }
}

impl std::ops::Mul for Num {
    type Output = Num;
    fn mul(self, o: Num) -> Num {
        Num::ratio(self.n * o.n, self.d * o.d).unwrap()
    }
}

impl std::ops::Neg for Num {
    type Output = Num;
    fn neg(self) -> Num {
        Num { n: -self.n, d: self.d }
    }
}

impl PartialOrd for Num {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Num {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.n as i128 * other.d as i128).cmp(&(other.n as i128 * self.d as i128))
    }
}

impl fmt::Display for Num {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.d == 1 {
            write!(f, "{}", self.n)
        } else {
            write!(f, "{}/{}", self.n, self.d)
        }
    }
}

/// A dice expression such as `2d6 + 1d4 + 3`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Dice {
    /// Die faces → number of dice.
    pub terms: BTreeMap<u32, i64>,
    pub flat: i64,
}

impl Dice {
    pub fn new(count: i64, faces: u32) -> Dice {
        let mut terms = BTreeMap::new();
        if count != 0 {
            terms.insert(faces, count);
        }
        Dice { terms, flat: 0 }
    }

    pub fn plus(&self, other: &Dice) -> Dice {
        let mut out = self.clone();
        for (&faces, &count) in &other.terms {
            *out.terms.entry(faces).or_default() += count;
        }
        out.terms.retain(|_, c| *c != 0);
        out.flat += other.flat;
        out
    }

    pub fn plus_flat(&self, n: i64) -> Dice {
        Dice {
            terms: self.terms.clone(),
            flat: self.flat + n,
        }
    }

    pub fn times(&self, k: i64) -> Dice {
        Dice {
            terms: self.terms.iter().map(|(&f, &c)| (f, c * k)).filter(|(_, c)| *c != 0).collect(),
            flat: self.flat * k,
        }
    }

    /// Average result, rounded down (the 5e "fixed damage" value).
    pub fn average(&self) -> i64 {
        let twice: i64 = self.terms.iter().map(|(&f, &c)| c * (f as i64 + 1)).sum();
        (twice + 2 * self.flat).div_euclid(2)
    }

    /// The exact mean, for comparing dice.
    pub fn mean(&self) -> f64 {
        let twice: i64 = self.terms.iter().map(|(&f, &c)| c * (f as i64 + 1)).sum();
        (twice + 2 * self.flat) as f64 / 2.0
    }
}

impl fmt::Display for Dice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Larger dice first, as written in the books.
        let mut first = true;
        for (faces, count) in self.terms.iter().rev() {
            if !first {
                f.write_str(if *count < 0 { " - " } else { " + " })?;
            } else if *count < 0 {
                f.write_str("-")?;
            }
            write!(f, "{}d{faces}", count.abs())?;
            first = false;
        }
        if self.flat != 0 || first {
            if first {
                write!(f, "{}", self.flat)?;
            } else {
                write!(f, " {} {}", if self.flat < 0 { '-' } else { '+' }, self.flat.abs())?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Num(Num),
    Dice(Dice),
    Bool(bool),
    Str(String),
    /// Only as a literal argument, e.g. to `scale`.
    Map(Vec<(Value, Value)>),
}

impl Value {
    pub fn int(n: i64) -> Value {
        Value::Num(Num::int(n))
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Num(_) => "number",
            Value::Dice(_) => "dice",
            Value::Bool(_) => "boolean",
            Value::Str(_) => "string",
            Value::Map(_) => "map",
        }
    }

    pub fn as_num(&self) -> Option<Num> {
        match self {
            Value::Num(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// The value as finally shown on a sheet: numbers round down (5e rule).
    pub fn rounded(self) -> Value {
        match self {
            Value::Num(n) if !n.is_int() => Value::int(n.floor()),
            Value::Dice(d) if d.terms.is_empty() => Value::int(d.flat),
            other => other,
        }
    }
}

impl From<i64> for Value {
    fn from(n: i64) -> Value {
        Value::int(n)
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Value {
        Value::Bool(b)
    }
}

impl From<Dice> for Value {
    fn from(d: Dice) -> Value {
        Value::Dice(d)
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Num(n) => write!(f, "{n}"),
            Value::Dice(d) => write!(f, "{d}"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Str(s) => write!(f, "{s:?}"),
            Value::Map(entries) => {
                f.write_str("{")?;
                for (i, (k, v)) in entries.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{k}: {v}")?;
                }
                f.write_str("}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractions() {
        let half = Num::ratio(1, 2).unwrap();
        assert_eq!(half + half, Num::int(1));
        assert_eq!(Num::ratio(7, 2).unwrap().floor(), 3);
        assert_eq!(Num::ratio(7, 2).unwrap().ceil(), 4);
        assert_eq!(Num::ratio(-1, 2).unwrap().floor(), -1);
        assert_eq!(Num::ratio(2, -4).unwrap(), Num::ratio(-1, 2).unwrap());
        assert!(Num::ratio(1, 0).is_none());
        assert!(Num::ratio(1, 3).unwrap() < Num::ratio(1, 2).unwrap());
    }

    #[test]
    fn dice_arithmetic_and_display() {
        let d = Dice::new(2, 6).plus(&Dice::new(1, 8)).plus_flat(3);
        assert_eq!(d.to_string(), "1d8 + 2d6 + 3");
        assert_eq!(Dice::new(1, 6).times(3).to_string(), "3d6");
        assert_eq!(Dice::new(1, 4).plus_flat(-1).to_string(), "1d4 - 1");
        assert_eq!(Dice::new(2, 6).average(), 7);
        assert_eq!(Dice::new(1, 8).plus_flat(2).average(), 6);
    }

    #[test]
    fn rounding_for_display() {
        assert_eq!(Value::Num(Num::ratio(5, 2).unwrap()).rounded(), Value::int(2));
        assert_eq!(Value::Num(Num::ratio(-1, 2).unwrap()).rounded(), Value::int(-1));
    }
}
