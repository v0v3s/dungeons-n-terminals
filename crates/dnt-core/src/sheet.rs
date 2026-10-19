//! A character sheet as data to draw: the header, each tab (overview with
//! attacks, features, spells, inventory with equipment) and the breakdown
//! of a stat. Built from a session; frontends lay it out.

use dnt_data::{EntityId, Library};
use dnt_rules::builder::FeatureSource;
use dnt_rules::character::{Hand, Slot};
use dnt_rules::engine::PartKind;
use dnt_rules::overlay::{resolve_id, OverlaySet, Recharge};
use dnt_rules::spells::{self, Grant};
use dnt_rules::standard::{ABILITIES, SKILLS};
use dnt_rules::{conditions, equipment, play, Evaluation, Value};
use serde::Serialize;

use crate::rolls::Rolled;
use crate::session::{item_entity, line_name, CharacterSession};
use crate::spellbook::{self, Row};

fn stat(ev: &Evaluation, key: &str) -> String {
    ev.get(key).map_or("—".into(), |v| v.to_string())
}

fn signed(ev: &Evaluation, key: &str) -> String {
    match ev.get(key) {
        Some(Value::Num(n)) if n.is_int() => format!("{:+}", n.floor()),
        Some(v) => v.to_string(),
        None => "—".into(),
    }
}

// ---- header ----

#[derive(Debug, Clone, Serialize)]
pub struct Header {
    pub name: String,
    /// `Elf · Sage · Wizard 3 · Level 3`.
    pub identity: String,
    pub level: i64,
    /// Class choices still to make (see `CharacterSession::pending`).
    pub pending: usize,
    /// Spells still to choose.
    pub spells_to_choose: i64,
    pub warnings: Vec<String>,
}

/// `Species · Background · Classes · Level N`.
pub fn identity(lib: &Library, s: &CharacterSession) -> String {
    let ch = s.character();
    let name_of = |id: &Option<String>| id.as_deref().and_then(|id| resolve_id(lib, id)).map(|e| lib.entity(e).name.clone());
    let mut parts: Vec<String> = [name_of(&ch.species), name_of(&ch.background)].into_iter().flatten().collect();
    let classes = s.build.class_summary();
    if !classes.is_empty() {
        parts.push(classes);
    }
    parts.push(format!("Level {}", ch.total_level()));
    parts.join(" · ")
}

pub fn header(lib: &Library, s: &CharacterSession) -> Header {
    let gaps = spellbook::spell_gaps(s.character(), &spells::casters(lib, &s.build, &s.ev));
    Header {
        name: s.character().name.clone(),
        identity: identity(lib, s),
        level: s.character().total_level() as i64,
        pending: s.pending.len(),
        spells_to_choose: gaps.iter().map(|(_, c, sp)| c + sp).sum(),
        warnings: s.build.warnings.clone(),
    }
}

// ---- overview ----

#[derive(Debug, Clone, Serialize)]
pub struct Ability {
    pub key: &'static str,
    pub name: &'static str,
    pub score: String,
    pub modifier: String,
    pub save: String,
    pub proficient: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Casting {
    pub name: String,
    pub dc: String,
    pub attack: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Skill {
    pub key: &'static str,
    pub name: &'static str,
    pub ability: &'static str,
    /// `none`, `half`, `proficient` or `expertise`.
    pub proficiency: &'static str,
    pub bonus: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Tile {
    pub key: &'static str,
    pub label: &'static str,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Hp {
    pub current: i64,
    pub max: i64,
    pub temp: i64,
    pub death_successes: i64,
    pub death_failures: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Condition {
    pub name: String,
    pub summary: String,
}

/// A pool to spend from: a resource, a spell slot level, pact slots or
/// hit dice (see `commands::Pool`).
#[derive(Debug, Clone, Serialize)]
pub struct Tracker {
    /// `resource:<key>`, `slot:<level>`, `pact`, `hit_die:<faces>`.
    pub id: String,
    pub name: String,
    pub left: i64,
    pub max: i64,
    /// When it comes back.
    pub recharge: String,
    /// The stat to break down.
    pub stat: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Toggle {
    pub key: String,
    pub name: String,
    pub on: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct AttackRow {
    pub slug: String,
    pub name: String,
    /// `main`, `off`, `both`, `pair` or empty.
    pub hand: &'static str,
    pub hit: String,
    pub proficient: bool,
    pub damage: String,
    pub range: String,
    /// Shown in the accent colour (mastery, off-hand, monk weapon).
    pub notes: Vec<String>,
    /// Problems (no proficiency, needs attunement), shown in red.
    pub problems: Vec<String>,
    pub properties: Vec<String>,
    /// The inventory line of the weapon (not for an unarmed strike).
    pub line: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Rider {
    pub value: String,
    pub name: String,
    pub when: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Overview {
    pub abilities: Vec<Ability>,
    pub casting: Vec<Casting>,
    pub skills: Vec<Skill>,
    pub tiles: Vec<Tile>,
    pub hp: Hp,
    pub conditions: Vec<Condition>,
    pub exhaustion: i64,
    pub exhaustion_summary: String,
    pub concentration: Option<String>,
    /// Newest first.
    pub rolls: Vec<Rolled>,
    /// The next roll's mode: `normal`, `advantage`, `disadvantage`.
    pub next_roll: String,
    pub trackers: Vec<Tracker>,
    pub toggles: Vec<Toggle>,
    pub held: Vec<String>,
    pub attacks: Vec<AttackRow>,
    pub riders: Vec<Rider>,
}

pub fn overview(lib: &Library, s: &CharacterSession) -> Overview {
    let (ch, ev, build) = (s.character(), &s.ev, &s.build);
    let abilities = ABILITIES
        .iter()
        .map(|(a, name)| Ability {
            key: a,
            name,
            score: stat(ev, a),
            modifier: signed(ev, &format!("mod.{a}")),
            save: signed(ev, &format!("save.{a}")),
            proficient: ev.get(&format!("save.{a}")) != ev.get(&format!("mod.{a}")),
        })
        .collect();
    let casting = build
        .classes
        .iter()
        .filter(|c| c.casting_ability.is_some())
        .map(|c| Casting {
            name: c.name.clone(),
            dc: stat(ev, &format!("spell.{}.dc", c.slug)),
            attack: signed(ev, &format!("spell.{}.attack", c.slug)),
        })
        .collect();
    let skills = SKILLS
        .iter()
        .map(|(k, name, ab)| {
            let mult = build.model.inputs.get(&format!("prof.skill.{k}")).cloned().unwrap_or(Value::int(0));
            Skill {
                key: k,
                name,
                ability: ab,
                proficiency: match mult.to_string().as_str() {
                    "2" => "expertise",
                    "1" => "proficient",
                    "0" => "none",
                    _ => "half",
                },
                bonus: signed(ev, &format!("skill.{k}")),
            }
        })
        .collect();
    let tiles = [
        ("ac", "Armor Class", false),
        ("initiative", "Initiative", true),
        ("speed", "Speed", false),
        ("prof", "Proficiency", true),
        ("passive.perception", "Passive Perception", false),
    ]
    .into_iter()
    .map(|(key, label, sign)| Tile { key, label, value: if sign { signed(ev, key) } else { stat(ev, key) } })
    .collect();

    let st = &ch.state;
    let mut trackers = Vec::new();
    for (r, _) in &build.resources {
        let recharge = match (r.recharge, r.short_rest_regain.is_some()) {
            (Recharge::ShortRest, _) => "short rest",
            (Recharge::LongRest, true) => "long rest · 1 on a short",
            (Recharge::LongRest, false) => "long rest",
            (Recharge::Dawn, _) => "dawn",
            (Recharge::Manual, _) => "manual",
        };
        trackers.push(Tracker {
            id: format!("resource:{}", r.key),
            name: r.name.clone(),
            left: play::resource_left(ch, ev, &r.key),
            max: play::resource_max(ev, &r.key),
            recharge: recharge.into(),
            stat: format!("resource.{}.max", r.key),
        });
    }
    for l in (1..=9).filter(|l| play::slot_max(ev, *l) > 0) {
        trackers.push(Tracker {
            id: format!("slot:{l}"),
            name: format!("Level {l} slots"),
            left: play::slots_left(ch, ev, l),
            max: play::slot_max(ev, l),
            recharge: "long rest".into(),
            stat: format!("slots.{l}.max"),
        });
    }
    let pact = ev.int("pact.slots.max").unwrap_or(0);
    if pact > 0 {
        let level = build.model.inputs.get("pact.level").map(|v| v.to_string()).unwrap_or_default();
        trackers.push(Tracker {
            id: "pact".into(),
            name: format!("Pact slots (level {level})"),
            left: pact - st.pact_slots_used,
            max: pact,
            recharge: "short rest".into(),
            stat: "pact.slots.max".into(),
        });
    }
    let mut dice: Vec<i64> = ev.stats.keys().filter_map(|k| k.strip_prefix("hit_dice.d")?.strip_suffix(".max")?.parse().ok()).collect();
    dice.sort();
    for f in dice {
        let max = ev.int(&format!("hit_dice.d{f}.max")).unwrap_or(0);
        trackers.push(Tracker {
            id: format!("hit_die:{f}"),
            name: format!("Hit dice d{f}"),
            left: max - st.hit_dice_used.get(&format!("d{f}")).copied().unwrap_or(0),
            max,
            recharge: "long rest".into(),
            stat: format!("hit_dice.d{f}.max"),
        });
    }

    let held = ch
        .inventory
        .iter()
        .filter_map(|l| {
            let name = line_name(lib, l);
            Some(match l.hand? {
                Hand::Pair => format!("{name} ×2"),
                Hand::Both => format!("{name} (two hands)"),
                _ => name,
            })
        })
        .collect();
    let attacks = build
        .attacks
        .iter()
        .map(|a| {
            let get = |part: &str| ev.get(&format!("attack.{}.{part}", a.slug)).map(|v| v.to_string());
            let mut damage = format!("{} {}", get("damage").unwrap_or_else(|| "?".into()), a.damage_type);
            if let Some(two) = get("versatile") {
                damage.push_str(&format!(" ({two})"));
            }
            let mut notes = Vec::new();
            match (a.hand, get("offhand")) {
                (Some(Hand::Off), Some(off)) => {
                    damage = format!("{off} {}", a.damage_type);
                    notes.push("off-hand attack".into());
                }
                (Some(Hand::Pair), Some(off)) => notes.push(format!("off hand {off}")),
                _ => {}
            }
            if a.monk_weapon {
                notes.push("monk weapon".into());
            }
            if let Some(m) = &a.mastery {
                notes.push(format!("mastery: {m}"));
            }
            AttackRow {
                slug: a.slug.clone(),
                name: a.name.clone(),
                hand: match a.hand {
                    Some(Hand::Main) => "main",
                    Some(Hand::Off) => "off",
                    Some(Hand::Both) => "both",
                    Some(Hand::Pair) => "pair",
                    None => "",
                },
                hit: ev.int(&format!("attack.{}.bonus", a.slug)).map_or("?".into(), |n| format!("{n:+}")),
                proficient: a.proficient,
                damage,
                range: a.range.clone(),
                notes,
                problems: a.notes.clone(),
                properties: a.properties.clone(),
                // The line holding it, else any line of that weapon.
                line: a.item.and_then(|item| {
                    let lines = || ch.inventory.iter().enumerate().filter(|(_, l)| item_entity(lib, l) == Some(item));
                    lines().find(|(_, l)| l.hand.is_some()).or_else(|| lines().next()).map(|(i, _)| i)
                }),
            }
        })
        .collect();
    let riders = ev
        .stats
        .iter()
        .filter(|(k, _)| k.starts_with("rider."))
        .filter_map(|(_, r)| {
            let v = r.value.as_ref().ok()?;
            let why = r.parts.iter().rev().find(|p| p.applied).map(|p| p.origin.clone()).unwrap_or_default();
            let (name, when) = why.split_once(": ").unwrap_or((why.as_str(), ""));
            Some(Rider { value: format!("+{v}"), name: name.into(), when: when.into() })
        })
        .collect();
    let modern = s.edition == play::Edition::Modern;
    Overview {
        abilities,
        casting,
        skills,
        tiles,
        hp: Hp {
            current: play::current_hp(ch, ev),
            max: play::max_hp(ev),
            temp: st.temp_hp,
            death_successes: st.death_successes,
            death_failures: st.death_failures,
        },
        conditions: st.conditions.iter().map(|c| Condition { name: c.clone(), summary: conditions::summary(c).into() }).collect(),
        exhaustion: st.exhaustion,
        exhaustion_summary: conditions::exhaustion_summary(st.exhaustion, modern),
        concentration: st.concentration.clone(),
        rolls: s.roller.log.iter().take(6).cloned().collect(),
        next_roll: format!("{:?}", s.roller.next).to_lowercase(),
        trackers,
        toggles: build
            .toggles
            .iter()
            .map(|(t, _)| Toggle { key: t.key.clone(), name: t.name.clone(), on: st.toggles.get(&t.key).copied().unwrap_or(false) })
            .collect(),
        held,
        attacks,
        riders,
    }
}

/// Every condition, with whether the character has it.
pub fn conditions(s: &CharacterSession) -> Vec<(Condition, bool)> {
    let st = &s.character().state;
    conditions::CONDITIONS
        .iter()
        .map(|c| {
            (
                Condition { name: c.to_string(), summary: conditions::summary(c).into() },
                st.conditions.iter().any(|x| x.eq_ignore_ascii_case(c)),
            )
        })
        .collect()
}

// ---- breakdown ----

#[derive(Debug, Clone, Serialize)]
pub struct Part {
    /// `base`, `add`, `set`, `best of`, `cap`, `times`, `override`.
    pub kind: &'static str,
    pub value: String,
    pub origin: String,
    pub note: Option<String>,
    pub applied: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Breakdown {
    pub title: String,
    pub value: String,
    pub parts: Vec<Part>,
}

fn stat_label(s: &CharacterSession, key: &str) -> String {
    if let Some((_, name)) = ABILITIES.iter().find(|(a, _)| *a == key) {
        return name.to_string();
    }
    if let Some((_, name, _)) = key.strip_prefix("skill.").and_then(|k| SKILLS.iter().find(|(s, _, _)| *s == k)) {
        return name.to_string();
    }
    if let Some((r, _)) =
        key.strip_prefix("resource.").and_then(|k| k.strip_suffix(".max")).and_then(|k| s.build.resources.iter().find(|(r, _)| r.key == k))
    {
        return r.name.clone();
    }
    match key {
        "ac" => "Armor Class".into(),
        "hp.max" => "Hit Point Maximum".into(),
        "initiative" => "Initiative".into(),
        "speed" => "Speed".into(),
        "prof" => "Proficiency Bonus".into(),
        "passive.perception" => "Passive Perception".into(),
        "pact.slots.max" => "Pact slots".into(),
        k => k.to_string(),
    }
}

/// How a stat is made up (`ac`, `skill.stealth`, `attack.<slug>.bonus`).
pub fn breakdown(s: &CharacterSession, key: &str) -> Option<Breakdown> {
    let r = s.ev.stats.get(key)?;
    let parts = r
        .parts
        .iter()
        .map(|p| Part {
            kind: match p.kind {
                PartKind::Base => "base",
                PartKind::Add => "add",
                PartKind::Set => "set",
                PartKind::Max => "best of",
                PartKind::Min => "cap",
                PartKind::Mul => "times",
                PartKind::Override => "override",
            },
            value: p.value.as_ref().map_or("—".into(), |v| v.to_string()),
            origin: p.origin.clone(),
            note: p.note.clone(),
            applied: p.applied,
        })
        .collect();
    let value = match &r.value {
        Ok(v) => v.to_string(),
        Err(e) => format!("error: {e}"),
    };
    let title = match key.strip_prefix("attack.").and_then(|k| k.rsplit_once('.')) {
        Some((slug, part)) => {
            let name = s.build.attacks.iter().find(|a| a.slug == slug).map_or(slug.to_string(), |a| a.name.clone());
            format!("{name}: {}", if part == "bonus" { "attack bonus" } else { part })
        }
        None => stat_label(s, key),
    };
    Some(Breakdown { title, value, parts })
}

// ---- features ----

#[derive(Debug, Clone, Serialize)]
pub struct Feature {
    /// Index into the build's features.
    pub index: usize,
    pub entity: EntityId,
    pub name: String,
    pub level: i64,
    /// Has rules that change the sheet.
    pub automated: bool,
    /// A weapon mastery (can be swapped): the weapon's id.
    pub mastery: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FeatureGroup {
    pub title: String,
    pub features: Vec<Feature>,
}

pub fn features(lib: &Library, overlays: &OverlaySet, s: &CharacterSession) -> Vec<FeatureGroup> {
    let mut groups: Vec<FeatureGroup> = Vec::new();
    for (i, f) in s.build.features.iter().enumerate() {
        let title = match f.source {
            FeatureSource::Class(ci) => format!("{} {}", s.build.classes[ci].name, s.build.classes[ci].level),
            FeatureSource::Subclass(ci) => s.build.classes[ci].subclass.map_or("Subclass".into(), |e| lib.entity(e).name.clone()),
            FeatureSource::Species => "Species".into(),
            FeatureSource::Background => "Background".into(),
            FeatureSource::Feat => "Feats".into(),
            FeatureSource::Option => "Options".into(),
            FeatureSource::Mastery => "Weapon Mastery".into(),
        };
        let feature = Feature {
            index: i,
            entity: f.entity,
            name: lib.entity(f.entity).name.clone(),
            level: f.level,
            automated: !overlays.for_entity(&lib.entity(f.entity).key).is_empty(),
            mastery: (f.source == FeatureSource::Mastery).then(|| {
                let e = lib.entity(f.entity);
                format!("item:{}|{}", e.name, e.source)
            }),
        };
        match groups.last_mut() {
            Some(g) if g.title == title => g.features.push(feature),
            _ => groups.push(FeatureGroup { title, features: vec![feature] }),
        }
    }
    groups
}

// ---- spells ----

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SpellRow {
    /// A casting class, or a feature that grants spells, or the favorites.
    Header {
        title: String,
        /// `DC 13 · +5 · up to level 2`.
        detail: String,
        /// `(label, have, max)`: cantrips, prepared/known, spellbook.
        counts: Vec<(String, usize, i64)>,
        /// Add spells here (a casting class's id).
        class_id: Option<String>,
    },
    Spell {
        id: String,
        entity: Option<EntityId>,
        name: String,
        /// `cantrip`, `prepared`, `book`, `granted`.
        mark: &'static str,
        /// `8d6 fire · level 3 · C`.
        detail: String,
        favorite: bool,
        /// Cast from slots (or free casts): leveled spells.
        castable: bool,
        /// Free casts (innate), if any are left to spend.
        free: bool,
        /// Where it was added (to remove it from there), if it can be.
        target: Option<spellbook::Target>,
        /// A spellbook spell (Wizards): can be prepared or unprepared.
        class_id: Option<String>,
        /// Shown again among the favorites.
        in_favorites: bool,
    },
    /// A feature's spell choice with picks still missing.
    Choice { key: String, filter: String, missing: usize, text: String },
}

pub fn spells(lib: &Library, s: &CharacterSession) -> Vec<SpellRow> {
    let (ch, ev) = (s.character(), &s.ev);
    let data = spellbook::rows(lib, s);
    let mut out = Vec::new();
    for row in &data.rows {
        let (row, in_favorites) = match row {
            Row::Favorite(inner) => (&**inner, true),
            r => (r, false),
        };
        let spell = |id: &str, mark: &'static str, how: String, class_id: Option<String>, free: bool| {
            let e = resolve_id(lib, id);
            let mut detail = Vec::new();
            if !how.is_empty() {
                detail.push(how);
            }
            let mut castable = false;
            if let Some(e) = e {
                let lvl = spells::spell_level(lib, e);
                castable = lvl > 0;
                let r = spells::spell_roll(lib, e, ch.total_level() as i64, lvl);
                let caster = match row {
                    Row::ClassSpell { caster, .. } => data.casters.get(*caster).map(|c| c.slug.as_str()),
                    _ => None,
                };
                let (_, dc) = crate::rolls::spell_numbers(s, caster);
                let modifier = dc - 8 - ev.int("prof").unwrap_or(2);
                if let Some(d) = r.dice.as_ref().map(|d| if r.add_modifier { d.plus_flat(modifier) } else { d.clone() }) {
                    detail.push(format!("{d} {}", if r.kind.is_empty() { "damage" } else { r.kind.split(" or ").next().unwrap_or("") }));
                }
                detail.push(if lvl == 0 { "cantrip".into() } else { format!("level {lvl}") });
                if spells::concentration(lib, e) {
                    detail.push("C".into());
                }
                if spells::ritual(lib, e) {
                    detail.push("R".into());
                }
            }
            if in_favorites {
                match row {
                    Row::ClassSpell { caster, .. } => detail.push(data.casters[*caster].name.clone()),
                    Row::GrantSpell { grant, .. } => detail.push(s.build.granted[*grant].origin.clone()),
                    _ => {}
                }
            }
            SpellRow::Spell {
                id: id.to_string(),
                entity: e,
                name: e.map_or_else(|| id.to_string(), |e| lib.entity(e).name.clone()),
                mark,
                detail: detail.join(" · "),
                favorite: ch.spells.favorites.iter().any(|f| f == id),
                castable,
                free,
                target: row.target(&data.casters),
                class_id,
                in_favorites,
            }
        };
        out.push(match row {
            Row::FavoritesHeader => {
                SpellRow::Header { title: "★ Favorites".into(), detail: String::new(), counts: Vec::new(), class_id: None }
            }
            Row::ClassHeader(c) => {
                let caster = &data.casters[*c];
                let mine = ch.spells.classes.get(&caster.id).cloned().unwrap_or_default();
                let l = &caster.limits;
                let mut counts = Vec::new();
                if l.cantrips > 0 {
                    counts.push(("cantrips".into(), mine.cantrips.len(), l.cantrips));
                }
                let label = match l.list {
                    spells::SpellList::Prepared => "prepared",
                    spells::SpellList::Known => "known",
                };
                counts.push((label.into(), mine.prepared.len(), l.spells));
                if let Some(book) = l.spellbook {
                    let in_book = mine.spellbook.iter().chain(&mine.prepared).collect::<std::collections::BTreeSet<_>>().len();
                    counts.push(("book".into(), in_book, book));
                }
                SpellRow::Header {
                    title: format!("{} {}", caster.name, caster.level),
                    detail: format!(
                        "DC {} · {} · up to level {}",
                        stat(ev, &format!("spell.{}.dc", caster.slug)),
                        signed(ev, &format!("spell.{}.attack", caster.slug)),
                        l.max_level
                    ),
                    counts,
                    class_id: Some(caster.id.clone()),
                }
            }
            Row::ClassSpell { id, slot, caster } => {
                let wizard = data.casters[*caster].limits.spellbook.is_some().then(|| data.casters[*caster].id.clone());
                let (mark, how, class_id) = match slot {
                    spellbook::Slot::Cantrip => ("cantrip", String::new(), None),
                    spellbook::Slot::Prepared => ("prepared", String::new(), wizard),
                    spellbook::Slot::Book => ("book", "spellbook".to_string(), wizard),
                };
                spell(id, mark, how, class_id, false)
            }
            Row::GrantHeader(g) => {
                SpellRow::Header { title: s.build.granted[*g].origin.clone(), detail: String::new(), counts: Vec::new(), class_id: None }
            }
            Row::GrantSpell { id, how, .. } => {
                let (text, free) = match how {
                    Grant::Prepared => ("always prepared".to_string(), false),
                    Grant::Known => ("known".to_string(), false),
                    Grant::Innate(None) => ("at will".to_string(), false),
                    Grant::Innate(Some(n)) => {
                        let name = resolve_id(lib, id).map(|e| lib.entity(e).name.clone()).unwrap_or_default();
                        let left = play::resource_left(ch, ev, &format!("innate_{}", dnt_rules::class_table::slug(&name)));
                        (format!("{left}/{n} free"), true)
                    }
                };
                spell(id, "granted", text, None, free)
            }
            Row::GrantChoice { key, filter, missing, .. } => SpellRow::Choice {
                key: key.clone(),
                filter: filter.clone(),
                missing: *missing,
                text: format!("choose {missing} {}", spells::describe_filter(filter)),
            },
            Row::Favorite(_) => unreachable!("unwrapped above"),
        });
    }
    out
}

// ---- inventory ----

#[derive(Debug, Clone, Serialize)]
pub struct Line {
    /// Index into the inventory (what commands take).
    pub index: usize,
    pub depth: usize,
    pub name: String,
    pub entity: Option<EntityId>,
    pub quantity: i64,
    /// Pounds for the whole line.
    pub weight: f64,
    /// Worn in a slot (its name), or held (the hand), or its armor kind.
    pub kind: String,
    pub worn: bool,
    pub held: bool,
    pub attuned: bool,
    pub needs_attunement: bool,
    /// A container: how full (`12/30 lb`), and whether it is over.
    pub fill: Option<String>,
    pub over: bool,
    pub weightless: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct WornSlot {
    pub slot: Slot,
    pub label: &'static str,
    /// Inventory lines worn there: (index, name).
    pub items: Vec<(usize, String)>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Inventory {
    pub lines: Vec<Line>,
    pub carried: f64,
    pub capacity: i64,
    pub attuned: usize,
    /// `(coin, amount)`, gold always.
    pub coins: Vec<(&'static str, i64)>,
    pub worn: Vec<WornSlot>,
    pub main_hand: Option<(usize, String)>,
    pub off_hand: Option<(usize, String)>,
}

pub fn inventory(lib: &Library, s: &CharacterSession) -> Inventory {
    let ch = s.character();
    let lines = equipment::tree(ch, &Default::default())
        .into_iter()
        .map(|(i, depth)| {
            let line = &ch.inventory[i];
            let entity = item_entity(lib, line);
            let container = equipment::container(lib, line);
            let (fill, over) = match &container {
                Some(k) => {
                    let held = equipment::contents_weight(lib, ch, i);
                    let text = match k.capacity {
                        Some(cap) => format!("{held:.0}/{cap:.0} lb"),
                        None => format!("{held:.0} lb"),
                    };
                    (Some(text), k.capacity.is_some_and(|cap| held > cap))
                }
                None => (None, false),
            };
            let kind = match (line.slot.filter(|_| line.equipped), line.hand) {
                (Some(slot), _) => slot.label().to_lowercase(),
                (_, Some(h)) => h.label().to_string(),
                _ => entity.and_then(|e| equipment::armor_kind(&lib.entity(e).data)).map(str::to_string).unwrap_or_default(),
            };
            Line {
                index: i,
                depth,
                name: line_name(lib, line),
                entity,
                quantity: line.quantity,
                weight: equipment::weight(lib, line) * line.quantity as f64,
                kind,
                worn: line.equipped,
                held: line.hand.is_some(),
                attuned: line.attuned,
                needs_attunement: entity.is_some_and(|e| equipment::requires_attunement(&lib.entity(e).data)),
                fill,
                over,
                weightless: container.is_some_and(|k| k.weightless),
            }
        })
        .collect();
    let c = &ch.currency;
    let coins =
        [("pp", c.pp), ("gp", c.gp), ("ep", c.ep), ("sp", c.sp), ("cp", c.cp)].into_iter().filter(|(l, n)| *n != 0 || *l == "gp").collect();
    let worn = Slot::ALL
        .iter()
        .map(|&slot| WornSlot {
            slot,
            label: slot.label(),
            items: ch
                .inventory
                .iter()
                .enumerate()
                .filter(|(_, l)| l.equipped && l.slot == Some(slot))
                .map(|(i, l)| (i, line_name(lib, l)))
                .collect(),
        })
        .collect();
    let hand = |main: bool| {
        ch.inventory
            .iter()
            .enumerate()
            .find(|(_, l)| l.hand.is_some_and(|h| if main { h.uses_main() } else { h.uses_off() }))
            .map(|(i, l)| (i, line_name(lib, l)))
    };
    Inventory {
        lines,
        carried: equipment::carried_weight(lib, ch),
        capacity: s.ev.int("str").unwrap_or(10) * 15,
        attuned: ch.inventory.iter().filter(|l| l.attuned).count(),
        coins,
        worn,
        main_hand: hand(true),
        off_hand: hand(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dnt_rules::character::Character;
    use dnt_store::CharacterFile;
    use serde_json::json;

    #[test]
    fn a_sheet_serializes() {
        let mut lib = Library::new();
        lib.load_json(&json!({
            "class": [{"name": "Fighter", "source": "T", "hd": {"number": 1, "faces": 10}}],
            "baseitem": [{"name": "Club", "source": "T", "type": "M|T", "weapon": true, "weaponCategory": "simple", "dmg1": "1d4", "dmgType": "B"}]
        }));
        let mut c = Character { name: "Ada".into(), ..Default::default() };
        c.levels.push(dnt_rules::character::Level { class: "class:Fighter|T".into(), hp: Default::default(), choices: Default::default() });
        equipment::add_item(&mut c, "item:Club|T", 1, None);
        let file = CharacterFile { slug: "ada".into(), path: "/tmp/ada.toml".into(), character: c };
        let s = CharacterSession::new(file, &lib, &OverlaySet::default(), &Default::default());

        let h = header(&lib, &s);
        assert_eq!((h.name.as_str(), h.level), ("Ada", 1));
        assert!(h.identity.contains("Fighter 1"), "{}", h.identity);
        let o = overview(&lib, &s);
        assert_eq!(o.abilities.len(), 6);
        assert_eq!(o.hp.max, o.hp.current);
        assert!(o.trackers.iter().any(|t| t.id == "hit_die:10"));
        assert!(o.attacks.iter().any(|a| a.name == "Club"));
        let inv = inventory(&lib, &s);
        assert_eq!(inv.lines[0].name, "Club");
        assert!(breakdown(&s, "ac").is_some_and(|b| !b.parts.is_empty()));
        // All of it goes over the wire.
        serde_json::to_string(&(h, o, inv, features(&lib, &OverlaySet::default(), &s), spells(&lib, &s))).unwrap();
    }
}
