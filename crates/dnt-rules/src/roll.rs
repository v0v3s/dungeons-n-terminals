//! Rolling dice: d20 tests with advantage and disadvantage, and damage
//! with critical hits and Great Weapon Fighting. Results keep every die
//! so the breakdown can be shown.

use crate::value::{Dice, Value};

/// A small, fast pseudo-random generator (SplitMix64). Seeded from the
/// clock in play; fixed seeds make tests repeatable.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    pub fn from_time() -> Rng {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0);
        Rng(nanos ^ 0x9E37_79B9_7F4A_7C15)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// 1..=faces, uniformly.
    pub fn die(&mut self, faces: u32) -> i64 {
        let faces = faces.max(1) as u64;
        // Rejection sampling avoids modulo bias.
        let zone = u64::MAX - u64::MAX % faces;
        loop {
            let x = self.next();
            if x < zone {
                return (x % faces) as i64 + 1;
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Advantage {
    #[default]
    Normal,
    Advantage,
    Disadvantage,
}

impl Advantage {
    /// Advantage and disadvantage cancel out, however many of each.
    pub fn combine(sources: &[Advantage]) -> Advantage {
        let adv = sources.contains(&Advantage::Advantage);
        let dis = sources.contains(&Advantage::Disadvantage);
        match (adv, dis) {
            (true, false) => Advantage::Advantage,
            (false, true) => Advantage::Disadvantage,
            _ => Advantage::Normal,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Advantage::Normal => "",
            Advantage::Advantage => "advantage",
            Advantage::Disadvantage => "disadvantage",
        }
    }
}

/// A d20 roll plus a modifier.
#[derive(Debug, Clone, PartialEq)]
pub struct D20 {
    /// Both dice with advantage or disadvantage, else one.
    pub dice: Vec<i64>,
    pub kept: i64,
    pub modifier: i64,
    pub total: i64,
    pub mode: Advantage,
}

impl D20 {
    pub fn natural_20(&self) -> bool {
        self.kept == 20
    }

    pub fn natural_1(&self) -> bool {
        self.kept == 1
    }

    /// `14 (3) + 7 = 21`: the kept die, the other one in brackets.
    pub fn describe(&self) -> String {
        let dice = match self.dice.as_slice() {
            [a, b] => {
                let other = if *a == self.kept { b } else { a };
                format!("{} ({other})", self.kept)
            }
            _ => self.kept.to_string(),
        };
        let sign = if self.modifier < 0 { '-' } else { '+' };
        let mut s = format!("{dice} {sign} {} = {}", self.modifier.abs(), self.total);
        if self.natural_20() {
            s.push_str(" · natural 20");
        } else if self.natural_1() {
            s.push_str(" · natural 1");
        }
        s
    }
}

pub fn d20(rng: &mut Rng, modifier: i64, mode: Advantage) -> D20 {
    let first = rng.die(20);
    let (dice, kept) = match mode {
        Advantage::Normal => (vec![first], first),
        Advantage::Advantage | Advantage::Disadvantage => {
            let second = rng.die(20);
            let kept = if mode == Advantage::Advantage { first.max(second) } else { first.min(second) };
            (vec![first, second], kept)
        }
    };
    D20 { dice, kept, modifier, total: kept + modifier, mode }
}

/// How damage dice are treated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DamageRules {
    /// Roll every die twice (a critical hit).
    pub critical: bool,
    /// Great Weapon Fighting, 2024: 1s and 2s count as 3s.
    pub min_three: bool,
    /// Great Weapon Fighting, 2014: reroll 1s and 2s once.
    pub reroll_low: bool,
}

/// One rolled die.
#[derive(Debug, Clone, PartialEq)]
pub struct Die {
    pub faces: u32,
    pub value: i64,
    /// The roll before Great Weapon Fighting changed it.
    pub was: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Damage {
    pub dice: Vec<Die>,
    pub flat: i64,
    pub total: i64,
}

impl Damage {
    /// `[6, 3→3] + 4 = 13`
    pub fn describe(&self) -> String {
        if self.dice.is_empty() {
            return self.total.to_string();
        }
        let dice: Vec<String> = self
            .dice
            .iter()
            .map(|d| match d.was {
                Some(w) => format!("{w}→{}", d.value),
                None => d.value.to_string(),
            })
            .collect();
        let mut s = format!("[{}]", dice.join(", "));
        if self.flat != 0 {
            let sign = if self.flat < 0 { '-' } else { '+' };
            s.push_str(&format!(" {sign} {}", self.flat.abs()));
        }
        s.push_str(&format!(" = {}", self.total));
        s
    }
}

/// Roll a dice value (`2d6 + 3`; a plain number rolls nothing).
pub fn damage(rng: &mut Rng, value: &Value, rules: DamageRules) -> Damage {
    let (terms, flat): (Vec<(u32, i64)>, i64) = match value {
        Value::Dice(d) => (d.terms.iter().map(|(&f, &c)| (f, c)).collect(), d.flat),
        Value::Num(n) => (Vec::new(), n.floor()),
        _ => (Vec::new(), 0),
    };
    let mut dice = Vec::new();
    // Larger dice first, as written.
    for (faces, count) in terms.into_iter().rev() {
        let count = if rules.critical { count * 2 } else { count };
        for _ in 0..count.max(0) {
            let mut value = rng.die(faces);
            let mut was = None;
            if rules.reroll_low && value <= 2 {
                was = Some(value);
                value = rng.die(faces);
            }
            if rules.min_three && value <= 2 {
                was = Some(value);
                value = 3;
            }
            dice.push(Die { faces, value, was });
        }
    }
    let total = (dice.iter().map(|d| d.value).sum::<i64>() + flat).max(0);
    Damage { dice, flat, total }
}

/// Parse a dice expression like `2d6 + 3` (as in data tags).
pub fn parse_dice(text: &str) -> Option<Dice> {
    match crate::formula::Formula::parse(text).ok()?.eval(&std::collections::HashMap::<String, Value>::new()).ok()? {
        Value::Dice(d) => Some(d),
        Value::Num(n) => {
            Some(Dice { flat: n.floor(), ..Default::default() })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dice_are_in_range_and_spread() {
        let mut rng = Rng::new(7);
        let mut seen = [0; 7];
        for _ in 0..6000 {
            let v = rng.die(6);
            assert!((1..=6).contains(&v));
            seen[v as usize] += 1;
        }
        assert!(seen[1..].iter().all(|&n| n > 800), "{seen:?}");
    }

    #[test]
    fn advantage_keeps_the_higher() {
        let mut rng = Rng::new(1);
        for _ in 0..200 {
            let r = d20(&mut rng, 5, Advantage::Advantage);
            assert_eq!(r.kept, *r.dice.iter().max().unwrap());
            assert_eq!(r.total, r.kept + 5);
            let r = d20(&mut rng, 0, Advantage::Disadvantage);
            assert_eq!(r.kept, *r.dice.iter().min().unwrap());
        }
        assert_eq!(Advantage::combine(&[Advantage::Advantage, Advantage::Disadvantage, Advantage::Advantage]), Advantage::Normal);
        assert_eq!(Advantage::combine(&[Advantage::Normal, Advantage::Disadvantage]), Advantage::Disadvantage);
    }

    #[test]
    fn damage_rules() {
        let mut rng = Rng::new(3);
        let v = Value::Dice(parse_dice("2d6 + 3").unwrap());
        let r = damage(&mut rng, &v, DamageRules::default());
        assert_eq!(r.dice.len(), 2);
        assert_eq!(r.total, r.dice.iter().map(|d| d.value).sum::<i64>() + 3);
        let crit = damage(&mut rng, &v, DamageRules { critical: true, ..Default::default() });
        assert_eq!(crit.dice.len(), 4, "a critical hit doubles the dice, not the modifier");
        assert_eq!(crit.flat, 3);
        for _ in 0..100 {
            let gwf = damage(&mut rng, &v, DamageRules { min_three: true, ..Default::default() });
            assert!(gwf.dice.iter().all(|d| d.value >= 3));
        }
        assert_eq!(damage(&mut rng, &Value::int(4), DamageRules::default()).total, 4);
        assert!(r.describe().ends_with(&format!("+ 3 = {}", r.total)));
    }
}
