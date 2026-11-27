//! A character's companions, for the frontends: each one's card (its
//! numbers, rolls and state), and what the character has that gives
//! companions (a familiar spell, a class's beast) with the creatures each
//! may be.

use dnt_data::{creature, EntityId, Library};
use dnt_rules::companion;
use dnt_rules::overlay::{resolve_id, OverlaySet};
use dnt_rules::standard::{ABILITIES, SKILLS};
use serde::Serialize;

use crate::session::CharacterSession;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Companions {
    pub list: Vec<Card>,
    /// What gives companions, and what each may be.
    pub givers: Vec<Giver>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Card {
    pub index: usize,
    /// Its own name, or the creature's.
    pub name: String,
    /// The creature's name and id (for its stat block).
    pub creature: String,
    pub creature_id: String,
    /// Its creature, to show its stat block.
    pub entity: Option<EntityId>,
    /// `Familiar`, or `Follower` for one nothing gives.
    pub kind: String,
    /// `Tiny Fey, Unaligned`.
    pub type_line: String,
    pub away: bool,
    /// `None`: its creature is not in the data.
    pub numbers: Option<Numbers>,
    pub conditions: Vec<String>,
    pub notes: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Numbers {
    pub hp: i64,
    pub hp_max: i64,
    pub temp_hp: i64,
    pub ac: i64,
    pub initiative: i64,
    /// `40 ft., Fly 60 ft.`
    pub speed: String,
    pub abilities: Vec<Ability>,
    /// The skills its stat block has (or its rules give).
    pub skills: Vec<Skill>,
    pub passive_perception: i64,
    pub senses: String,
    pub languages: String,
    pub cr: Option<String>,
    pub actions: Vec<Action>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Ability {
    pub key: String,
    pub score: i64,
    pub modifier: i64,
    pub save: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Skill {
    pub key: String,
    pub name: String,
    pub bonus: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Action {
    pub name: String,
    /// `Action`, `Bonus Action`, …, or the rule's kind.
    pub section: String,
    pub attack: Option<i64>,
    /// The save's ability and DC.
    pub save: Option<String>,
    pub dc: Option<i64>,
    /// `2d4 + 2 piercing`.
    pub damage: Option<String>,
    pub heal: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, serde::Deserialize)]
pub struct Giver {
    /// `spell:Find Familiar|XPHB`.
    pub id: String,
    pub name: String,
    pub kind: String,
    /// The creatures it may be, by name: `(id, name, cr)`.
    pub creatures: Vec<(String, String, String)>,
    /// Creature types to choose from instead of its own.
    pub types: Vec<String>,
    /// How many at once, and how many there are.
    pub most: i64,
    pub have: i64,
    /// A spell's level, when the companion depends on it: what it is cast
    /// at, at least.
    pub spell_level: Option<i64>,
}

/// Creatures to pick for a companion: what `giver` (a rule's id) allows,
/// or any creature the campaign has (a follower).
pub fn creature_choices(s: &CharacterSession, lib: &Library, overlays: &OverlaySet, giver: Option<&str>) -> Vec<crate::options::Choice> {
    let choice = |id: EntityId| {
        let e = lib.entity(id);
        let cr = creature::challenge(&e.data).map_or(String::new(), |cr| format!("CR {cr} · "));
        crate::options::Choice::new(
            e.name.clone(),
            format!("{cr}{} · {}", creature::creature_type(&e.data), e.source),
            format!("creature:{}|{}", e.name, e.source),
        )
    };
    match giver {
        Some(g) => resolve_id(lib, g)
            .map(|e| gives(s, lib, overlays, e))
            .unwrap_or_default()
            .into_iter()
            .flat_map(|x| x.creatures)
            .filter_map(|(id, _, _)| resolve_id(lib, &id))
            .map(choice)
            .collect(),
        None => {
            let mut ids: Vec<EntityId> = (0..lib.entities().len())
                .filter(|&i| lib.entity(i).kind == dnt_data::Kind::Creature && s.filter.allows(lib.entity(i)))
                .collect();
            ids.sort_by(|a, b| lib.entity(*a).name.cmp(&lib.entity(*b).name));
            ids.into_iter().map(choice).collect()
        }
    }
}

pub fn companions(s: &CharacterSession, lib: &Library, overlays: &OverlaySet) -> Companions {
    let c = s.character();
    let list = c
        .companions
        .iter()
        .enumerate()
        .map(|(index, comp)| {
            let entity = resolve_id(lib, &comp.creature);
            let data = entity.map(|id| &lib.entity(id).data);
            let rules = companion::rules_for(lib, overlays, comp);
            let kind = rules.first().map_or("Follower".to_string(), |r| r.kind.clone());
            let creature_name = entity.map_or_else(|| comp.creature.clone(), |id| lib.entity(id).name.clone());
            let mut type_line = data.map(creature::kind_line).unwrap_or_default();
            if let (Some(t), Some(d)) = (&comp.creature_type, data) {
                type_line = type_line.replacen(&creature::creature_type(d), &dnt_data::view::title_case(t), 1);
            }
            let numbers = s.companions.get(index).and_then(|x| x.as_ref()).map(|(sheet, ev)| numbers(lib, sheet, ev, comp));
            Card {
                index,
                name: if comp.name.is_empty() { creature_name.clone() } else { comp.name.clone() },
                creature: creature_name,
                creature_id: comp.creature.clone(),
                entity,
                kind,
                type_line,
                away: comp.away,
                numbers,
                conditions: comp.conditions.clone(),
                notes: comp.notes.clone(),
            }
        })
        .collect();
    Companions { list, givers: givers(s, lib, overlays) }
}

fn numbers(lib: &Library, sheet: &companion::Sheet, ev: &dnt_rules::Evaluation, comp: &dnt_rules::character::Companion) -> Numbers {
    let d = &lib.entity(sheet.creature).data;
    let int = |k: &str| ev.int(k).unwrap_or(0);
    let hp_max = int("hp.max").max(1);
    let mut speed = vec![format!("{} ft.", int("speed"))];
    for mode in ["burrow", "climb", "fly", "swim"] {
        if let Some(n) = ev.int(&format!("speed.{mode}")).filter(|n| *n > 0) {
            speed.push(format!("{} {n} ft.", dnt_data::view::title_case(mode)));
        }
    }
    let listed = creature::bonuses(d, "skill");
    let skills = SKILLS
        .iter()
        .filter(|(k, _, _)| listed.iter().any(|(l, _)| l.replace(' ', "_") == *k) || ev.int(&format!("skill.extra.{k}")).unwrap_or(0) != 0)
        .map(|(k, n, _)| Skill { key: k.to_string(), name: n.to_string(), bonus: int(&format!("skill.{k}")) })
        .collect();
    let list =
        |key: &str| d.get(key).and_then(|v| v.as_array()).into_iter().flatten().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(", ");
    let actions = sheet
        .actions
        .iter()
        .filter(|(a, _)| a.when.is_none() || matches!(ev.get(&key(&a.name, "on")), Some(dnt_rules::Value::Bool(true))))
        .map(|(a, section)| Action {
            name: a.name.clone(),
            section: section.clone(),
            attack: a.attack.as_ref().and(ev.int(&key(&a.name, "attack"))),
            save: a.save.clone(),
            dc: a.dc.as_ref().and(ev.int(&key(&a.name, "dc"))),
            damage: ev.get(&key(&a.name, "damage")).map(|v| match &a.damage_type {
                Some(t) => format!("{v} {t}"),
                None => v.to_string(),
            }),
            heal: ev.get(&key(&a.name, "heal")).map(|v| v.to_string()),
        })
        .collect();
    Numbers {
        hp: comp.hp.unwrap_or(hp_max).min(hp_max),
        hp_max,
        temp_hp: comp.temp_hp,
        ac: int("ac"),
        initiative: int("initiative"),
        speed: speed.join(", "),
        abilities: ABILITIES
            .iter()
            .map(|(k, _)| Ability {
                key: k.to_string(),
                score: int(k),
                modifier: int(&format!("mod.{k}")),
                save: int(&format!("save.{k}")),
            })
            .collect(),
        skills,
        passive_perception: int("passive.perception"),
        senses: dnt_data::tags::parse(&list("senses")).into_iter().map(|s| s.text).collect(),
        languages: list("languages"),
        cr: creature::challenge(d),
        actions,
    }
}

fn key(name: &str, part: &str) -> String {
    format!("action.{}.{part}", dnt_rules::class_table::slug(name))
}

/// What the character has that gives companions: its features, and the
/// spells it knows or has prepared.
pub fn givers(s: &CharacterSession, lib: &Library, overlays: &OverlaySet) -> Vec<Giver> {
    let c = s.character();
    let mut have: Vec<EntityId> = s.build.features.iter().map(|f| f.entity).collect();
    let spells = &c.spells;
    for ids in spells.classes.values().flat_map(|k| [&k.cantrips, &k.prepared, &k.spellbook]).chain(spells.chosen.values()) {
        have.extend(ids.iter().filter_map(|id| resolve_id(lib, id)));
    }
    let mut seen = Vec::new();
    have.retain(|e| {
        !seen.contains(e) && {
            seen.push(*e);
            true
        }
    });
    have.into_iter().flat_map(|e| gives(s, lib, overlays, e)).collect()
}

/// The companions an entity's rules give (a spell's familiar, a feature's
/// beast), whoever has it and however: a cast spell summons by its own
/// rules.
pub fn gives(s: &CharacterSession, lib: &Library, overlays: &OverlaySet, e: EntityId) -> Vec<Giver> {
    let c = s.character();
    let vars = companion::owner_vars(&s.ev, &s.build.model.inputs);
    let entity = lib.entity(e);
    // Its own id (`from` in the character file), whichever entry its
    // rules were written for (the SRD's, for a book's reprint).
    let id = crate::rules::overlay_id(lib, e);
    overlays
        .for_entity(&entity.key)
        .into_iter()
        .flat_map(|o| o.companions.iter())
        .map(|rule| {
            let creatures = companion::choices(rule, lib, &s.filter)
                .into_iter()
                .map(|cid| {
                    let ce = lib.entity(cid);
                    (format!("creature:{}|{}", ce.name, ce.source), ce.name.clone(), creature::challenge(&ce.data).unwrap_or_default())
                })
                .collect();
            let most = rule.count.as_ref().and_then(|f| f.eval(&vars).ok()).and_then(|v| v.as_num()).map_or(1, |n| n.floor());
            let have =
                c.companions.iter().filter(|x| !x.away && x.from.as_deref().and_then(|f| resolve_id(lib, f)) == Some(e)).count() as i64;
            let scales = entity.kind == dnt_data::Kind::Spell && companion::uses_spell_level(rule, lib);
            let spell_level = scales.then(|| entity.data.get("level").and_then(|l| l.as_i64()).unwrap_or(0));
            Giver {
                id: id.clone(),
                name: entity.name.clone(),
                kind: rule.kind.clone(),
                creatures,
                types: rule.types.clone(),
                most,
                have,
                spell_level,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::Command;
    use crate::rolls::{self, CompanionRoll};
    use dnt_rules::character::{Character, ClassSpells};
    use dnt_rules::overlay::Overlay;
    use dnt_store::CharacterFile;
    use serde_json::json;

    /// The spell summons by its own rules, however the character has it
    /// (here an invocation gives it at will) and however it is cast (here
    /// without a slot: the character has none).
    #[test]
    fn a_spell_an_invocation_gives_summons_without_a_slot() {
        let mut lib = dnt_data::Library::new();
        lib.load_json(&json!({
            "spell": [{"name": "Find Pet", "source": "T", "level": 1, "meta": {"ritual": true}}],
            "optionalfeature": [{"name": "Pact of Pets", "source": "T", "featureType": ["EI"], "entries": ["x"],
                "additionalSpells": [{"ability": "cha", "innate": {"_": {"ritual": ["find pet|t"]}}}]}],
            "monster": [{"name": "Owl", "source": "T", "size": ["T"], "type": "beast", "familiar": true, "cr": "0", "ac": [11],
                "hp": {"average": 3}, "speed": {"walk": 5}, "str": 3, "dex": 13, "con": 8, "int": 2, "wis": 12, "cha": 7}]
        }));
        let (rules, _) = Overlay::parse(
            r#"
[[feature]]
id = "spell:Find Pet|T"
[[feature.companion]]
kind = "Familiar"
creatures = "creature.familiar"
types = ["fey"]
"#,
        )
        .unwrap();
        let mut set = OverlaySet::default();
        set.add(rules);
        let mut c = Character { name: "A".into(), ..Default::default() };
        c.options.push("optionalfeature:Pact of Pets|T".into());
        let file = CharacterFile { slug: "a".into(), path: "/tmp/a.toml".into(), character: c };
        let mut s = CharacterSession::new(file, &lib, &set, &Default::default());
        let pet = lib.get(&dnt_data::Key::new(dnt_data::Kind::Spell, &["Find Pet", "T"])).unwrap();
        let origin = crate::spellbook::origin(&lib, &s, pet);
        assert_eq!(origin.as_deref(), Some("optionalfeature:Pact of Pets|T"));
        let (_, ways) = crate::spellbook::cast_choices(&lib, &s, pet, origin.as_deref()).unwrap();
        assert_eq!((ways[0].label.as_str(), ways[0].hint.as_str(), ways[0].value.as_str()), ("No slot", "Pact of Pets", "free"));
        let cast = s.apply(&lib, &set, Command::Cast { id: "spell:Find Pet|T".into(), how: "free".into(), origin }).unwrap();
        assert_eq!(cast.message, "Cast Find Pet");
        let summon = cast.summon.expect("asks what to summon");
        assert_eq!(summon.giver.creatures.iter().map(|c| c.1.as_str()).collect::<Vec<_>>(), ["Owl"]);
        // Its attack and DC are the invocation's ability's: Charisma, not
        // the better Intelligence.
        (s.file.character.abilities.cha, s.file.character.abilities.int) = (14, 20);
        s.rebuild(&lib, &set);
        let prof = s.ev.int("prof").unwrap();
        assert_eq!(rolls::spell_numbers(&s, Some("optionalfeature:Pact of Pets|T")), (2 + prof, 10 + prof));
    }

    #[test]
    fn a_spell_gives_a_familiar_that_rolls() {
        let mut lib = dnt_data::Library::new();
        lib.load_json(&json!({
            "spell": [{"name": "Find Pet", "source": "T", "level": 1}],
            "condition": [{"name": "Prone", "source": "T"}],
            "monster": [
                {"name": "Owl", "source": "T", "size": ["T"], "type": "beast", "familiar": true, "cr": "0", "ac": [11],
                 "hp": {"average": 3}, "speed": {"walk": 5, "fly": 60}, "str": 3, "dex": 13, "con": 8, "int": 2, "wis": 12, "cha": 7,
                 "skill": {"perception": "+5"},
                 "action": [{"name": "Talons", "entries": ["{@atkr m} {@hit 3}, reach 5 ft. {@h}{@damage 1d4} Slashing damage."]}]},
                {"name": "Bear", "source": "T", "size": ["L"], "type": "beast", "cr": "1", "ac": [11], "hp": {"average": 19},
                 "speed": {"walk": 40}, "str": 15, "dex": 10, "con": 14, "int": 2, "wis": 13, "cha": 7}
            ]
        }));
        let (rules, d) = Overlay::parse(
            r#"
[[feature]]
id = "spell:Find Pet|T"
[[feature.companion]]
kind = "Familiar"
creatures = "creature.familiar"
types = ["fey"]
"#,
        )
        .unwrap();
        assert!(d.is_empty(), "{d:?}");
        let mut set = OverlaySet::default();
        set.add(rules);
        let mut c = Character { name: "A".into(), ..Default::default() };
        c.spells.classes.insert("class:Wizard|T".into(), ClassSpells { prepared: vec!["spell:Find Pet|T".into()], ..Default::default() });
        let file = CharacterFile { slug: "a".into(), path: "/tmp/a.toml".into(), character: c };
        let mut s = CharacterSession::new(file, &lib, &set, &Default::default());

        let g = &companions(&s, &lib, &set).givers;
        assert_eq!((g.len(), g[0].kind.as_str(), g[0].most, g[0].have), (1, "Familiar", 1, 0));
        assert_eq!(g[0].creatures.iter().map(|c| c.1.as_str()).collect::<Vec<_>>(), ["Owl"]);

        let add = |creature: &str, name: &str| Command::AddCompanion {
            creature: creature.into(),
            name: name.into(),
            from: Some("spell:Find Pet|T".into()),
            spell_level: None,
            creature_type: Some("fey".into()),
        };
        assert_eq!(s.apply(&lib, &set, add("creature:Bear|T", "")).unwrap_err(), "Bear cannot be Find Pet");
        s.apply(&lib, &set, add("creature:Owl|T", "Pip")).unwrap();
        assert_eq!(s.apply(&lib, &set, add("creature:Owl|T", "Hoot")).unwrap_err(), "Find Pet gives 1 at once");
        // A follower: any creature, nothing gives it.
        let bear = Command::AddCompanion {
            creature: "creature:Bear|T".into(),
            name: String::new(),
            from: None,
            spell_level: None,
            creature_type: None,
        };
        s.apply(&lib, &set, bear).unwrap();

        let list = companions(&s, &lib, &set).list;
        assert_eq!((list[0].name.as_str(), list[0].kind.as_str(), list[0].type_line.as_str()), ("Pip", "Familiar", "Tiny Fey"));
        assert_eq!((list[1].name.as_str(), list[1].kind.as_str()), ("Bear", "Follower"));
        let n = list[0].numbers.as_ref().unwrap();
        assert_eq!((n.hp, n.hp_max, n.ac, n.speed.as_str()), (3, 3, 11, "5 ft., Fly 60 ft."));
        assert_eq!((n.skills[0].name.as_str(), n.skills[0].bonus, n.passive_perception), ("Perception", 5, 15));
        assert_eq!((n.actions[0].attack, n.actions[0].damage.as_deref()), (Some(3), Some("1d4 slashing")));

        s.apply(&lib, &set, Command::CompanionTempHp { index: 0, amount: 1 }).unwrap();
        s.apply(&lib, &set, Command::CompanionHp { index: 0, damage: 2 }).unwrap();
        s.apply(&lib, &set, Command::SetCompanionCondition { index: 0, condition: "prone".into(), on: true }).unwrap();
        let pip = &companions(&s, &lib, &set).list[0];
        assert_eq!((pip.numbers.as_ref().unwrap().hp, pip.conditions.as_slice()), (2, ["Prone".to_string()].as_slice()));
        s.apply(&lib, &set, Command::CompanionHp { index: 0, damage: -10 }).unwrap();
        assert_eq!(s.character().companions[0].hp, None, "healed to full");

        let r = rolls::companion(&mut s, 0, &CompanionRoll::Skill { key: "perception".into() }).unwrap();
        assert_eq!(r.title, "Pip: Perception");
        let r = rolls::companion(&mut s, 0, &CompanionRoll::Action { name: "Talons".into() }).unwrap();
        assert_eq!(r.title, "Pip: Talons");
        assert!(r.result.ends_with("to hit") || r.result.contains("miss") || r.result.contains("critical"), "{r:?}");
        assert!(r.extra[0].contains("slashing damage"), "{r:?}");

        s.apply(&lib, &set, Command::RemoveCompanion { index: 0 }).unwrap();
        assert_eq!(s.character().companions.len(), 1);
    }
}
