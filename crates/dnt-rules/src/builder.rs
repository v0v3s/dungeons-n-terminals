//! Turns a [`Character`] (choices) plus the library and overlays into a
//! [`Model`] ready to evaluate, along with the character's classes, active
//! features, resources, toggles and any warnings. Multiclassing is handled
//! throughout: nothing assumes a single class.

use std::collections::BTreeMap;

use dnt_data::{EntityId, Kind, Library};
use serde_json::Value as Json;

use crate::attacks;
use crate::character::Character;
use crate::class_table::{class_columns, slug, subclass_columns};
use crate::engine::{Effect, Evaluation, Model, Op, StatDef};
use crate::equipment;
use crate::formula::Formula;
use crate::multiclass::{self, Progression};
use crate::overlay::{resolve_id, OverlaySet, Resource, Toggle};
use crate::spells;
use crate::standard::{self, ABILITIES};
use crate::value::Value;

/// One of the character's classes.
#[derive(Debug, Clone)]
pub struct ClassState {
    pub id: String,
    pub entity: EntityId,
    pub name: String,
    pub slug: String,
    pub level: i64,
    pub subclass: Option<EntityId>,
    pub hit_die: i64,
    pub progression: Option<Progression>,
    /// Spellcasting ability from the class or its subclass.
    pub casting_ability: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeatureSource {
    /// Index into [`Build::classes`].
    Class(usize),
    Subclass(usize),
    Species,
    Background,
    Feat,
    Option,
    /// A weapon whose mastery property the character can use.
    Mastery,
}

#[derive(Debug, Clone)]
pub struct ActiveFeature {
    pub entity: EntityId,
    pub source: FeatureSource,
    /// Class level at which it was gained (0 for non-class features).
    pub level: i64,
}

#[derive(Debug, Clone)]
pub struct Build {
    pub model: Model,
    pub classes: Vec<ClassState>,
    pub features: Vec<ActiveFeature>,
    /// Resources from overlays, with the feature name they come from.
    pub resources: Vec<(Resource, String)>,
    pub toggles: Vec<(Toggle, String)>,
    /// Spells and spell choices granted by features.
    pub granted: Vec<GrantedSpells>,
    /// Weapon attacks (carried weapons and the unarmed strike).
    pub attacks: Vec<attacks::Attack>,
    pub weapon_proficiency: attacks::WeaponProficiency,
    /// The Monk's Martial Arts, if the character has it.
    pub martial_arts: Option<attacks::MartialArts>,
    pub warnings: Vec<String>,
}

/// Spells granted by one feature (subclass, species, feat, …).
#[derive(Debug, Clone)]
pub struct GrantedSpells {
    pub origin: String,
    /// Storage prefix for choices: `<origin id>#<n>` keys `Spells::chosen`.
    pub key: String,
    pub grants: Vec<spells::Granted>,
}

impl Build {
    pub fn evaluate(&self) -> Evaluation {
        self.model.evaluate()
    }

    /// `Fighter 5 / Wizard 3`.
    pub fn class_summary(&self) -> String {
        self.classes
            .iter()
            .map(|c| format!("{} {}", c.name, c.level))
            .collect::<Vec<_>>()
            .join(" / ")
    }
}

fn str_of<'a>(v: &'a Json, k: &str) -> Option<&'a str> {
    v.get(k).and_then(Json::as_str)
}

pub fn build(character: &Character, lib: &Library, overlays: &OverlaySet) -> Build {
    let mut warnings = Vec::new();
    let mut model = Model {
        stats: standard::stats(),
        inputs: standard::default_inputs(),
        ..Model::default()
    };
    let resolve = |id: &str, warnings: &mut Vec<String>| -> Option<EntityId> {
        let found = resolve_id(lib, id);
        if found.is_none() {
            warnings.push(format!("not in the library (or not enabled): {id}"));
        }
        found
    };

    // ---- abilities ----
    for (ab, _) in ABILITIES {
        model.inputs.insert(format!("base.{ab}"), Value::int(character.abilities.get(ab)));
    }
    for bonus in &character.ability_bonus {
        for (ab, n) in &bonus.scores {
            model.effects.push(Effect {
                target: ab.clone(),
                op: Op::Add,
                value: Formula::constant(Value::int(*n)),
                when: None,
                origin: bonus.origin.clone(),
            });
        }
    }

    // ---- per-level choices: ASIs, feats and feature picks ----
    let mut level_feats: Vec<String> = Vec::new();
    // (class id, pick value) for picks that become features.
    let mut level_picks: Vec<(String, String)> = Vec::new();
    let mut expertise: Vec<String> = Vec::new();
    let mut counts: BTreeMap<&str, i64> = BTreeMap::new();
    for l in &character.levels {
        let n = counts.entry(&l.class).or_default();
        *n += 1;
        // `class:Fighter|XPHB` → `Fighter`
        let class_name = l.class.split_once(':').map_or("", |(_, r)| r.split('|').next().unwrap_or(r));
        for (ab, v) in &l.choices.ability {
            model.effects.push(Effect {
                target: ab.clone(),
                op: Op::Add,
                value: Formula::constant(Value::int(*v)),
                when: None,
                origin: format!("Ability Score Improvement ({class_name} {n})"),
            });
        }
        level_feats.extend(l.choices.feat.iter().cloned());
        for (key, values) in &l.choices.picks {
            if key == "Expertise" {
                expertise.extend(values.iter().cloned());
            } else {
                level_picks.extend(values.iter().map(|v| (l.class.clone(), v.clone())));
            }
        }
    }

    // ---- classes ----
    let mut classes: Vec<ClassState> = Vec::new();
    for (id, level) in character.class_levels() {
        let Some(entity) = resolve(id, &mut warnings) else { continue };
        let data = &lib.entity(entity).data;
        let name = lib.entity(entity).name.clone();
        let subclass = character.subclasses.get(id).and_then(|s| resolve(s, &mut warnings));
        let sub_data = subclass.map(|s| &lib.entity(s).data);
        let progression = Progression::from_json(data.get("casterProgression"))
            .or_else(|| sub_data.and_then(|s| Progression::from_json(s.get("casterProgression"))));
        let casting_ability = str_of(data, "spellcastingAbility")
            .or_else(|| sub_data.and_then(|s| str_of(s, "spellcastingAbility")))
            .map(str::to_string);
        classes.push(ClassState {
            id: id.to_string(),
            entity,
            slug: slug(&name),
            name,
            level,
            subclass,
            hit_die: data.get("hd").and_then(|h| h.get("faces")).and_then(Json::as_i64).unwrap_or(8),
            progression,
            casting_ability,
        });
    }
    let total = character.total_level() as i64;
    model.inputs.insert("level".into(), Value::int(total));
    if total > 20 {
        warnings.push(format!("total level {total} is above 20"));
    }

    for c in &classes {
        model.class_levels.insert(c.name.to_lowercase(), c.level);
        model.inputs.insert(format!("class.{}.level", c.slug), Value::int(c.level));
        let mut columns = class_columns(&lib.entity(c.entity).data);
        if let Some(s) = c.subclass {
            columns.extend(subclass_columns(&lib.entity(s).data));
        }
        for col in columns {
            model.inputs.insert(col.var.clone(), col.at(c.level as usize));
        }
    }

    // ---- proficiencies: saves from the starting class only ----
    if let Some(first) = classes.first() {
        for ab in lib.entity(first.entity).data.get("proficiency").and_then(Json::as_array).into_iter().flatten() {
            if let Some(ab) = ab.as_str() {
                model.inputs.insert(format!("prof.save.{ab}"), Value::int(1));
            }
        }
    }
    let all_expertise: Vec<String> = character.proficiencies.expertise.iter().cloned().chain(expertise).collect();
    for (list, mult) in [
        (&character.proficiencies.half, standard::half()),
        (&character.proficiencies.skills, Value::int(1)),
        (&all_expertise, Value::int(2)),
    ] {
        for s in list {
            let key = slug(s);
            if standard::SKILLS.iter().any(|(k, _, _)| *k == key) {
                model.inputs.insert(format!("prof.skill.{key}"), mult.clone());
            } else {
                warnings.push(format!("unknown skill `{s}`"));
            }
        }
    }

    // ---- hit points and hit dice ----
    let mut hp = 0;
    let mut pools: BTreeMap<i64, i64> = BTreeMap::new();
    for (i, l) in character.levels.iter().enumerate() {
        let Some(c) = classes.iter().find(|c| c.id == l.class) else { continue };
        // The first level is always the maximum.
        hp += if i == 0 { c.hit_die } else { l.hp.amount(c.hit_die) };
        *pools.entry(c.hit_die).or_default() += 1;
    }
    model.inputs.insert("hp.base".into(), Value::int(hp));
    for (faces, n) in &pools {
        model.stats.push(StatDef {
            key: format!("hit_dice.d{faces}.max"),
            base: Formula::constant(Value::int(*n)),
            origin: "class levels".into(),
        });
    }

    // ---- spellcasting ----
    for c in &classes {
        if let Some(ab) = &c.casting_ability {
            let s = &c.slug;
            model.stats.push(StatDef::new(&format!("spell.{s}.mod"), &format!("mod.{ab}"), "spellcasting ability"));
            model.stats.push(StatDef::new(&format!("spell.{s}.attack"), &format!("spell.{s}.mod + prof"), "ability + proficiency"));
            model.stats.push(StatDef::new(&format!("spell.{s}.dc"), &format!("8 + spell.{s}.mod + prof"), "8 + ability + proficiency"));
        }
    }
    let slot_classes: Vec<&ClassState> = classes
        .iter()
        .filter(|c| c.progression.is_some_and(|p| p != Progression::Pact))
        .collect();
    let slots: [i64; 9] = match slot_classes.as_slice() {
        [] => [0; 9],
        // One slot-granting class: its own table (third-casters need this).
        [only] => {
            let own: Vec<i64> = (1..=9)
                .map(|n| match model.inputs.get(&format!("class.{}.slots.{n}", only.slug)) {
                    Some(Value::Num(v)) => v.floor(),
                    _ => -1,
                })
                .collect();
            if own.iter().all(|n| *n < 0) {
                let cl = only.progression.unwrap().caster_levels(only.level);
                multiclass::slots_for(cl).map(i64::from)
            } else {
                std::array::from_fn(|i| own[i].max(0))
            }
        }
        many => {
            let cl = multiclass::caster_level(&many.iter().map(|c| (c.progression.unwrap(), c.level)).collect::<Vec<_>>());
            multiclass::slots_for(cl).map(i64::from)
        }
    };
    for (i, n) in slots.iter().enumerate() {
        if *n > 0 {
            model.stats.push(StatDef {
                key: format!("slots.{}.max", i + 1),
                base: Formula::constant(Value::int(*n)),
                origin: "spellcasting".into(),
            });
        }
    }
    for c in classes.iter().filter(|c| c.progression == Some(Progression::Pact)) {
        let get = |col: &str| match model.inputs.get(&format!("class.{}.{col}", c.slug)) {
            Some(Value::Num(n)) => n.floor(),
            _ => 0,
        };
        let (count, level) = (get("spell_slots"), get("slot_level"));
        if count > 0 {
            model.stats.push(StatDef {
                key: "pact.slots.max".into(),
                base: Formula::constant(Value::int(count)),
                origin: format!("{} Pact Magic", c.name),
            });
            model.inputs.insert("pact.level".into(), Value::int(level));
        }
    }

    // ---- equipment: armor, shield and item bonuses ----
    // Worn and held inventory lines win over the older `equipped` fields;
    // a shield counts while in hand.
    let equipped: Vec<(EntityId, bool)> = character
        .inventory
        .iter()
        .filter(|l| l.equipped || l.hand.is_some())
        .filter_map(|l| Some((resolve_id(lib, l.item.as_deref()?)?, l.attuned)))
        .collect();
    let armor_id = equipped
        .iter()
        .map(|(e, _)| *e)
        .find(|e| matches!(equipment::armor_kind(&lib.entity(*e).data), Some("light" | "medium" | "heavy")))
        .or_else(|| character.equipped.armor.as_deref().and_then(|a| resolve(a, &mut warnings)));
    let shield_id = equipped
        .iter()
        .map(|(e, _)| *e)
        .find(|e| equipment::armor_kind(&lib.entity(*e).data) == Some("shield"));
    let bonus = |e: EntityId, field: &str| -> i64 {
        let d = &lib.entity(e).data;
        d.get(field)
            .or_else(|| d.get("inherits").and_then(|i| i.get(field)))
            .and_then(Json::as_str)
            .and_then(|b| b.trim_start_matches('+').parse::<i64>().ok())
            .unwrap_or(0)
    };
    if let Some(armor) = armor_id {
        let d = &lib.entity(armor).data;
        match equipment::armor_kind(d) {
            Some(cat @ ("light" | "medium" | "heavy")) => {
                model.inputs.insert("armor.none".into(), Value::Bool(false));
                model.inputs.insert(format!("armor.{cat}"), Value::Bool(true));
                let base = d.get("inherits").unwrap_or(d);
                let ac = base.get("ac").and_then(Json::as_i64).unwrap_or(10);
                model.inputs.insert("armor.base".into(), Value::int(ac + bonus(armor, "bonusAc")));
            }
            _ => warnings.push(format!("{} is not armor", lib.entity(armor).name)),
        }
    }
    match shield_id {
        Some(s) => {
            let d = &lib.entity(s).data;
            let ac = d.get("ac").or_else(|| d.get("inherits").and_then(|i| i.get("ac"))).and_then(Json::as_i64).unwrap_or(2);
            model.inputs.insert("armor.shield".into(), Value::int(ac + bonus(s, "bonusAc")));
        }
        None if character.equipped.shield => {
            model.inputs.insert("armor.shield".into(), Value::int(2));
        }
        None => {}
    }
    // Other worn magic items (rings, cloaks, …); attunement items only
    // work while attuned.
    for (e, attuned) in &equipped {
        let d = &lib.entity(*e).data;
        if equipment::armor_kind(d).is_some() || (equipment::requires_attunement(d) && !attuned) {
            continue;
        }
        let name = lib.entity(*e).name.clone();
        let mut add = |target: String, n: i64| {
            model.effects.push(Effect {
                target,
                op: Op::Add,
                value: Formula::constant(Value::int(n)),
                when: None,
                origin: name.clone(),
            });
        };
        let (ac, saves) = (bonus(*e, "bonusAc"), bonus(*e, "bonusSavingThrow"));
        if ac != 0 {
            add("ac".into(), ac);
        }
        if saves != 0 {
            for (ab, _) in ABILITIES {
                add(format!("save.{ab}"), saves);
            }
        }
    }
    let attuned = character.inventory.iter().filter(|l| l.attuned).count();
    if attuned > 3 {
        warnings.push(format!("attuned to {attuned} items (the limit is 3)"));
    }

    // ---- active features ----
    let mut features: Vec<ActiveFeature> = Vec::new();
    for (ci, c) in classes.iter().enumerate() {
        let data = &lib.entity(c.entity).data;
        for f in data.get("classFeatures").and_then(Json::as_array).into_iter().flatten() {
            let Some(r) = f.as_str().or_else(|| str_of(f, "classFeature")) else { continue };
            let level: i64 = r.split('|').nth(3).and_then(|l| l.parse().ok()).unwrap_or(1);
            if level > c.level {
                continue;
            }
            if let Some(e) = lib.resolve_feature_ref(Kind::ClassFeature, r) {
                features.push(ActiveFeature { entity: e, source: FeatureSource::Class(ci), level });
            }
        }
        if let Some(s) = c.subclass {
            for r in lib.entity(s).data.get("subclassFeatures").and_then(Json::as_array).into_iter().flatten() {
                let Some(r) = r.as_str() else { continue };
                let level: i64 = r.split('|').nth(5).and_then(|l| l.parse().ok()).unwrap_or(1);
                if level > c.level {
                    continue;
                }
                if let Some(e) = lib.resolve_feature_ref(Kind::SubclassFeature, r) {
                    features.push(ActiveFeature { entity: e, source: FeatureSource::Subclass(ci), level });
                }
            }
        }
    }
    // Features nested inside other features (`refClassFeature` /
    // `refSubclassFeature` entries) are active too.
    let mut i = 0;
    while i < features.len() {
        let parent = features[i].clone();
        let mut nested = Vec::new();
        collect_refs(&lib.entity(parent.entity).data, &mut nested);
        for (kind, r) in nested {
            if let Some(e) = lib.resolve_feature_ref(kind, &r) {
                if !features.iter().any(|f| f.entity == e) {
                    let source = match (kind, parent.source) {
                        (Kind::SubclassFeature, FeatureSource::Class(ci)) => FeatureSource::Subclass(ci),
                        (_, s) => s,
                    };
                    features.push(ActiveFeature { entity: e, source, level: parent.level });
                }
            }
        }
        i += 1;
    }
    let singles = [
        (character.species.as_deref(), FeatureSource::Species),
        (character.subspecies.as_deref(), FeatureSource::Species),
        (character.background.as_deref(), FeatureSource::Background),
    ];
    for (id, source) in singles {
        if let Some(e) = id.and_then(|id| resolve(id, &mut warnings)) {
            features.push(ActiveFeature { entity: e, source, level: 0 });
            if source == FeatureSource::Species {
                if let Some(speed) = species_speed(&lib.entity(e).data) {
                    model.inputs.insert("speed.base".into(), Value::int(speed));
                }
            }
        }
    }
    // Picked sub-features join their class's features; feats, optional
    // features and weapon masteries get their own groups.
    for (class_id, value) in &level_picks {
        let Some(e) = resolve_id(lib, value) else {
            warnings.push(format!("not in the library (or not enabled): {value}"));
            continue;
        };
        let ci = classes.iter().position(|c| &c.id == class_id);
        let source = match (lib.entity(e).kind, ci) {
            (Kind::ClassFeature, Some(ci)) => FeatureSource::Class(ci),
            (Kind::SubclassFeature, Some(ci)) => FeatureSource::Subclass(ci),
            (Kind::Feat, _) => FeatureSource::Feat,
            (Kind::BaseItem | Kind::Item, _) => FeatureSource::Mastery,
            _ => FeatureSource::Option,
        };
        let level = lib.entity(e).data.get("level").and_then(Json::as_i64).unwrap_or(0);
        features.push(ActiveFeature { entity: e, source, level });
    }
    let all_feats: Vec<String> = character.feats.iter().cloned().chain(level_feats).collect();
    for (list, source) in [(&all_feats, FeatureSource::Feat), (&character.options, FeatureSource::Option)] {
        for id in list {
            if let Some(e) = resolve(id, &mut warnings) {
                features.push(ActiveFeature { entity: e, source, level: 0 });
            }
        }
    }

    // Group features: each class then its subclass (by level), then
    // species, background, feats, options and weapon masteries.
    let rank = |s: FeatureSource| match s {
        FeatureSource::Class(ci) => ci * 2,
        FeatureSource::Subclass(ci) => ci * 2 + 1,
        FeatureSource::Species => 10_000,
        FeatureSource::Background => 10_001,
        FeatureSource::Feat => 10_002,
        FeatureSource::Option => 10_003,
        FeatureSource::Mastery => 10_004,
    };
    features.sort_by_key(|f| (rank(f.source), f.level));

    // ---- spells granted by features ----
    let mut granted = Vec::new();
    let mut sources: Vec<(EntityId, i64)> = classes.iter().filter_map(|c| Some((c.subclass?, c.level))).collect();
    for f in &features {
        if matches!(f.source, FeatureSource::Species | FeatureSource::Background | FeatureSource::Feat | FeatureSource::Option) {
            sources.push((f.entity, total));
        }
    }
    let mut sets: Vec<(String, String, Vec<spells::Granted>)> = Vec::new();
    for (e, level) in sources {
        let grants = spells::granted_spells(&lib.entity(e).data, level);
        if !grants.is_empty() {
            let ent = lib.entity(e);
            sets.push((ent.name.clone(), format!("{}:{}|{}", ent.kind.json_key(), ent.name, ent.source), grants));
        }
    }
    // Mystic Arcanum (Warlock 11, 13, 15, 17): a level 6–9 Warlock spell
    // each, cast once per long rest without a slot.
    let mut arcana: Vec<i64> = features
        .iter()
        .map(|f| lib.entity(f.entity))
        .filter(|e| e.kind == Kind::ClassFeature && e.name.starts_with("Mystic Arcanum"))
        .filter(|e| e.data.get("className").and_then(Json::as_str) == Some("Warlock"))
        .filter_map(|e| e.data.get("level").and_then(Json::as_i64))
        .filter(|l| *l >= 11)
        .map(|l| 6 + (l - 11) / 2)
        .collect();
    arcana.sort();
    arcana.dedup();
    if !arcana.is_empty() {
        let grants = arcana
            .iter()
            .map(|lvl| spells::Granted::Choice {
                filter: format!("level={lvl}|class=Warlock"),
                count: 1,
                grant: spells::Grant::Innate(Some(1)),
            })
            .collect();
        sets.push(("Mystic Arcanum".into(), "classFeature:Mystic Arcanum|Warlock".into(), grants));
    }
    let mut innate: Vec<(Resource, String)> = Vec::new();
    for (origin, key, grants) in sets {
        for (i, g) in grants.iter().enumerate() {
            // Innate casts per long rest become trackable resources.
            let (ids, uses) = match g {
                spells::Granted::Spell { id, grant: spells::Grant::Innate(Some(n)) } => (vec![id.clone()], *n),
                spells::Granted::Choice { grant: spells::Grant::Innate(Some(n)), .. } => {
                    (character.spells.chosen.get(&format!("{key}#{i}")).cloned().unwrap_or_default(), *n)
                }
                _ => continue,
            };
            for id in ids {
                let Some(s) = resolve_id(lib, &id) else { continue };
                let name = lib.entity(s).name.clone();
                innate.push((
                    Resource {
                        key: format!("innate_{}", slug(&name)),
                        name: format!("{name} (free cast)"),
                        max: Formula::constant(Value::int(uses)),
                        recharge: crate::overlay::Recharge::LongRest,
                        short_rest_regain: None,
                    },
                    origin.clone(),
                ));
            }
        }
        granted.push(GrantedSpells { origin, key, grants });
    }
    for (r, _) in &innate {
        model.stats.push(StatDef {
            key: format!("resource.{}.max", r.key),
            base: r.max.clone(),
            origin: r.name.clone(),
        });
    }

    // ---- overlays ----
    let mut resources = innate;
    let mut toggles = Vec::new();
    for f in &features {
        for o in overlays.for_entity(&lib.entity(f.entity).key) {
            o.apply(&mut model);
            resources.extend(o.resources.iter().map(|r| (r.clone(), o.name.clone())));
            toggles.extend(o.toggles.iter().map(|t| (t.clone(), o.name.clone())));
        }
    }

    // ---- conditions and exhaustion (before attacks, which copy attack.bonus) ----
    let rules_2024 = classes.first().is_none_or(|c| lib.entity(c.entity).source == "XPHB");
    model.stats.push(StatDef::new("d20.penalty", "0", "no exhaustion"));
    let state = &character.state;
    model.effects.extend(crate::conditions::effects(&state.conditions, state.exhaustion, rules_2024));
    if rules_2024 && state.exhaustion > 0 {
        for c in &classes {
            if c.casting_ability.is_some() {
                model.effects.push(Effect {
                    target: format!("spell.{}.attack", c.slug),
                    op: Op::Add,
                    value: Formula::constant(Value::int(-2 * state.exhaustion.min(6))),
                    when: None,
                    origin: format!("Exhaustion {}", state.exhaustion.min(6)),
                });
            }
        }
    }

    // ---- attacks (after overlays: their attack effects are copied) ----
    let class_entities: Vec<EntityId> = classes.iter().map(|c| c.entity).collect();
    let proficiency_sources: Vec<EntityId> = features
        .iter()
        .filter(|f| matches!(f.source, FeatureSource::Species | FeatureSource::Background | FeatureSource::Feat))
        .map(|f| f.entity)
        .collect();
    let weapon_proficiency = attacks::weapon_proficiency(lib, character, &class_entities, &proficiency_sources);
    let mastered: Vec<EntityId> = features.iter().filter(|f| f.source == FeatureSource::Mastery).map(|f| f.entity).collect();
    let martial_arts = features.iter().find_map(|f| {
        let e = lib.entity(f.entity);
        let monk = e.data.get("className").and_then(Json::as_str) == Some("Monk");
        (e.kind == Kind::ClassFeature && monk && e.name == "Martial Arts").then(|| attacks::MartialArts { rules_2024: e.source == "XPHB" })
    });
    let dual_wielder = features.iter().find_map(|f| {
        let e = lib.entity(f.entity);
        (e.kind == Kind::Feat && e.name == "Dual Wielder").then(|| attacks::DualWielder { rules_2024: e.source == "XPHB" })
    });
    let great_weapon = features.iter().find_map(|f| {
        let e = lib.entity(f.entity);
        // A 2024 feat, or a 2014 fighting style (an optional feature).
        (matches!(e.kind, Kind::Feat | Kind::OptionalFeature) && e.name == "Great Weapon Fighting").then(|| e.source == "XPHB")
    });
    let found = attacks::AttackFeatures { martial_arts, dual_wielder, great_weapon };
    let attacks = attacks::add_attacks(&mut model, lib, character, &weapon_proficiency, &mastered, found);

    // ---- state that feeds stats, and overrides ----
    for (k, on) in &character.state.toggles {
        model.inputs.insert(format!("toggle.{k}"), Value::Bool(*on));
    }
    for (k, src) in &character.overrides {
        match Formula::parse(src).and_then(|f| f.eval(&model.inputs)) {
            Ok(v) => {
                model.overrides.insert(k.clone(), v);
            }
            Err(e) => warnings.push(format!("override `{k}`: {}", e.message)),
        }
    }

    // ---- multiclass requirements ----
    if classes.len() > 1 {
        let ev = model.evaluate();
        let score = |ab: &str| ev.int(ab).unwrap_or(0);
        for c in &classes {
            let reqs = multiclass::requirements(&lib.entity(c.entity).data);
            if !reqs.is_empty() && !reqs.iter().any(|r| r.met(&score)) {
                let need = reqs.iter().map(|r| r.describe()).collect::<Vec<_>>().join("; or ");
                warnings.push(format!("multiclassing {} requires {need}", c.name));
            }
        }
    }

    Build {
        model,
        classes,
        features,
        resources,
        toggles,
        granted,
        attacks,
        weapon_proficiency,
        martial_arts,
        warnings,
    }
}

fn collect_refs(v: &Json, out: &mut Vec<(Kind, String)>) {
    match v {
        Json::Array(a) => a.iter().for_each(|x| collect_refs(x, out)),
        Json::Object(o) => {
            match o.get("type").and_then(Json::as_str) {
                // Options are chosen, not all granted; picks are added separately.
                Some("options") => return,
                Some("refClassFeature") => {
                    if let Some(r) = o.get("classFeature").and_then(Json::as_str) {
                        out.push((Kind::ClassFeature, r.to_string()));
                    }
                }
                Some("refSubclassFeature") => {
                    if let Some(r) = o.get("subclassFeature").and_then(Json::as_str) {
                        out.push((Kind::SubclassFeature, r.to_string()));
                    }
                }
                _ => {}
            }
            if let Some(e) = o.get("entries") {
                collect_refs(e, out);
            }
        }
        _ => {}
    }
}

fn species_speed(d: &Json) -> Option<i64> {
    match d.get("speed")? {
        Json::Number(n) => n.as_i64(),
        Json::Object(o) => o.get("walk").and_then(Json::as_i64),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::{AbilityBonus, HitPoints, Level};
    use crate::overlay::Overlay;
    use serde_json::json;

    fn slots_rows(rows: &[[i64; 2]]) -> Json {
        json!(rows.iter().map(|r| r.to_vec()).collect::<Vec<_>>())
    }

    fn library() -> Library {
        let mut wizard_rows = Vec::new();
        for l in 1..=20 {
            let s = multiclass::slots_for(l);
            wizard_rows.push([s[0] as i64, s[1] as i64]);
        }
        let mut lib = Library::new();
        lib.load_json(&json!({
            "class": [
                {"name": "Wizard", "source": "T", "hd": {"faces": 6}, "proficiency": ["int", "wis"],
                 "casterProgression": "full", "spellcastingAbility": "int",
                 "primaryAbility": [{"int": true}],
                 "classTableGroups": [{"title": "Spell Slots per Spell Level", "colLabels": ["1st", "2nd"],
                                       "rowsSpellProgression": slots_rows(&wizard_rows)}],
                 "classFeatures": ["Arcane Recovery|Wizard|T|1", "Scholar|Wizard|T|2"]},
                {"name": "Fighter", "source": "T", "hd": {"faces": 10}, "proficiency": ["str", "con"],
                 "multiclassing": {"requirements": {"or": [{"str": 13, "dex": 13}]}},
                 "classFeatures": ["Second Wind|Fighter|T|1", {"classFeature": "Fighter Subclass|Fighter|T|3", "gainSubclassFeature": true}]},
                {"name": "Warlock", "source": "T", "hd": {"faces": 8}, "proficiency": ["wis", "cha"],
                 "casterProgression": "pact", "spellcastingAbility": "cha",
                 "classTableGroups": [{"colLabels": ["Spell Slots", "Slot Level"],
                                       "rows": [[1, "1st"], [2, "1st"], [2, "2nd"]]}],
                 "classFeatures": ["Mystic Arcanum|Warlock|T|11", "Mystic Arcanum|Warlock|T|13"]}
            ],
            "subclass": [
                {"name": "Eldritch Knight", "shortName": "Eldritch Knight", "source": "T", "className": "Fighter", "classSource": "T",
                 "casterProgression": "1/3", "spellcastingAbility": "int",
                 "subclassTableGroups": [{"title": "Spell Slots per Spell Level", "colLabels": ["1st", "2nd"],
                     "rowsSpellProgression": [[0,0],[0,0],[2,0],[3,0],[3,0],[3,0],[4,2]]}],
                 "subclassFeatures": ["Eldritch Knight|Fighter|T|Eldritch Knight|T|3"]}
            ],
            "classFeature": [
                {"name": "Arcane Recovery", "source": "T", "className": "Wizard", "classSource": "T", "level": 1, "entries": ["x"]},
                {"name": "Scholar", "source": "T", "className": "Wizard", "classSource": "T", "level": 2, "entries": ["x"]},
                {"name": "Second Wind", "source": "T", "className": "Fighter", "classSource": "T", "level": 1, "entries": ["x"]},
                {"name": "Fighter Subclass", "source": "T", "className": "Fighter", "classSource": "T", "level": 3, "entries": ["x"]},
                {"name": "Mystic Arcanum", "source": "T", "className": "Warlock", "classSource": "T", "level": 11, "entries": ["x"]},
                {"name": "Mystic Arcanum", "source": "T", "className": "Warlock", "classSource": "T", "level": 13, "entries": ["x"]}
            ],
            "spell": [{"name": "Eyebite", "source": "T", "level": 6, "entries": ["x"]}],
            "subclassFeature": [
                {"name": "Eldritch Knight", "source": "T", "className": "Fighter", "classSource": "T",
                 "subclassShortName": "Eldritch Knight", "subclassSource": "T", "level": 3,
                 "entries": [{"type": "refSubclassFeature", "subclassFeature": "War Bond|Fighter|T|Eldritch Knight|T|3"}]},
                {"name": "War Bond", "source": "T", "className": "Fighter", "classSource": "T",
                 "subclassShortName": "Eldritch Knight", "subclassSource": "T", "level": 3, "entries": ["x"]}
            ]
        }));
        lib
    }

    fn character(levels: &[&str]) -> Character {
        let mut c = Character {
            name: "Test".into(),
            ..Character::default()
        };
        c.abilities.str = 14;
        c.abilities.con = 14;
        c.abilities.int = 16;
        c.levels = levels
            .iter()
            .map(|cls| Level {
                class: format!("class:{cls}|T"),
                hp: Default::default(),
                choices: Default::default(),
            })
            .collect();
        c
    }

    fn eldritch_knight(c: &mut Character) {
        c.subclasses.insert("class:Fighter|T".into(), "subclass:Eldritch Knight|Fighter|T|T".into());
    }

    #[test]
    fn single_third_caster_uses_its_own_table() {
        let mut c = character(&["Fighter"; 4]);
        eldritch_knight(&mut c);
        let lib = library();
        let b = build(&c, &lib, &OverlaySet::default());
        assert!(b.warnings.is_empty(), "{:?}", b.warnings);
        let ev = b.evaluate();
        // The multiclass formula would give 2 (floor(4/3) = 1); the EK table says 3.
        assert_eq!(ev.int("slots.1.max"), Some(3));
        assert_eq!(ev.int("spell.fighter.dc"), Some(8 + 3 + 2));
        let names: Vec<&str> = b.features.iter().map(|f| lib.entity(f.entity).name.as_str()).collect();
        assert!(names.contains(&"War Bond"), "nested subclass features are active: {names:?}");
    }

    #[test]
    fn multiclass_slots_combine_caster_levels() {
        let mut c = character(&["Wizard", "Wizard", "Wizard", "Fighter", "Fighter", "Fighter"]);
        eldritch_knight(&mut c);
        let b = build(&c, &library(), &OverlaySet::default());
        assert_eq!(b.class_summary(), "Wizard 3 / Fighter 3");
        let ev = b.evaluate();
        // Caster level 3 + floor(3/3) = 4 → 4/3 slots.
        assert_eq!(ev.int("slots.1.max"), Some(4));
        assert_eq!(ev.int("slots.2.max"), Some(3));
        // One spell DC per casting class.
        assert!(ev.get("spell.wizard.dc").is_some() && ev.get("spell.fighter.dc").is_some());
    }

    #[test]
    fn pact_magic_stays_separate() {
        let c = character(&["Wizard", "Warlock", "Warlock", "Warlock"]);
        let b = build(&c, &library(), &OverlaySet::default());
        let ev = b.evaluate();
        assert_eq!(ev.int("slots.1.max"), Some(2), "wizard 1 alone");
        assert_eq!(ev.int("pact.slots.max"), Some(2));
        assert_eq!(b.model.inputs["pact.level"], Value::int(2));
    }

    #[test]
    fn mystic_arcanum() {
        let lib = library();
        let mut c = character(&["Warlock"; 12]);
        let b = build(&c, &lib, &OverlaySet::default());
        let arcanum = b.granted.iter().find(|g| g.origin == "Mystic Arcanum").expect("level 11 arcanum");
        assert_eq!(arcanum.grants.len(), 1);
        assert!(matches!(&arcanum.grants[0], spells::Granted::Choice { filter, grant: spells::Grant::Innate(Some(1)), .. } if filter == "level=6|class=Warlock"));
        // A chosen arcanum is a free cast per long rest.
        c.spells.chosen.insert(format!("{}#0", arcanum.key), vec!["spell:Eyebite|T".into()]);
        let b = build(&c, &lib, &OverlaySet::default());
        assert!(b.resources.iter().any(|(r, _)| r.key == "innate_eyebite"));
        let c = character(&["Warlock"; 13]);
        let b = build(&c, &lib, &OverlaySet::default());
        assert_eq!(b.granted.iter().find(|g| g.origin == "Mystic Arcanum").unwrap().grants.len(), 2);
        assert!(build(&character(&["Warlock"; 3]), &lib, &OverlaySet::default()).granted.iter().all(|g| g.origin != "Mystic Arcanum"));
    }

    #[test]
    fn hp_saves_and_hit_dice() {
        let mut c = character(&["Fighter", "Wizard", "Wizard"]);
        c.levels[1].hp = HitPoints::Rolled(2);
        let b = build(&c, &library(), &OverlaySet::default());
        let ev = b.evaluate();
        // 10 (max d10) + 2 (rolled) + 4 (average d6) + 3 × Con 2.
        assert_eq!(ev.int("hp.max"), Some(22));
        assert_eq!(ev.int("save.str"), Some(2 + 2), "starting class save");
        assert_eq!(ev.int("save.int"), Some(3), "no save from a later class");
        assert_eq!(ev.int("hit_dice.d10.max"), Some(1));
        assert_eq!(ev.int("hit_dice.d6.max"), Some(2));
    }

    #[test]
    fn requirements_warn_but_build() {
        let mut c = character(&["Wizard", "Fighter"]);
        c.abilities.str = 8;
        c.abilities.int = 12;
        let b = build(&c, &library(), &OverlaySet::default());
        assert!(b.warnings.iter().any(|w| w.contains("Fighter requires Strength 13 or Dexterity 13")), "{:?}", b.warnings);
        assert!(b.warnings.iter().any(|w| w.contains("Wizard requires Intelligence 13")));
        // A +1 Int bonus fixes the Wizard requirement.
        c.ability_bonus.push(AbilityBonus {
            origin: "Background".into(),
            scores: [("int".to_string(), 1)].into(),
        });
        let b = build(&c, &library(), &OverlaySet::default());
        assert!(!b.warnings.iter().any(|w| w.contains("Wizard requires")));
    }

    #[test]
    fn level_choices_apply_and_undo() {
        let mut c = character(&["Fighter", "Fighter", "Fighter", "Fighter"]);
        c.levels[3].choices.ability.insert("str".into(), 2);
        let lib = library();
        let ev = build(&c, &lib, &OverlaySet::default()).evaluate();
        assert_eq!(ev.int("str"), Some(16));
        assert!(ev.stats["str"].explain().contains("Ability Score Improvement (Fighter 4)"));

        eldritch_knight(&mut c);
        for _ in 0..2 {
            c.remove_last_level();
        }
        assert!(c.subclasses.contains_key("class:Fighter|T"), "still has Fighter levels");
        let ev = build(&c, &lib, &OverlaySet::default()).evaluate();
        assert_eq!(ev.int("str"), Some(14), "the ASI left with its level");
        c.remove_last_level();
        c.remove_last_level();
        assert!(c.subclasses.is_empty());
        assert!(c.remove_last_level().is_none());
    }

    #[test]
    fn inventory_armor_shield_and_attunement() {
        use crate::character::InventoryItem;
        let mut lib = library();
        lib.load_json(&json!({
            "baseitem": [
                {"name": "Chain Mail", "source": "T", "type": "HA", "ac": 16},
                {"name": "Shield", "source": "T", "type": "S", "ac": 2}
            ],
            "item": [{"name": "Ring of Protection", "source": "T", "bonusAc": "+1", "bonusSavingThrow": "+1", "reqAttune": true}]
        }));
        let line = |id: &str, attuned: bool| InventoryItem {
            item: Some(id.into()),
            quantity: 1,
            equipped: true,
            attuned,
            ..Default::default()
        };
        let mut c = character(&["Fighter"]);
        c.inventory = vec![line("item:Chain Mail|T", false), line("item:Shield|T", false), line("item:Ring of Protection|T", false)];
        let ev = build(&c, &lib, &OverlaySet::default()).evaluate();
        assert_eq!(ev.int("ac"), Some(18), "the ring needs attunement");
        c.inventory[2].attuned = true;
        let ev = build(&c, &lib, &OverlaySet::default()).evaluate();
        assert_eq!(ev.int("ac"), Some(19));
        assert_eq!(ev.int("save.wis"), Some(1));
        c.inventory[0].equipped = false;
        assert_eq!(build(&c, &lib, &OverlaySet::default()).evaluate().int("ac"), Some(10 + 2 + 1), "unarmored + shield + ring");
    }

    #[test]
    fn overlays_attach_to_active_features() {
        let (overlay, _) = Overlay::parse(
            r#"
[[feature]]
id = "classFeature:Second Wind|Fighter|T|1"
[[feature.resource]]
key = "second_wind"
name = "Second Wind"
max = "scale(level('Fighter'), {1: 2, 4: 3})"
recharge = "long_rest"
"#,
        )
        .unwrap();
        let mut set = OverlaySet::default();
        set.add(overlay);
        let b = build(&character(&["Wizard", "Fighter", "Fighter", "Fighter", "Fighter"]), &library(), &set);
        assert_eq!(b.resources.len(), 1);
        assert_eq!(b.evaluate().int("resource.second_wind.max"), Some(3), "uses the Fighter level, not total");
    }
}
