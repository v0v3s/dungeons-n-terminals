//! Stat evaluation: stats are nodes with a base formula; effects modify them.
//! Everything is evaluated in dependency order, cycles are reported instead
//! of crashing, and every result keeps a breakdown of how it was reached.
//!
//! Per stat, in order:
//! 1. `base`, and every active `max` effect, are candidates; the highest wins
//!    (e.g. alternative AC calculations).
//! 2. The last active `set` effect replaces that.
//! 3. Active `add` effects are summed on top.
//! 4. Active `min` effects cap the result.
//! 5. Numbers round down; a manual override, if any, replaces the result.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::formula::{Env, Formula};
use crate::value::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Add,
    Set,
    Max,
    Min,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Effect {
    pub target: String,
    pub op: Op,
    pub value: Formula,
    pub when: Option<Formula>,
    /// Where the effect comes from, e.g. "Defense (Fighting Style)".
    pub origin: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StatDef {
    pub key: String,
    pub base: Formula,
    /// Label for the base part of the breakdown, e.g. "base" or "Dexterity".
    pub origin: String,
}

impl StatDef {
    /// Convenience constructor; panics on an invalid formula (for built-in definitions).
    pub fn new(key: &str, base: &str, origin: &str) -> StatDef {
        StatDef {
            key: key.to_string(),
            base: Formula::parse(base).unwrap_or_else(|e| panic!("built-in stat {key}: {}", e.display(base))),
            origin: origin.to_string(),
        }
    }
}

/// Everything needed to compute a character's stats.
#[derive(Debug, Clone, Default)]
pub struct Model {
    pub stats: Vec<StatDef>,
    pub effects: Vec<Effect>,
    /// Plain inputs: ability base scores, levels, toggles, equipment flags, …
    pub inputs: HashMap<String, Value>,
    /// Levels per class (lowercase name), for `level('Wizard')`.
    pub class_levels: HashMap<String, i64>,
    /// Manual overrides, applied last.
    pub overrides: HashMap<String, Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartKind {
    Base,
    Add,
    Set,
    Max,
    Min,
    Override,
}

/// One line of a stat's breakdown.
#[derive(Debug, Clone, PartialEq)]
pub struct Part {
    pub kind: PartKind,
    pub origin: String,
    /// `None` when the part could not be evaluated or is inactive.
    pub value: Option<Value>,
    /// Whether this part contributed to the final value.
    pub applied: bool,
    /// Why it did not apply, or an evaluation error.
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StatResult {
    pub value: Result<Value, String>,
    pub parts: Vec<Part>,
}

impl StatResult {
    /// A one-line explanation, e.g. `17 = 10 base + 3 Dexterity + 2 Shield`.
    pub fn explain(&self) -> String {
        let value = match &self.value {
            Ok(v) => v.to_string(),
            Err(e) => return format!("error: {e}"),
        };
        let applied: Vec<&Part> = self.parts.iter().filter(|p| p.applied).collect();
        if let [only] = applied.as_slice() {
            return format!("{value} ({})", only.origin);
        }
        let mut out = format!("{value} =");
        let mut first = true;
        for p in applied {
            let v = p.value.as_ref().map(Value::to_string).unwrap_or_default();
            let piece = match p.kind {
                PartKind::Add if !first => match v.strip_prefix('-') {
                    Some(abs) => format!(" - {abs} {}", p.origin),
                    None => format!(" + {v} {}", p.origin),
                },
                PartKind::Min => format!(" capped at {v} ({})", p.origin),
                PartKind::Override => format!(" overridden to {v}"),
                _ => format!(" {v} {}", p.origin),
            };
            out.push_str(&piece);
            first = false;
        }
        out
    }
}

#[derive(Debug, Clone, Default)]
pub struct Evaluation {
    pub stats: BTreeMap<String, StatResult>,
}

impl Evaluation {
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.stats.get(key).and_then(|r| r.value.as_ref().ok())
    }

    /// The stat as a whole number, if it is one.
    pub fn int(&self, key: &str) -> Option<i64> {
        match self.get(key)? {
            Value::Num(n) if n.is_int() => Some(n.floor()),
            _ => None,
        }
    }

    pub fn errors(&self) -> impl Iterator<Item = (&str, &str)> {
        self.stats.iter().filter_map(|(k, r)| r.value.as_ref().err().map(|e| (k.as_str(), e.as_str())))
    }
}

struct EvalEnv<'a> {
    model: &'a Model,
    done: &'a BTreeMap<String, StatResult>,
}

impl Env for EvalEnv<'_> {
    fn var(&self, name: &str) -> Option<Value> {
        if let Some(r) = self.done.get(name) {
            return r.value.clone().ok();
        }
        self.model.inputs.get(name).cloned()
    }

    fn call(&self, name: &str, args: &[Value]) -> Option<Result<Value, String>> {
        match name {
            "level" => Some(match args {
                [] => Ok(self.var("level").unwrap_or(Value::int(0))),
                [Value::Str(class)] => Ok(Value::int(
                    self.model.class_levels.get(&class.to_lowercase()).copied().unwrap_or(0),
                )),
                _ => Err("level() takes an optional class name".into()),
            }),
            _ => None,
        }
    }
}

impl Model {
    pub fn evaluate(&self) -> Evaluation {
        // Every stat key: defined stats plus implicit ones targeted by effects.
        let mut defs: BTreeMap<&str, Option<&StatDef>> = BTreeMap::new();
        for e in &self.effects {
            defs.entry(&e.target).or_insert(None);
        }
        for s in &self.stats {
            defs.insert(&s.key, Some(s));
        }
        let mut effects_by_target: HashMap<&str, Vec<&Effect>> = HashMap::new();
        for e in &self.effects {
            effects_by_target.entry(&e.target).or_default().push(e);
        }

        // Dependencies between stats (inputs are leaves).
        let mut deps: BTreeMap<&str, HashSet<String>> = BTreeMap::new();
        for (&key, def) in &defs {
            let mut d: HashSet<String> = HashSet::new();
            if let Some(def) = def {
                d.extend(def.base.vars());
            }
            for e in effects_by_target.get(key).into_iter().flatten() {
                d.extend(e.value.vars());
                if let Some(w) = &e.when {
                    d.extend(w.vars());
                }
            }
            d.retain(|v| defs.contains_key(v.as_str()));
            deps.insert(key, d);
        }

        // Kahn's algorithm.
        let mut order = Vec::with_capacity(defs.len());
        let mut remaining: BTreeMap<&str, usize> = deps.iter().map(|(k, d)| (*k, d.len())).collect();
        let mut dependents: HashMap<&str, Vec<&str>> = HashMap::new();
        for (k, d) in &deps {
            for dep in d {
                dependents.entry(defs.get_key_value(dep.as_str()).unwrap().0).or_default().push(k);
            }
        }
        let mut ready: Vec<&str> = remaining.iter().filter(|(_, n)| **n == 0).map(|(k, _)| *k).collect();
        while let Some(k) = ready.pop() {
            remaining.remove(k);
            order.push(k);
            for &dep in dependents.get(k).into_iter().flatten() {
                let n = remaining.get_mut(dep).unwrap();
                *n -= 1;
                if *n == 0 {
                    ready.push(dep);
                }
            }
        }

        let mut done: BTreeMap<String, StatResult> = BTreeMap::new();
        for key in order {
            let result = self.evaluate_stat(key, defs[key], effects_by_target.get(key).map(Vec::as_slice), &done);
            done.insert(key.to_string(), result);
        }
        // Whatever is left is in, or depends on, a cycle.
        let stuck: Vec<&str> = remaining.keys().copied().collect();
        for key in &stuck {
            let involved: Vec<&str> = deps[key].iter().map(String::as_str).filter(|d| remaining.contains_key(d)).collect();
            done.insert(
                key.to_string(),
                StatResult {
                    value: Err(format!("circular dependency via {}", involved.join(", "))),
                    parts: Vec::new(),
                },
            );
        }
        Evaluation { stats: done }
    }

    fn evaluate_stat(
        &self,
        key: &str,
        def: Option<&StatDef>,
        effects: Option<&[&Effect]>,
        done: &BTreeMap<String, StatResult>,
    ) -> StatResult {
        let env = EvalEnv { model: self, done };
        let mut parts: Vec<Part> = Vec::new();

        // 1. Base and max candidates.
        let base = match def {
            Some(def) => match def.base.eval(&env) {
                Ok(v) => Some((def.origin.clone(), v)),
                Err(e) => {
                    return StatResult {
                        value: Err(format!("{} (in `{}`)", e.message, def.base.source)),
                        parts,
                    }
                }
            },
            // Implicit stats start from an input of the same name, or zero.
            None => Some(("base".to_string(), self.inputs.get(key).cloned().unwrap_or(Value::int(0)))),
        };
        if let Some((origin, v)) = base {
            parts.push(Part {
                kind: PartKind::Base,
                origin,
                value: Some(v),
                applied: true,
                note: None,
            });
        }

        let effects = effects.unwrap_or_default();
        let mut active: Vec<(&Effect, Value)> = Vec::new();
        for e in effects {
            let kind = match e.op {
                Op::Add => PartKind::Add,
                Op::Set => PartKind::Set,
                Op::Max => PartKind::Max,
                Op::Min => PartKind::Min,
            };
            let on = match &e.when {
                None => Ok(true),
                Some(w) => w.eval(&env).and_then(|v| {
                    v.as_bool().ok_or_else(|| crate::formula::Error {
                        message: format!("condition must be true/false, found {}", v.type_name()),
                        span: w.expr.span.clone(),
                    })
                }),
            };
            match on.and_then(|on| if on { e.value.eval(&env).map(Some) } else { Ok(None) }) {
                Ok(Some(v)) => active.push((e, v)),
                Ok(None) => parts.push(Part {
                    kind,
                    origin: e.origin.clone(),
                    value: None,
                    applied: false,
                    note: Some("inactive".into()),
                }),
                Err(err) => parts.push(Part {
                    kind,
                    origin: e.origin.clone(),
                    value: None,
                    applied: false,
                    note: Some(err.message),
                }),
            }
        }

        let push = |parts: &mut Vec<Part>, e: &Effect, kind: PartKind, v: &Value| {
            parts.push(Part {
                kind,
                origin: e.origin.clone(),
                value: Some(v.clone()),
                applied: true,
                note: None,
            });
            parts.len() - 1
        };

        let mut current = parts[0].value.clone().unwrap();
        let mut current_idx = 0;
        for (e, v) in active.iter().filter(|(e, _)| e.op == Op::Max) {
            let idx = push(&mut parts, e, PartKind::Max, v);
            // Dice compare by their average (1d4 + 2 beats 3).
            let size = |x: &Value| match x {
                Value::Dice(d) => Some(d.mean()),
                other => other.as_num().map(|n| n.to_f64()),
            };
            match (size(&current), size(v)) {
                (Some(cur), Some(new)) if new > cur => {
                    parts[current_idx].applied = false;
                    parts[current_idx].note = Some(format!("lower than {}", e.origin));
                    current = v.clone();
                    current_idx = idx;
                }
                (Some(_), Some(_)) => {
                    parts[idx].applied = false;
                    parts[idx].note = Some("not higher".into());
                }
                _ => {
                    parts[idx].applied = false;
                    parts[idx].note = Some("max needs numbers".into());
                }
            }
        }

        // 2. Set: the last one wins.
        for (e, v) in active.iter().filter(|(e, _)| e.op == Op::Set) {
            let idx = push(&mut parts, e, PartKind::Set, v);
            parts[current_idx].applied = false;
            parts[current_idx].note.get_or_insert_with(|| format!("replaced by {}", e.origin));
            current = v.clone();
            current_idx = idx;
        }

        // 3. Adds.
        for (e, v) in active.iter().filter(|(e, _)| e.op == Op::Add) {
            let idx = push(&mut parts, e, PartKind::Add, v);
            match add(&current, v) {
                Some(sum) => current = sum,
                None => {
                    parts[idx].applied = false;
                    parts[idx].note = Some(format!("cannot add {} to {}", v.type_name(), current.type_name()));
                }
            }
        }

        // 4. Caps.
        for (e, v) in active.iter().filter(|(e, _)| e.op == Op::Min) {
            let idx = push(&mut parts, e, PartKind::Min, v);
            match (current.as_num(), v.as_num()) {
                (Some(cur), Some(cap)) if cap < cur => current = v.clone(),
                _ => {
                    parts[idx].applied = false;
                    parts[idx].note = Some("not lower".into());
                }
            }
        }

        // 5. Round, then override.
        let mut value = current.rounded();
        if let Some(o) = self.overrides.get(key) {
            parts.push(Part {
                kind: PartKind::Override,
                origin: "manual override".into(),
                value: Some(o.clone()),
                applied: true,
                note: Some(format!("computed {value}")),
            });
            value = o.clone();
        }
        StatResult { value: Ok(value), parts }
    }
}

fn add(a: &Value, b: &Value) -> Option<Value> {
    Some(match (a, b) {
        (Value::Num(x), Value::Num(y)) => Value::Num(*x + *y),
        (Value::Dice(d), Value::Num(n)) | (Value::Num(n), Value::Dice(d)) if n.is_int() => {
            Value::Dice(d.plus_flat(n.floor()))
        }
        (Value::Dice(x), Value::Dice(y)) => Value::Dice(x.plus(y)),
        (Value::Bool(x), Value::Bool(y)) => Value::Bool(*x || *y),
        _ => return None,
    })
}

/// Convenience for building effects in code and tests.
pub fn effect(target: &str, op: Op, value: &str, when: Option<&str>, origin: &str) -> Effect {
    Effect {
        target: target.to_string(),
        op,
        value: Formula::parse(value).expect("valid effect formula"),
        when: when.map(|w| Formula::parse(w).expect("valid condition")),
        origin: origin.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Dice;

    fn model() -> Model {
        let mut m = Model::default();
        m.inputs.insert("base.dex".into(), Value::int(14));
        m.inputs.insert("base.con".into(), Value::int(16));
        m.inputs.insert("armor.none".into(), Value::Bool(true));
        m.stats = vec![
            StatDef::new("dex", "base.dex", "base"),
            StatDef::new("con", "base.con", "base"),
            StatDef::new("ac", "10 + mod(dex)", "unarmored"),
        ];
        m
    }

    #[test]
    fn base_and_adds_with_breakdown() {
        let mut m = model();
        m.effects.push(effect("dex", Op::Add, "2", None, "Ability Score Improvement"));
        m.effects.push(effect("ac", Op::Add, "2", None, "Shield"));
        let ev = m.evaluate();
        assert_eq!(ev.int("dex"), Some(16));
        assert_eq!(ev.int("ac"), Some(15));
        assert_eq!(ev.stats["ac"].explain(), "15 = 13 unarmored + 2 Shield");
    }

    #[test]
    fn max_picks_best_alternative() {
        let mut m = model();
        m.effects.push(effect("ac", Op::Max, "10 + mod(dex) + mod(con)", Some("armor.none"), "Unarmored Defense"));
        m.effects.push(effect("ac", Op::Max, "13 + mod(dex)", Some("toggle('mage_armor')"), "Mage Armor"));
        let ev = m.evaluate();
        assert_eq!(ev.int("ac"), Some(15));
        let parts = &ev.stats["ac"].parts;
        assert!(!parts[0].applied, "base should lose to Unarmored Defense");
        assert!(parts.iter().any(|p| p.origin == "Mage Armor" && p.note.as_deref() == Some("inactive")));
    }

    #[test]
    fn set_and_min_and_override() {
        let mut m = model();
        m.effects.push(effect("dex", Op::Set, "19", None, "Gloves"));
        m.effects.push(effect("dex", Op::Min, "18", None, "Cap"));
        m.effects.push(effect("con", Op::Add, "1", None, "Feat"));
        m.overrides.insert("con".into(), Value::int(20));
        let ev = m.evaluate();
        assert_eq!(ev.int("dex"), Some(18));
        assert_eq!(ev.int("con"), Some(20));
        assert_eq!(ev.stats["con"].explain(), "20 = 16 base + 1 Feat overridden to 20");
    }

    #[test]
    fn implicit_targets_and_dice() {
        let mut m = model();
        m.inputs.insert("class.blood_hunter.hemocraft_die".into(), Value::Dice(Dice::new(1, 6)));
        m.inputs.insert("toggle.crimson_rite".into(), Value::Bool(true));
        m.effects.push(effect(
            "attack.weapon.damage_bonus",
            Op::Add,
            "class.blood_hunter.hemocraft_die",
            Some("toggle('crimson_rite')"),
            "Crimson Rite",
        ));
        let ev = m.evaluate();
        assert_eq!(ev.get("attack.weapon.damage_bonus").unwrap().to_string(), "1d6");
    }

    #[test]
    fn halves_round_down_at_the_end() {
        let mut m = model();
        m.inputs.insert("level".into(), Value::int(5));
        m.stats.push(StatDef::new("half", "level / 2", "half level"));
        m.stats.push(StatDef::new("twice_half", "half * 2", "double"));
        let ev = m.evaluate();
        assert_eq!(ev.int("half"), Some(2));
        // Downstream stats see the rounded value.
        assert_eq!(ev.int("twice_half"), Some(4));
    }

    #[test]
    fn cycles_are_reported() {
        let mut m = model();
        m.stats.push(StatDef::new("a", "b + 1", "base"));
        m.stats.push(StatDef::new("b", "a + 1", "base"));
        m.stats.push(StatDef::new("c", "b", "base"));
        let ev = m.evaluate();
        assert!(ev.stats["a"].value.as_ref().unwrap_err().contains("circular"));
        assert!(ev.stats["c"].value.is_err());
        assert_eq!(ev.int("ac"), Some(12), "unrelated stats still evaluate");
    }

    #[test]
    fn errors_are_contained() {
        let mut m = model();
        m.effects.push(effect("ac", Op::Add, "nope", None, "Broken Item"));
        m.stats.push(StatDef::new("bad", "missing + 1", "base"));
        let ev = m.evaluate();
        assert_eq!(ev.int("ac"), Some(12));
        assert!(ev.stats["ac"].parts.iter().any(|p| p.note.as_deref() == Some("unknown variable `nope`")));
        assert_eq!(ev.errors().count(), 1);
    }

    #[test]
    fn class_levels() {
        let mut m = model();
        m.class_levels.insert("wizard".into(), 3);
        m.stats.push(StatDef::new("recovery", "ceil(level('Wizard') / 2)", "Arcane Recovery"));
        assert_eq!(m.evaluate().int("recovery"), Some(2));
    }
}
