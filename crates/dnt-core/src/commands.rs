//! Everything that changes a character in play, as data: a frontend builds
//! a `Command` (from a key, a tap or a JSON request) and applies it to the
//! session. The session saves nothing itself; the outcome says whether the
//! character changed and so needs saving.

use dnt_data::Library;
use dnt_rules::builder::FeatureSource;
use dnt_rules::character::{Hand, InventoryItem, Slot};
use dnt_rules::overlay::{resolve_id, OverlaySet};
use dnt_rules::{choices, equipment, play, spells};
use serde::{Deserialize, Serialize};

use crate::rolls::{self, Rolled};
use crate::session::{item_entity, line_name, CharacterSession};
use crate::spellbook::{self, Target};

/// A use-limited pool to spend from or restore.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "pool", rename_all = "snake_case")]
pub enum Pool {
    Resource { key: String },
    Slot { level: usize },
    Pact,
    HitDie { faces: i64 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum Command {
    // ---- play ----
    /// Spend `amount` (or restore, when negative).
    Spend {
        pool: Pool,
        amount: i64,
    },
    SetToggle {
        key: String,
        on: bool,
    },
    Damage {
        amount: i64,
        /// From a critical hit (two death save failures at 0 hit points).
        #[serde(default)]
        critical: bool,
    },
    /// Roll a death save (at 0 hit points).
    DeathSave,
    /// Spend a hit die: roll it, add Constitution and heal that much.
    RollHitDie {
        faces: i64,
    },
    /// Keep track of something with uses (a magic item's charges).
    AddTracker {
        name: String,
        max: i64,
        recharge: dnt_rules::overlay::Recharge,
    },
    RemoveTracker {
        key: String,
    },
    /// Dead (killed outright), or not.
    SetDead {
        dead: bool,
    },
    Heal {
        amount: i64,
    },
    SetTempHp {
        amount: i64,
    },
    ShortRest,
    LongRest,
    SetCondition {
        condition: String,
        on: bool,
    },
    SetExhaustion {
        level: i64,
    },
    EndConcentration,

    // ---- inventory (lines are indexes into the inventory) ----
    /// Add a library item (`item:Longsword|XPHB`), inside a container if given.
    AddItem {
        id: String,
        inside: Option<String>,
    },
    /// A custom item: just a name.
    AddCustomItem {
        name: String,
        inside: Option<String>,
    },
    ChangeQuantity {
        line: usize,
        delta: i64,
    },
    RemoveLine {
        line: usize,
    },
    /// Weapons and shields are held, other items worn: whichever fits.
    Equip {
        line: usize,
    },
    Wear {
        line: usize,
        slot: Slot,
    },
    TakeOff {
        line: usize,
    },
    /// Take in a hand, or put away if already there.
    Hold {
        line: usize,
        hand: Hand,
    },
    ToggleAttuned {
        line: usize,
    },
    /// Mark or unmark a monk weapon (back to the rules when they agree).
    ToggleMonkWeapon {
        line: usize,
    },
    /// Put a line into a container line, or carry it loose (`None`).
    PutIn {
        line: usize,
        into: Option<usize>,
    },
    Unpack {
        line: usize,
    },
    /// Container settings: `30`, `weightless`, `no limit`, `off`, or empty.
    SetContainer {
        line: usize,
        settings: String,
    },
    /// Coins: `+15gp`, `-3sp`, `20` (sets gold).
    ChangeCoins {
        change: String,
    },

    // ---- spells (ids like `spell:Fireball|XPHB`) ----
    /// Add spells to a casting class or a feature's choice, within limits.
    AddSpells {
        to: Target,
        ids: Vec<String>,
    },
    RemoveSpell {
        from: Target,
        id: String,
    },
    /// Prepare a spellbook spell, or unprepare a prepared one (Wizards).
    TogglePrepared {
        class_id: String,
        id: String,
    },
    /// Cast a spell `how`: `slot:N`, `pact`, `ritual`, or `free` (an innate
    /// cast); rolls what the spell rolls.
    Cast {
        id: String,
        how: String,
    },
    ToggleFavorite {
        id: String,
    },
    // ---- levels ----
    /// Make a choice a level is missing (see `CharacterSession::pending`).
    MakeChoice {
        level: usize,
        key: String,
        values: Vec<String>,
    },
    /// Remove the latest level.
    UndoLevel,

    /// Trade a mastered weapon (`item:Club|XPHB`) for another.
    SwapMastery {
        weapon: String,
        new: String,
    },
}

/// What applying a command did.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Outcome {
    /// For the status line.
    pub message: String,
    /// The character changed (and should be saved).
    pub changed: bool,
    /// A roll the command made (casting a spell that rolls).
    pub rolled: Option<Rolled>,
}

impl Outcome {
    fn changed(message: impl Into<String>) -> Outcome {
        Outcome { message: message.into(), changed: true, rolled: None }
    }
}

/// A command that could not be applied, and why.
pub type Refused = String;

/// An entity's name for messages (the id when it is unknown).
fn spell_name(lib: &Library, id: &str) -> String {
    resolve_id(lib, id).map_or_else(|| id.to_string(), |e| lib.entity(e).name.clone())
}

fn line(session: &CharacterSession, i: usize) -> Result<&InventoryItem, Refused> {
    session.file.character.inventory.get(i).ok_or_else(|| format!("There is no inventory line {i}"))
}

impl CharacterSession {
    /// Apply a command; on success the session is recomputed.
    pub fn apply(&mut self, lib: &Library, overlays: &OverlaySet, command: Command) -> Result<Outcome, Refused> {
        let cast = match &command {
            Command::Cast { id, how } => resolve_id(lib, id).map(|e| (e, how.clone(), self.caster_of(lib, id))),
            _ => None,
        };
        let mut outcome = self.run(lib, command)?;
        if outcome.changed {
            self.rebuild(lib, overlays);
        }
        if let Some((spell, how, caster)) = cast.filter(|(e, _, _)| rolls::spell_rolls(lib, *e)) {
            let (atk, dc) = rolls::spell_numbers(self, caster.as_deref());
            let level = rolls::cast_level(self, lib, spell, &how);
            outcome.rolled = Some(rolls::spell(self, lib, spell, level, atk, dc));
        }
        Ok(outcome)
    }

    /// The slug of the casting class that has spell `id` (for its attack
    /// and DC), or `None` for spells from features and items.
    fn caster_of(&self, lib: &Library, id: &str) -> Option<String> {
        let mine = &self.file.character.spells.classes;
        spellbook::rows(lib, self).casters.into_iter().find_map(|c| {
            let s = mine.get(&c.id)?;
            s.cantrips.iter().chain(&s.prepared).chain(&s.spellbook).any(|x| x == id).then_some(c.slug)
        })
    }

    fn run(&mut self, lib: &Library, command: Command) -> Result<Outcome, Refused> {
        let ev = &self.ev;
        let c = &mut self.file.character;
        Ok(match command {
            Command::Spend { pool, amount } => {
                let (ok, what) = match &pool {
                    Pool::Resource { key } => (
                        play::spend_resource(c, ev, key, amount),
                        self.build.resources.iter().find(|(r, _)| &r.key == key).map_or(key.clone(), |(r, _)| r.name.clone()),
                    ),
                    Pool::Slot { level } => (play::spend_slot(c, ev, *level, amount), format!("level {level} slot")),
                    Pool::Pact => (play::spend_pact_slot(c, ev, amount), "pact slot".into()),
                    Pool::HitDie { faces } => (play::spend_hit_die(c, ev, *faces, amount), format!("d{faces} hit die")),
                };
                if !ok {
                    return Err(if amount > 0 { format!("No {what} left") } else { format!("{what} is full") });
                }
                Outcome::changed(format!("{} {what}", if amount > 0 { "Used" } else { "Restored" }))
            }
            Command::SetToggle { key, on } => {
                if on {
                    c.state.toggles.insert(key.clone(), true);
                } else {
                    c.state.toggles.remove(&key);
                }
                let name = self.build.toggles.iter().find(|(t, _)| t.key == key).map_or(key, |(t, _)| t.name.clone());
                Outcome::changed(format!("{name} {}", if on { "on" } else { "off" }))
            }
            Command::Damage { amount, critical } => {
                let d = play::damage(c, ev, amount, critical);
                Outcome::changed(match (d.dead, d.failures) {
                    (true, _) => format!("Damage {amount} at 0 hit points: a third failed death save, dead"),
                    (_, 0) => format!("Damage {amount}"),
                    (_, n) => format!("Damage {amount} at 0 hit points: {n} failed death save(s)"),
                })
            }
            Command::DeathSave => {
                if c.state.dead {
                    return Err("Dead: no more death saves".into());
                }
                if play::current_hp(c, ev) > 0 {
                    return Err("Death saves are made at 0 hit points".into());
                }
                let r = rolls::death_save(self);
                return Ok(Outcome { message: "Death save".into(), changed: true, rolled: Some(r) });
            }
            Command::RollHitDie { faces } => {
                if c.state.dead {
                    return Err("Dead: no healing".into());
                }
                if play::current_hp(c, ev) >= play::max_hp(ev) {
                    return Err("Hit points are full".into());
                }
                if !play::spend_hit_die(c, ev, faces, 1) {
                    return Err(format!("No d{faces} hit die left"));
                }
                let r = rolls::hit_die(self, faces);
                return Ok(Outcome { message: format!("Spent a d{faces} hit die"), changed: true, rolled: Some(r) });
            }
            Command::AddTracker { name, max, recharge } => {
                let name = name.trim().to_string();
                if name.is_empty() || max < 1 {
                    return Err("A tracker needs a name and at least one use".into());
                }
                let base: String = name.to_lowercase().chars().map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' }).collect();
                let base = format!("custom_{}", base.trim_matches('_'));
                let mut key = base.clone();
                let mut n = 2;
                while c.trackers.iter().any(|t| t.key == key) {
                    key = format!("{base}_{n}");
                    n += 1;
                }
                c.trackers.push(dnt_rules::character::CustomTracker { key, name: name.clone(), max, recharge });
                Outcome::changed(format!("Tracking {name}"))
            }
            Command::RemoveTracker { key } => {
                let at = c.trackers.iter().position(|t| t.key == key).ok_or("No such tracker")?;
                let t = c.trackers.remove(at);
                c.state.resources_used.remove(&t.key);
                Outcome::changed(format!("No longer tracking {}", t.name))
            }
            Command::SetDead { dead } => {
                play::set_dead(c, dead);
                Outcome::changed(if dead { "Dead" } else { "No longer dead" })
            }
            Command::Heal { amount } => {
                play::heal(c, ev, amount);
                Outcome::changed(format!("Heal {amount}"))
            }
            Command::SetTempHp { amount } => {
                play::set_temp_hp(c, amount);
                Outcome::changed(format!("Temporary hit points {amount}"))
            }
            Command::ShortRest | Command::LongRest => {
                let long = command == Command::LongRest;
                let report = if long { play::long_rest(c, &self.build, ev, self.edition) } else { play::short_rest(c, &self.build, ev) };
                let what = if long { "Long rest" } else { "Short rest" };
                let detail = if report.restored.is_empty() { "nothing to restore".to_string() } else { report.restored.join(", ") };
                let mut msg = format!("{what}: {detail}");
                if long && self.build.features.iter().any(|f| f.source == FeatureSource::Mastery) {
                    msg.push_str(" · you may swap a Weapon Mastery");
                }
                Outcome::changed(msg)
            }
            Command::SetCondition { condition, on } => {
                let name = dnt_rules::conditions::CONDITIONS
                    .iter()
                    .find(|n| n.eq_ignore_ascii_case(&condition))
                    .ok_or_else(|| format!("{condition} is not a condition"))?;
                let had = c.state.conditions.iter().position(|x| x.eq_ignore_ascii_case(name));
                match (had, on) {
                    (Some(i), false) => {
                        c.state.conditions.remove(i);
                    }
                    (None, true) => c.state.conditions.push(name.to_string()),
                    _ => {}
                }
                Outcome::changed(if on { name.to_string() } else { format!("No longer {name}") })
            }
            Command::SetExhaustion { level } => {
                c.state.exhaustion = level.clamp(0, 6);
                Outcome::changed(format!("Exhaustion {}", c.state.exhaustion))
            }
            Command::EndConcentration => match c.state.concentration.take() {
                Some(name) => Outcome::changed(format!("Stopped concentrating on {name}")),
                None => return Err("Not concentrating".into()),
            },

            Command::AddItem { id, inside } => {
                let before = c.inventory.len();
                equipment::add_item(c, &id, 1, None);
                if c.inventory.len() > before {
                    c.inventory[before].inside = inside;
                }
                let name = dnt_rules::overlay::resolve_id(lib, &id).map(|e| lib.entity(e).name.clone()).unwrap_or(id);
                Outcome::changed(format!("Added {name}"))
            }
            Command::AddCustomItem { name, inside } => {
                let name = name.trim().to_string();
                if name.is_empty() {
                    return Err("A custom item needs a name".into());
                }
                c.inventory.push(InventoryItem { name: Some(name.clone()), quantity: 1, inside, ..Default::default() });
                Outcome::changed(format!("Added {name}"))
            }
            Command::ChangeQuantity { line: i, delta } => {
                line(self, i)?;
                let l = &mut self.file.character.inventory[i];
                if l.quantity + delta < 1 {
                    return Err("Remove the line instead (x)".into());
                }
                l.quantity += delta;
                Outcome::changed(format!("Quantity {delta:+}"))
            }
            Command::RemoveLine { line: i } => {
                line(self, i)?;
                let removed = equipment::remove_line(&mut self.file.character, i);
                Outcome::changed(format!("Removed {}", line_name(lib, &removed)))
            }
            Command::Equip { line: i } => {
                let l = line(self, i)?;
                let data = item_entity(lib, l).map(|e| &lib.entity(e).data);
                if data.is_some_and(equipment::is_held) {
                    let shield = data.and_then(equipment::armor_kind) == Some("shield");
                    return self.run(lib, Command::Hold { line: i, hand: if shield { Hand::Off } else { Hand::Main } });
                }
                if l.equipped {
                    return self.run(lib, Command::TakeOff { line: i });
                }
                let name = line_name(lib, l);
                let slot = data.map(|d| equipment::guess_slot(&self.file.character, &name, d)).unwrap_or(Slot::Other);
                return self.run(lib, Command::Wear { line: i, slot });
            }
            Command::Wear { line: i, slot } => {
                line(self, i)?;
                let off = equipment::wear(lib, &mut self.file.character, i, slot);
                let name = line_name(lib, &self.file.character.inventory[i]);
                let mut msg = format!("Wearing {name} ({})", slot.label());
                if !off.is_empty() {
                    msg.push_str(&format!(", took off {}", off.join(", ")));
                }
                Outcome::changed(msg)
            }
            Command::TakeOff { line: i } => {
                line(self, i)?;
                equipment::take_off(&mut self.file.character, i);
                Outcome::changed(format!("Took off {}", line_name(lib, &self.file.character.inventory[i])))
            }
            Command::Hold { line: i, hand } => {
                let l = line(self, i)?;
                if !item_entity(lib, l).is_some_and(|e| equipment::is_held(&lib.entity(e).data)) {
                    return Err("Only weapons and shields are held".into());
                }
                Outcome::changed(equipment::wield(lib, &mut self.file.character, i, hand))
            }
            Command::ToggleAttuned { line: i } => {
                line(self, i)?;
                let l = &mut self.file.character.inventory[i];
                l.attuned = !l.attuned;
                let mut msg = format!("{} {}", if l.attuned { "Attuned to" } else { "No longer attuned to" }, line_name(lib, l));
                if l.attuned && !l.equipped && l.hand.is_none() {
                    msg.push_str(" (not worn: its magic works only when worn)");
                }
                Outcome::changed(msg)
            }
            Command::ToggleMonkWeapon { line: i } => {
                let l = line(self, i)?;
                let Some(e) = item_entity(lib, l) else { return Err("Not a library item".into()) };
                let ent = lib.entity(e);
                let Some(w) = dnt_rules::attacks::Weapon::from_item(&ent.name.to_lowercase(), &ent.data) else {
                    return Err(format!("{} is not a weapon", ent.name));
                };
                let preset = self.build.martial_arts.is_some_and(|m| m.preset(&w));
                let now = !l.monk_weapon.unwrap_or(preset);
                let name = ent.name.clone();
                self.file.character.inventory[i].monk_weapon = (now != preset).then_some(now);
                Outcome::changed(match (now, self.build.martial_arts.is_some()) {
                    (true, true) => format!("{name} is a monk weapon"),
                    (true, false) => format!("{name} is marked as a monk weapon (it counts once you have Martial Arts)"),
                    (false, _) => format!("{name} is not a monk weapon"),
                })
            }
            Command::PutIn { line: i, into } => {
                line(self, i)?;
                Outcome::changed(equipment::put_in(lib, &mut self.file.character, i, into)?)
            }
            Command::Unpack { line: i } => {
                line(self, i)?;
                Outcome::changed(equipment::unpack(lib, &mut self.file.character, i)?)
            }
            Command::SetContainer { line: i, settings } => {
                line(self, i)?;
                equipment::set_container(&mut self.file.character.inventory[i], &settings)?;
                Outcome::changed(match equipment::container(lib, &self.file.character.inventory[i]) {
                    Some(k) => format!(
                        "Container: {}{}",
                        k.capacity.map(|w| format!("{w} lb")).unwrap_or_else(|| "no limit".into()),
                        if k.weightless { ", weightless" } else { "" }
                    ),
                    None => "Not a container".into(),
                })
            }
            Command::ChangeCoins { change } => {
                if !c.currency.apply(&change) {
                    return Err(format!("Could not read {change:?}: try +15gp, -3sp or 20"));
                }
                Outcome::changed(format!("Coins {}", change.trim()))
            }

            Command::AddSpells { to, ids } => {
                let names: Vec<String> = ids.iter().map(|id| spell_name(lib, id)).collect();
                spellbook::add(lib, c, &spells::casters(lib, &self.build, ev), &to, ids)?;
                Outcome::changed(format!("Added {}", names.join(", ")))
            }
            Command::RemoveSpell { from, id } => {
                if !spellbook::remove(c, &from, &id) {
                    return Err(format!("{} is not there", spell_name(lib, &id)));
                }
                // Gone from the sheet: no longer a favorite either.
                let granted = self
                    .build
                    .granted
                    .iter()
                    .flat_map(|g| &g.grants)
                    .any(|g| matches!(g, spells::Granted::Spell { id: s, .. } if *s == id));
                if !granted && !spellbook::has(c, &id) {
                    c.spells.favorites.retain(|f| *f != id);
                }
                Outcome::changed(format!("Removed {}", spell_name(lib, &id)))
            }
            Command::TogglePrepared { class_id, id } => {
                let on = spellbook::toggle_prepared(c, &spells::casters(lib, &self.build, ev), &class_id, &id)?;
                Outcome::changed(format!("{} {}", if on { "Prepared" } else { "Unprepared" }, spell_name(lib, &id)))
            }
            Command::Cast { id, how } => {
                let spell = resolve_id(lib, &id).ok_or_else(|| format!("Unknown spell {id}"))?;
                Outcome::changed(spellbook::cast(lib, c, ev, spell, &how)?)
            }
            Command::ToggleFavorite { id } => {
                let name = spell_name(lib, &id);
                let on = spellbook::toggle_favorite(c, &id);
                Outcome::changed(if on { format!("{name} is a favorite") } else { format!("{name} is no longer a favorite") })
            }
            Command::MakeChoice { level, key, values } => {
                let l = c.levels.get_mut(level).ok_or_else(|| format!("There is no level {}", level + 1))?;
                l.choices.picks.entry(key.clone()).or_default().extend(values);
                Outcome::changed(format!("{key} chosen"))
            }
            Command::UndoLevel => {
                let l = c.remove_last_level().ok_or("There is no level to remove")?;
                let name = l.class.split_once(':').map_or("", |(_, r)| r.split('|').next().unwrap_or(r)).to_string();
                Outcome::changed(format!("Removed a {name} level"))
            }
            Command::SwapMastery { weapon, new } => {
                let old = resolve_id(lib, &weapon).ok_or_else(|| format!("Unknown weapon {weapon}"))?;
                let (from, to) = (lib.entity(old).name.clone(), spell_name(lib, &new));
                if !choices::swap_mastery(lib, c, old, &new) {
                    return Err(format!("{from} is not a mastery chosen at a level"));
                }
                Outcome::changed(format!("Weapon Mastery: {from} → {to}"))
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dnt_rules::character::Character;
    use dnt_store::CharacterFile;
    use serde_json::json;

    fn session() -> (Library, CharacterSession) {
        let mut lib = Library::new();
        lib.load_json(&json!({
            "baseitem": [{"name": "Club", "source": "T", "type": "M|T", "weapon": true, "weaponCategory": "simple", "dmg1": "1d4", "dmgType": "B"}],
            "item": [{"name": "Cloak", "source": "T", "wondrous": true},
                     {"name": "Sack", "source": "T", "weight": 0.5, "containerCapacity": {"weight": [30]}}]
        }));
        let file =
            CharacterFile { slug: "a".into(), path: "/tmp/a.toml".into(), character: Character { name: "A".into(), ..Default::default() } };
        let s = CharacterSession::new(file, &lib, &OverlaySet::default(), &Default::default());
        (lib, s)
    }

    #[test]
    fn commands_round_trip_as_json() {
        let c = Command::Spend { pool: Pool::Slot { level: 1 }, amount: 1 };
        let text = serde_json::to_string(&c).unwrap();
        assert_eq!(text, r#"{"command":"spend","pool":{"pool":"slot","level":1},"amount":1}"#);
        assert_eq!(serde_json::from_str::<Command>(&text).unwrap(), c);
        let w: Command = serde_json::from_str(r#"{"command":"wear","line":0,"slot":"shoulders"}"#).unwrap();
        assert_eq!(w, Command::Wear { line: 0, slot: Slot::Shoulders });
    }

    #[test]
    fn inventory_and_play() {
        let (lib, mut s) = session();
        let o = &OverlaySet::default();
        for id in ["item:Club|T", "item:Cloak|T", "item:Sack|T"] {
            s.apply(&lib, o, Command::AddItem { id: id.into(), inside: None }).unwrap();
        }
        // Equip picks holding or wearing.
        assert!(s.apply(&lib, o, Command::Equip { line: 0 }).unwrap().message.contains("main hand"));
        assert!(s.apply(&lib, o, Command::Equip { line: 1 }).unwrap().message.contains("Shoulders"));
        assert_eq!(s.build.attacks[0].hand, Some(Hand::Main), "the session was rebuilt");
        s.apply(&lib, o, Command::PutIn { line: 0, into: Some(2) }).unwrap();
        assert!(s.character().inventory[0].inside.is_some());
        assert!(s.apply(&lib, o, Command::ChangeQuantity { line: 0, delta: -1 }).is_err());
        assert!(s.apply(&lib, o, Command::Hold { line: 1, hand: Hand::Main }).is_err(), "a cloak is not held");
        s.apply(&lib, o, Command::SetCondition { condition: "poisoned".into(), on: true }).unwrap();
        assert_eq!(s.character().state.conditions, ["Poisoned"]);
        assert!(s.apply(&lib, o, Command::SetCondition { condition: "sleepy".into(), on: true }).is_err());
        s.apply(&lib, o, Command::Damage { amount: 3, critical: false }).unwrap();
        s.apply(
            &lib,
            o,
            Command::AddTracker { name: "Ring of Spell Storing".into(), max: 1, recharge: dnt_rules::overlay::Recharge::LongRest },
        )
        .unwrap();
        let key = s.character().trackers[0].key.clone();
        assert_eq!(key, "custom_ring_of_spell_storing");
        assert!(s.build.resources.iter().any(|(r, _)| r.key == key), "a tracker is a resource");
        s.apply(&lib, o, Command::Spend { pool: Pool::Resource { key: key.clone() }, amount: 1 }).unwrap();
        assert!(s.apply(&lib, o, Command::Spend { pool: Pool::Resource { key: key.clone() }, amount: 1 }).is_err(), "only one use");
        assert!(s.apply(&lib, o, Command::LongRest).unwrap().message.starts_with("Long rest"));
        assert!(s.apply(&lib, o, Command::ChangeCoins { change: "+15gp".into() }).is_ok());
        assert_eq!(s.character().currency.gp, 15);
    }

    #[test]
    fn hit_dice_roll_and_heal() {
        let mut lib = Library::new();
        lib.load_json(&json!({"class": [{"name": "Monk", "source": "T", "hd": {"faces": 8}}]}));
        let mut c = Character { name: "A".into(), ..Default::default() };
        c.abilities.con = 14;
        c.levels =
            vec![dnt_rules::character::Level { class: "class:Monk|T".into(), hp: Default::default(), choices: Default::default() }; 2];
        let file = CharacterFile { slug: "a".into(), path: "/tmp/a.toml".into(), character: c };
        let o = &OverlaySet::default();
        let mut s = CharacterSession::new(file, &lib, o, &Default::default());
        assert!(s.apply(&lib, o, Command::RollHitDie { faces: 8 }).is_err(), "full hit points");

        s.apply(&lib, o, Command::Damage { amount: 15, critical: false }).unwrap();
        let before = play::current_hp(s.character(), &s.ev);
        let out = s.apply(&lib, o, Command::RollHitDie { faces: 8 }).unwrap();
        let healed = play::current_hp(s.character(), &s.ev) - before;
        assert!((3..=10).contains(&healed), "d8 + 2: {healed}");
        assert_eq!(out.rolled.unwrap().result, format!("+{healed} hit points"));
        assert_eq!(s.character().state.hit_dice_used["d8"], 1);

        s.apply(&lib, o, Command::RollHitDie { faces: 8 }).ok();
        s.apply(&lib, o, Command::Damage { amount: 30, critical: false }).unwrap();
        s.apply(&lib, o, Command::Heal { amount: 1 }).unwrap();
        assert_eq!(s.apply(&lib, o, Command::RollHitDie { faces: 8 }).unwrap_err(), "No d8 hit die left");
        assert!(s.apply(&lib, o, Command::RollHitDie { faces: 6 }).is_err(), "no d6 at all");
    }

    #[test]
    fn a_class_spell_rolls_with_its_own_class() {
        let mut lib = Library::new();
        lib.load_json(&json!({
            "class": [{"name": "Wiz", "source": "T", "hd": {"faces": 6}, "casterProgression": "full", "spellcastingAbility": "int"},
                      {"name": "Sor", "source": "T", "hd": {"faces": 6}, "casterProgression": "full", "spellcastingAbility": "cha"}],
            "spell": [{"name": "Bolt", "source": "T", "level": 0, "school": "V", "spellAttack": ["R"], "damageInflict": ["fire"]}]
        }));
        let mut c = Character { name: "A".into(), ..Default::default() };
        (c.abilities.int, c.abilities.cha) = (10, 18);
        let level = |class: &str| dnt_rules::character::Level { class: class.into(), hp: Default::default(), choices: Default::default() };
        c.levels = vec![level("class:Wiz|T"), level("class:Sor|T")];
        c.spells.classes.insert(
            "class:Wiz|T".into(),
            dnt_rules::character::ClassSpells { cantrips: vec!["spell:Bolt|T".into()], ..Default::default() },
        );
        let file = CharacterFile { slug: "a".into(), path: "/tmp/a.toml".into(), character: c };
        let s = CharacterSession::new(file, &lib, &OverlaySet::default(), &Default::default());

        let wiz = s.caster_of(&lib, "spell:Bolt|T");
        assert_eq!(wiz.as_deref(), Some(s.build.classes[0].slug.as_str()), "the Wizard's cantrip");
        assert_eq!(s.caster_of(&lib, "spell:Other|T"), None);
        let (own, best) = (rolls::spell_numbers(&s, wiz.as_deref()), rolls::spell_numbers(&s, None));
        assert_eq!(own.1 + 4, best.1, "Int 10 rather than Cha 18: {own:?} vs {best:?}");
    }

    #[test]
    fn spells_are_added_cast_and_removed() {
        let (mut lib, _) = session();
        lib.load_json(&json!({"spell": [{"name": "Alarm", "source": "T", "level": 1, "school": "A", "meta": {"ritual": true}},
                                        {"name": "Bless", "source": "T", "level": 1, "school": "E", "duration": [{"concentration": true}]}]}));
        let file =
            CharacterFile { slug: "a".into(), path: "/tmp/a.toml".into(), character: Character { name: "A".into(), ..Default::default() } };
        let o = &OverlaySet::default();
        let mut s = CharacterSession::new(file, &lib, o, &Default::default());
        let to = Target::Grant { key: "feat:X|T#0".into() };
        let ids = vec!["spell:Alarm|T".to_string(), "spell:Bless|T".to_string()];
        let added = s.apply(&lib, o, Command::AddSpells { to: to.clone(), ids }).unwrap();
        assert_eq!(added.message, "Added Alarm, Bless");
        assert_eq!(s.character().spells.chosen["feat:X|T#0"].len(), 2);

        let cast = s.apply(&lib, o, Command::Cast { id: "spell:Alarm|T".into(), how: "ritual".into() }).unwrap();
        assert_eq!(cast.message, "Cast Alarm");
        assert!(s.apply(&lib, o, Command::Cast { id: "spell:Bless|T".into(), how: "slot:1".into() }).is_err(), "no slots");

        assert!(s.apply(&lib, o, Command::ToggleFavorite { id: "spell:Bless|T".into() }).unwrap().message.contains("is a favorite"));
        assert!(s.apply(&lib, o, Command::RemoveSpell { from: to.clone(), id: "spell:Bless|T".into() }).is_ok());
        assert!(s.character().spells.favorites.is_empty(), "a removed spell is no longer a favorite");
        assert!(s.apply(&lib, o, Command::RemoveSpell { from: to, id: "spell:Bless|T".into() }).is_err(), "already gone");
        assert_eq!(s.character().spells.chosen["feat:X|T#0"], vec!["spell:Alarm|T".to_string()]);

        let json = r#"{"command":"add_spells","to":{"kind":"class","class_id":"class:Wizard|XPHB"},"ids":["spell:Alarm|T"]}"#;
        let c: Command = serde_json::from_str(json).unwrap();
        assert_eq!(c, Command::AddSpells { to: Target::Class { class_id: "class:Wizard|XPHB".into() }, ids: vec!["spell:Alarm|T".into()] });
    }
}
