//! Rolling from the sheet: checks, saves, initiative, attacks with their
//! damage and extra damage, and spells. Conditions set advantage and
//! disadvantage; a one-shot mode can be picked for the next roll.

use dnt_data::{EntityId, Library};
use dnt_rules::attacks::Attack;
use dnt_rules::character::Hand;
use dnt_rules::conditions::{self, RollKind};
use dnt_rules::roll::{self, Advantage, DamageRules, Rng};
use dnt_rules::standard::ABILITIES;
use dnt_rules::Evaluation;

use crate::characters::Sheet;

/// One entry of the roll log.
#[derive(Debug, Clone, PartialEq)]
pub struct Rolled {
    pub title: String,
    /// The headline: `21 to hit`, `17`, `fails`.
    pub result: String,
    /// The dice and modifiers, and why the roll had advantage.
    pub detail: String,
    /// Further rolls (damage, extra damage).
    pub extra: Vec<String>,
    /// A natural 20 or 1, for colour.
    pub crit: Option<bool>,
}

impl Rolled {
    /// A single line for the status bar and the session note.
    pub fn line(&self) -> String {
        let mut s = format!("{}: {}", self.title, self.result);
        if !self.detail.is_empty() {
            s.push_str(&format!(" ({})", self.detail));
        }
        for e in &self.extra {
            s.push_str(&format!(" · {e}"));
        }
        s
    }
}

#[derive(Debug, Clone)]
pub struct Roller {
    rng: Rng,
    /// Advantage or disadvantage for the next roll only.
    pub next: Advantage,
    pub log: Vec<Rolled>,
    /// Append rolls to the campaign's latest session note.
    pub to_notes: bool,
}

impl Default for Roller {
    fn default() -> Roller {
        Roller { rng: Rng::from_time(), next: Advantage::Normal, log: Vec::new(), to_notes: false }
    }
}

impl Roller {
    /// Cycle the next roll: normal → advantage → disadvantage.
    pub fn cycle(&mut self) -> Advantage {
        self.next = match self.next {
            Advantage::Normal => Advantage::Advantage,
            Advantage::Advantage => Advantage::Disadvantage,
            Advantage::Disadvantage => Advantage::Normal,
        };
        self.next
    }

    fn push(&mut self, r: Rolled) -> Rolled {
        self.log.insert(0, r.clone());
        self.log.truncate(30);
        r
    }
}

fn rules_2024(sheet: &Sheet) -> bool {
    sheet.edition == dnt_rules::play::Edition::Modern
}

/// The roll's mode from conditions and the one-shot choice, and why.
fn mode(sheet: &mut Sheet, kind: RollKind) -> (Advantage, Vec<String>) {
    let st = &sheet.file.character.state;
    let mut sources = conditions::advantage(&st.conditions, st.exhaustion, rules_2024(sheet), kind);
    let next = std::mem::take(&mut sheet.roller.next);
    if next != Advantage::Normal {
        sources.push((next, "chosen".into()));
    }
    let mode = Advantage::combine(&sources.iter().map(|(a, _)| *a).collect::<Vec<_>>());
    let why = sources.into_iter().map(|(a, w)| format!("{} ({w})", a.label())).collect();
    (mode, why)
}

fn detail(r: &roll::D20, why: &[String]) -> String {
    let mut s = r.describe();
    if !why.is_empty() {
        s.push_str(&format!(" · {}", why.join(", ")));
    }
    s
}

fn d20_test(sheet: &mut Sheet, title: String, modifier: i64, kind: RollKind) -> Rolled {
    if let Some(why) = conditions::auto_fail(&sheet.file.character.state.conditions, kind) {
        return sheet.roller.push(Rolled { title, result: "fails".into(), detail: why, extra: Vec::new(), crit: None });
    }
    let (m, why) = mode(sheet, kind);
    let r = roll::d20(&mut sheet.roller.rng, modifier, m);
    let crit = r.natural_20().then_some(true).or(r.natural_1().then_some(false));
    let rolled = Rolled { title, result: r.total.to_string(), detail: detail(&r, &why), extra: Vec::new(), crit };
    sheet.roller.push(rolled)
}

fn ability_name(ab: &str) -> &'static str {
    ABILITIES.iter().find(|(k, _)| *k == ab).map(|(_, n)| *n).unwrap_or("?")
}

/// An ability check (`skill` for a skill check with that ability).
pub fn check(sheet: &mut Sheet, ab: &str, skill: Option<(&str, &str)>) -> Rolled {
    let ev = &sheet.ev;
    let (title, modifier) = match skill {
        Some((key, name)) => (name.to_string(), ev.int(&format!("skill.{key}")).unwrap_or(0)),
        None => (
            format!("{} check", ability_name(ab)),
            ev.int(&format!("mod.{ab}")).unwrap_or(0) + ev.int("d20.penalty").unwrap_or(0),
        ),
    };
    d20_test(sheet, title, modifier, RollKind::Check(ab))
}

pub fn save(sheet: &mut Sheet, ab: &str) -> Rolled {
    let modifier = sheet.ev.int(&format!("save.{ab}")).unwrap_or(0);
    d20_test(sheet, format!("{} save", ability_name(ab)), modifier, RollKind::Save(ab))
}

pub fn initiative(sheet: &mut Sheet) -> Rolled {
    let modifier = sheet.ev.int("initiative").unwrap_or(0);
    d20_test(sheet, "Initiative".into(), modifier, RollKind::Initiative)
}

/// Roll a stat's dice (damage) with the given rules.
fn roll_stat(rng: &mut Rng, ev: &Evaluation, key: &str, rules: DamageRules) -> Option<roll::Damage> {
    ev.get(key).map(|v| roll::damage(rng, v, rules))
}

/// An attack: the attack roll, then damage (doubled dice on a natural 20)
/// and each kind of extra damage on a hit, to add when it applies.
pub fn attack(sheet: &mut Sheet, a: &Attack) -> Rolled {
    let bonus = sheet.ev.int(&format!("attack.{}.bonus", a.slug)).unwrap_or(0);
    let (m, why) = mode(sheet, RollKind::Attack);
    let r = roll::d20(&mut sheet.roller.rng, bonus, m);
    let critical = r.natural_20();
    let mut rules = DamageRules { critical, ..Default::default() };
    if let Some(g) = a.great_weapon.filter(|_| a.hand == Some(Hand::Both) || !a.has_versatile) {
        rules.min_three = g;
        rules.reroll_low = !g;
    }
    // Which damage: off hand, two-handed (versatile in both hands), or the usual.
    let key = match a.hand {
        Some(Hand::Off) if a.offhand => format!("attack.{}.offhand", a.slug),
        Some(Hand::Both) if a.has_versatile => format!("attack.{}.versatile", a.slug),
        _ => format!("attack.{}.damage", a.slug),
    };
    let mut extra = Vec::new();
    let ev = sheet.ev.clone();
    if let Some(d) = roll_stat(&mut sheet.roller.rng, &ev, &key, rules) {
        extra.push(format!("{} {} damage ({})", d.total, a.damage_type, d.describe()));
    }
    if a.hand == Some(Hand::Pair) {
        if let Some(d) = roll_stat(&mut sheet.roller.rng, &ev, &format!("attack.{}.offhand", a.slug), rules) {
            extra.push(format!("off hand {} ({})", d.total, d.describe()));
        }
    }
    let riders: Vec<(String, String)> = ev
        .stats
        .iter()
        .filter(|(k, _)| k.starts_with("rider."))
        .map(|(k, r)| {
            let why = r.parts.iter().rev().find(|p| p.applied).map(|p| p.origin.clone()).unwrap_or_default();
            (k.clone(), why.split(':').next().unwrap_or("").to_string())
        })
        .collect();
    for (k, name) in riders {
        let rider_rules = DamageRules { critical, ..Default::default() };
        if let Some(d) = roll_stat(&mut sheet.roller.rng, &ev, &k, rider_rules) {
            extra.push(format!("+{} {name} if it applies ({})", d.total, d.describe()));
        }
    }
    let hit = if critical {
        "critical hit".to_string()
    } else if r.natural_1() {
        "miss (natural 1)".to_string()
    } else {
        format!("{} to hit", r.total)
    };
    let crit = r.natural_20().then_some(true).or(r.natural_1().then_some(false));
    let rolled = Rolled { title: a.name.clone(), result: hit, detail: detail(&r, &why), extra, crit };
    sheet.roller.push(rolled)
}

/// A spell: its attack roll or save DC, and its damage or healing at
/// `slot_level` (cantrips scale with the character's level).
pub fn spell(sheet: &mut Sheet, lib: &Library, spell: EntityId, slot_level: i64, attack_bonus: i64, dc: i64) -> Rolled {
    let level = sheet.file.character.total_level() as i64;
    let s = dnt_rules::spells::spell_roll(lib, spell, level, slot_level);
    let name = lib.entity(spell).name.clone();
    let title = if slot_level > dnt_rules::spells::spell_level(lib, spell) {
        format!("{name} (level {slot_level})")
    } else {
        name
    };
    let mut critical = false;
    let (result, detail_text, crit) = if s.attack {
        let (m, why) = mode(sheet, RollKind::Attack);
        let r = roll::d20(&mut sheet.roller.rng, attack_bonus, m);
        critical = r.natural_20();
        let result = if critical { "critical hit".into() } else if r.natural_1() { "miss (natural 1)".into() } else { format!("{} to hit", r.total) };
        (result, detail(&r, &why), r.natural_20().then_some(true).or(r.natural_1().then_some(false)))
    } else if let Some(save) = &s.save {
        (format!("DC {dc}"), format!("{} save", dnt_data::view::title_case(save)), None)
    } else {
        ("cast".to_string(), String::new(), None)
    };
    let mut extra = Vec::new();
    if let Some(dice) = &s.dice {
        // "… plus your spellcasting ability modifier": the DC is 8 + proficiency + it.
        let dice = if s.add_modifier { dice.plus_flat(dc - 8 - sheet.ev.int("prof").unwrap_or(2)) } else { dice.clone() };
        let d = roll::damage(&mut sheet.roller.rng, &dnt_rules::Value::Dice(dice), DamageRules { critical, ..Default::default() });
        let what = if s.kind == "healing" { "healing".to_string() } else { format!("{} damage", s.kind) };
        extra.push(format!("{} {what} ({})", d.total, d.describe()));
        if s.save.is_some() && s.kind != "healing" {
            extra.push(format!("half on a success: {}", d.total / 2));
        }
    }
    sheet.roller.push(Rolled { title, result, detail: detail_text, extra, crit })
}

/// The spell attack bonus and save DC to use: the casting class's (the
/// best one if there are several), else proficiency and the best
/// mental ability.
pub fn spell_numbers(sheet: &Sheet, caster_slug: Option<&str>) -> (i64, i64) {
    let ev = &sheet.ev;
    let of = |slug: &str| Some((ev.int(&format!("spell.{slug}.attack"))?, ev.int(&format!("spell.{slug}.dc"))?));
    if let Some(n) = caster_slug.and_then(of) {
        return n;
    }
    let best = sheet.build.classes.iter().filter_map(|c| of(&c.slug)).max();
    best.unwrap_or_else(|| {
        let m = ["int", "wis", "cha"].iter().filter_map(|a| ev.int(&format!("mod.{a}"))).max().unwrap_or(0);
        let p = ev.int("prof").unwrap_or(2);
        (m + p, 8 + m + p)
    })
}

/// The slot level a cast used (`slot:3`, `pact`, `free`, `ritual`).
pub fn cast_level(sheet: &Sheet, lib: &Library, spell: EntityId, how: &str) -> i64 {
    let base = dnt_rules::spells::spell_level(lib, spell);
    match how {
        "pact" => match sheet.build.model.inputs.get("pact.level") {
            Some(dnt_rules::Value::Num(n)) => n.floor().max(base),
            _ => base,
        },
        h => h.strip_prefix("slot:").and_then(|n| n.parse().ok()).unwrap_or(base),
    }
}

/// Whether rolling the spell gives anything (an attack or dice).
pub fn spell_rolls(lib: &Library, spell: EntityId) -> bool {
    let s = dnt_rules::spells::spell_roll(lib, spell, 1, 9);
    s.attack || s.dice.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use dnt_rules::character::{Character, InventoryItem};
    use dnt_rules::overlay::OverlaySet;
    use dnt_store::CharacterFile;
    use serde_json::json;

    fn sheet(conditions: &[&str]) -> (Library, Sheet) {
        let mut lib = Library::new();
        lib.load_json(&json!({"baseitem": [{"name": "Club", "source": "T", "type": "M|T", "weapon": true, "weaponCategory": "simple",
                                             "dmg1": "1d4", "dmgType": "B"}]}));
        let mut c = Character { name: "A".into(), ..Character::default() };
        c.inventory.push(InventoryItem { item: Some("item:Club|T".into()), hand: Some(Hand::Main), ..Default::default() });
        c.state.conditions = conditions.iter().map(|s| s.to_string()).collect();
        let file = CharacterFile { slug: "a".into(), path: "/tmp/a.toml".into(), character: c };
        let s = Sheet::new(file, &lib, &OverlaySet::default(), &Default::default());
        (lib, s)
    }

    #[test]
    fn attacks_saves_and_modes() {
        let (_lib, mut s) = sheet(&["Stunned"]);
        let club = s.build.attacks[0].clone();
        let r = attack(&mut s, &club);
        assert!(r.result.contains("to hit") || r.result.contains("critical") || r.result.contains("miss"));
        assert!(r.extra[0].contains("bludgeoning damage"), "{:?}", r.extra);
        assert_eq!(save(&mut s, "dex").result, "fails", "Stunned");
        assert_ne!(save(&mut s, "wis").result, "fails");
        // The chosen mode lasts one roll.
        s.roller.cycle();
        let r = check(&mut s, "str", None);
        assert!(r.detail.contains("advantage (chosen)"), "{}", r.detail);
        assert!(!check(&mut s, "str", None).detail.contains("chosen"));
        assert_eq!(s.roller.log.len(), 5);
    }
}
