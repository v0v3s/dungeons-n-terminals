//! Exporting a character sheet as Markdown, or as an HTML page to print
//! (or save as PDF from the browser). Both are rendered from one simple
//! document so they hold the same content.

use dnt_data::{doc, Library};
use dnt_rules::builder::FeatureSource;
use dnt_rules::character::Hand;
use dnt_rules::equipment;
use dnt_rules::play;
use dnt_rules::spells::{self, Granted};
use dnt_rules::standard::{ABILITIES, SKILLS};

use crate::session::{line_name, CharacterSession};

pub enum Block {
    /// `label: value` lines.
    Facts(Vec<(String, String)>),
    Table {
        head: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    List(Vec<String>),
    /// A named entry with text (feature descriptions).
    Detail {
        name: String,
        text: String,
    },
    Text(String),
}

pub struct Section {
    pub title: String,
    pub blocks: Vec<Block>,
}

pub struct Document {
    pub title: String,
    pub subtitle: String,
    pub sections: Vec<Section>,
}

fn signed(n: i64) -> String {
    format!("{n:+}")
}

/// The character's sheet as a document.
pub fn character(lib: &Library, sheet: &CharacterSession) -> Document {
    let ch = &sheet.file.character;
    let ev = &sheet.ev;
    let b = &sheet.build;
    let int = |k: &str| ev.int(k).unwrap_or(0);
    let value = |k: &str| ev.get(k).map(|v| v.to_string()).unwrap_or_else(|| "—".into());
    let name_of =
        |id: &Option<String>| id.as_deref().and_then(|i| dnt_rules::overlay::resolve_id(lib, i)).map(|e| lib.entity(e).name.clone());
    let mut subtitle = vec![];
    subtitle.extend(name_of(&ch.species));
    subtitle.extend(name_of(&ch.background));
    subtitle.push(b.class_summary());
    subtitle.push(format!("Level {}", ch.total_level()));
    subtitle.retain(|s| !s.is_empty());
    let mut sections = Vec::new();

    // Abilities and the numbers used every turn.
    let rows = ABILITIES
        .iter()
        .map(|(ab, name)| {
            vec![name.to_string(), int(ab).to_string(), signed(int(&format!("mod.{ab}"))), signed(int(&format!("save.{ab}")))]
        })
        .collect();
    let mut combat = vec![
        ("Armor Class".to_string(), value("ac")),
        ("Hit Points".to_string(), format!("{} / {}", play::current_hp(ch, ev), play::max_hp(ev))),
        ("Initiative".to_string(), signed(int("initiative"))),
        ("Speed".to_string(), format!("{} ft.", int("speed"))),
        ("Proficiency Bonus".to_string(), signed(int("prof"))),
        ("Passive Perception".to_string(), int("passive.perception").to_string()),
    ];
    if ch.state.temp_hp > 0 {
        combat.push(("Temporary HP".into(), ch.state.temp_hp.to_string()));
    }
    let hit_dice: Vec<String> = ev
        .stats
        .keys()
        .filter_map(|k| k.strip_prefix("hit_dice.d")?.strip_suffix(".max").map(|f| (k.clone(), f.to_string())))
        .map(|(k, f)| format!("{}d{f}", int(&k)))
        .collect();
    combat.push(("Hit Dice".into(), hit_dice.join(" + ")));
    sections.push(Section {
        title: "Abilities".into(),
        blocks: vec![
            Block::Table { head: vec!["Ability".into(), "Score".into(), "Mod".into(), "Save".into()], rows },
            Block::Facts(combat),
        ],
    });

    let skills = SKILLS
        .iter()
        .map(|(k, name, ab)| {
            let mark = match b.model.inputs.get(&format!("prof.skill.{k}")).and_then(|v| v.as_num()).map(|n| n.to_f64()) {
                Some(x) if x >= 2.0 => " (expertise)",
                Some(x) if x >= 1.0 => " (proficient)",
                _ => "",
            };
            format!("{name} ({}) {}{mark}", ab.to_uppercase(), signed(int(&format!("skill.{k}"))))
        })
        .collect();
    sections.push(Section { title: "Skills".into(), blocks: vec![Block::List(skills)] });

    // Attacks, what is in hand, and extra damage on a hit.
    let rows = b
        .attacks
        .iter()
        .map(|a| {
            let mut damage = format!("{} {}", value(&format!("attack.{}.damage", a.slug)), a.damage_type);
            if ev.get(&format!("attack.{}.versatile", a.slug)).is_some() {
                damage.push_str(&format!(" ({} two-handed)", value(&format!("attack.{}.versatile", a.slug))));
            }
            if a.offhand {
                damage.push_str(&format!("; off hand {}", value(&format!("attack.{}.offhand", a.slug))));
            }
            let mut notes: Vec<String> = Vec::new();
            if let Some(h) = a.hand {
                notes.push(h.label().to_string());
            }
            if let Some(m) = &a.mastery {
                notes.push(format!("mastery: {m}"));
            }
            if a.monk_weapon {
                notes.push("monk weapon".into());
            }
            notes.extend(a.properties.iter().cloned());
            notes.extend(a.notes.iter().cloned());
            vec![a.name.clone(), signed(int(&format!("attack.{}.bonus", a.slug))), damage, a.range.clone(), notes.join(", ")]
        })
        .collect();
    let mut blocks =
        vec![Block::Table { head: vec!["Attack".into(), "Hit".into(), "Damage".into(), "Range".into(), "Notes".into()], rows }];
    let riders: Vec<String> = ev
        .stats
        .iter()
        .filter(|(k, _)| k.starts_with("rider."))
        .filter_map(|(_, r)| {
            let v = r.value.as_ref().ok()?;
            let why = r.parts.iter().rev().find(|p| p.applied).map(|p| p.origin.clone()).unwrap_or_default();
            Some(format!("+{v} {why}"))
        })
        .collect();
    if !riders.is_empty() {
        blocks.push(Block::Text("On a hit:".into()));
        blocks.push(Block::List(riders));
    }
    sections.push(Section { title: "Attacks".into(), blocks });

    // Resources.
    if !b.resources.is_empty() {
        let rows = b
            .resources
            .iter()
            .map(|(r, origin)| {
                let max = int(&format!("resource.{}.max", r.key));
                let recharge = match r.recharge {
                    dnt_rules::overlay::Recharge::ShortRest => "short rest",
                    dnt_rules::overlay::Recharge::LongRest => "long rest",
                    dnt_rules::overlay::Recharge::Dawn => "dawn",
                    dnt_rules::overlay::Recharge::Manual => "",
                };
                vec![r.name.clone(), format!("{} / {max}", play::resource_left(ch, ev, &r.key)), recharge.to_string(), origin.clone()]
            })
            .collect();
        sections.push(Section {
            title: "Resources".into(),
            blocks: vec![Block::Table { head: vec!["Resource".into(), "Left".into(), "Recharge".into(), "From".into()], rows }],
        });
    }

    // Spellcasting.
    let casters = spells::casters(lib, b, ev);
    let spell_names = |ids: &[String]| -> Vec<String> {
        let mut v: Vec<(i64, String)> = ids
            .iter()
            .filter_map(|id| dnt_rules::overlay::resolve_id(lib, id))
            .map(|e| (spells::spell_level(lib, e), lib.entity(e).name.clone()))
            .collect();
        v.sort();
        v.into_iter().map(|(l, n)| if l == 0 { n } else { format!("{n} ({l})") }).collect()
    };
    if !casters.is_empty() || !b.granted.is_empty() {
        let mut blocks = Vec::new();
        let slots: Vec<String> = (1..=9)
            .filter(|&l| play::slot_max(ev, l) > 0)
            .map(|l| format!("level {l}: {}/{}", play::slots_left(ch, ev, l), play::slot_max(ev, l)))
            .chain((int("pact.slots.max") > 0).then(|| {
                let level = b.model.inputs.get("pact.level").map(|v| v.to_string()).unwrap_or_default();
                format!("pact: {}/{} at level {level}", int("pact.slots.max") - ch.state.pact_slots_used, int("pact.slots.max"))
            }))
            .collect();
        if !slots.is_empty() {
            blocks.push(Block::Facts(vec![("Spell slots".into(), slots.join(" · "))]));
        }
        for c in &casters {
            let mine = ch.spells.classes.get(&c.id).cloned().unwrap_or_default();
            let mut facts = vec![(
                format!("{} {}", c.name, c.level),
                format!("save DC {} · attack {}", int(&format!("spell.{}.dc", c.slug)), signed(int(&format!("spell.{}.attack", c.slug)))),
            )];
            for (label, list) in [("Cantrips", &mine.cantrips), ("Prepared", &mine.prepared), ("Spellbook", &mine.spellbook)] {
                if !list.is_empty() {
                    facts.push((label.into(), spell_names(list).join(", ")));
                }
            }
            blocks.push(Block::Facts(facts));
        }
        for g in &b.granted {
            let mut ids = Vec::new();
            for (i, grant) in g.grants.iter().enumerate() {
                match grant {
                    Granted::Spell { id, .. } => ids.push(id.clone()),
                    Granted::Choice { .. } => ids.extend(ch.spells.chosen.get(&format!("{}#{i}", g.key)).cloned().unwrap_or_default()),
                }
            }
            if !ids.is_empty() {
                blocks.push(Block::Facts(vec![(g.origin.clone(), spell_names(&ids).join(", "))]));
            }
        }
        sections.push(Section { title: "Spellcasting".into(), blocks });
    }

    // Inventory: containers with their contents, worn and held items, coins.
    let mut rows = Vec::new();
    for (i, depth) in equipment::tree(ch, &Default::default()) {
        let line = &ch.inventory[i];
        let mut state = Vec::new();
        if let Some(s) = line.slot.filter(|_| line.equipped) {
            state.push(format!("worn ({})", s.label().to_lowercase()));
        }
        if let Some(h) = line.hand {
            state.push(match h {
                Hand::Pair => "one in each hand".to_string(),
                h => h.label().to_string(),
            });
        }
        if line.attuned {
            state.push("attuned".into());
        }
        if let Some(k) = equipment::container(lib, line) {
            let held = equipment::contents_weight(lib, ch, i);
            state.push(match k.capacity {
                Some(cap) => format!("holds {held}/{cap} lb"),
                None => format!("holds {held} lb"),
            });
            if k.weightless {
                state.push("weightless".into());
            }
        }
        let w = equipment::weight(lib, line) * line.quantity as f64;
        rows.push(vec![
            format!("{}{}", "  ".repeat(depth), line_name(lib, line)),
            line.quantity.to_string(),
            if w > 0.0 { format!("{w}") } else { String::new() },
            state.join(", "),
        ]);
    }
    let c = &ch.currency;
    let coins: Vec<String> = [(c.pp, "pp"), (c.gp, "gp"), (c.ep, "ep"), (c.sp, "sp"), (c.cp, "cp")]
        .iter()
        .filter(|(n, _)| *n != 0)
        .map(|(n, l)| format!("{n} {l}"))
        .collect();
    sections.push(Section {
        title: "Inventory".into(),
        blocks: vec![
            Block::Table { head: vec!["Item".into(), "Qty".into(), "lb".into(), "".into()], rows },
            Block::Facts(vec![
                ("Carried".into(), format!("{} / {} lb", equipment::carried_weight(lib, ch), int("str") * 15)),
                ("Coins".into(), if coins.is_empty() { "none".into() } else { coins.join(", ") }),
            ]),
        ],
    });

    // Features with their text.
    let mut blocks = Vec::new();
    for f in &b.features {
        let e = lib.entity(f.entity);
        let group = match f.source {
            FeatureSource::Class(ci) | FeatureSource::Subclass(ci) => b.classes.get(ci).map(|c| c.name.clone()).unwrap_or_default(),
            FeatureSource::Species => "Species".into(),
            FeatureSource::Background => "Background".into(),
            FeatureSource::Feat => "Feat".into(),
            FeatureSource::Option => "Option".into(),
            FeatureSource::Mastery => "Weapon Mastery".into(),
        };
        let level = if f.level > 0 { format!(" {}", f.level) } else { String::new() };
        let text = e.data.get("entries").map(|v| doc::plain_text(&doc::from_entries(v))).unwrap_or_default();
        blocks.push(Block::Detail { name: format!("{} ({group}{level})", e.name), text: text.trim().to_string() });
    }
    sections.push(Section { title: "Features".into(), blocks });

    Document { title: ch.name.clone(), subtitle: subtitle.join(" · "), sections }
}

fn md_cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

pub fn to_markdown(d: &Document) -> String {
    let mut out = format!("# {}\n\n*{}*\n", d.title, d.subtitle);
    for s in &d.sections {
        out.push_str(&format!("\n## {}\n\n", s.title));
        for b in &s.blocks {
            match b {
                Block::Facts(facts) => {
                    for (k, v) in facts {
                        out.push_str(&format!("- **{k}:** {v}\n"));
                    }
                    out.push('\n');
                }
                Block::Table { head, rows } => {
                    out.push_str(&format!("| {} |\n", head.iter().map(|h| md_cell(h)).collect::<Vec<_>>().join(" | ")));
                    out.push_str(&format!("|{}\n", head.iter().map(|_| " --- |").collect::<String>()));
                    for r in rows {
                        // Leading spaces (nesting) would be lost: use non-breaking ones.
                        let cells: Vec<String> = r.iter().map(|c| md_cell(&c.replacen("  ", "\u{a0}\u{a0}", c.len()))).collect();
                        out.push_str(&format!("| {} |\n", cells.join(" | ")));
                    }
                    out.push('\n');
                }
                Block::List(items) => {
                    for i in items {
                        out.push_str(&format!("- {i}\n"));
                    }
                    out.push('\n');
                }
                Block::Detail { name, text } => {
                    out.push_str(&format!("### {name}\n\n"));
                    if !text.is_empty() {
                        out.push_str(&format!("{text}\n\n"));
                    }
                }
                Block::Text(t) => out.push_str(&format!("{t}\n\n")),
            }
        }
    }
    out
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

pub fn to_html(d: &Document) -> String {
    let mut body = format!("<h1>{}</h1>\n<p class=\"sub\">{}</p>\n", esc(&d.title), esc(&d.subtitle));
    for s in &d.sections {
        body.push_str(&format!("<section>\n<h2>{}</h2>\n", esc(&s.title)));
        for b in &s.blocks {
            match b {
                Block::Facts(facts) => {
                    body.push_str("<dl>\n");
                    for (k, v) in facts {
                        body.push_str(&format!("<dt>{}</dt><dd>{}</dd>\n", esc(k), esc(v)));
                    }
                    body.push_str("</dl>\n");
                }
                Block::Table { head, rows } => {
                    body.push_str("<table>\n<tr>");
                    for h in head {
                        body.push_str(&format!("<th>{}</th>", esc(h)));
                    }
                    body.push_str("</tr>\n");
                    for r in rows {
                        body.push_str("<tr>");
                        for c in r {
                            let depth = c.len() - c.trim_start().len();
                            body.push_str(&format!(
                                "<td style=\"padding-left:{}em\">{}</td>",
                                0.4 + depth as f32 * 0.6,
                                esc(c.trim_start())
                            ));
                        }
                        body.push_str("</tr>\n");
                    }
                    body.push_str("</table>\n");
                }
                Block::List(items) => {
                    body.push_str("<ul>\n");
                    for i in items {
                        body.push_str(&format!("<li>{}</li>\n", esc(i)));
                    }
                    body.push_str("</ul>\n");
                }
                Block::Detail { name, text } => {
                    body.push_str(&format!("<div class=\"feature\"><h3>{}</h3>", esc(name)));
                    for para in text.split("\n\n").filter(|p| !p.trim().is_empty()) {
                        body.push_str(&format!("<p>{}</p>", esc(para.trim())));
                    }
                    body.push_str("</div>\n");
                }
                Block::Text(t) => body.push_str(&format!("<p>{}</p>\n", esc(t))),
            }
        }
        body.push_str("</section>\n");
    }
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<style>
  body {{ font: 10.5pt/1.4 Georgia, "Times New Roman", serif; color: #1b1b1b; background: #fff; max-width: 52em; margin: 2em auto; padding: 0 1em; }}
  h1 {{ margin: 0; font-size: 1.9em; }}
  .sub {{ margin: .2em 0 1.2em; color: #555; font-style: italic; }}
  h2 {{ font-size: 1.15em; border-bottom: 2px solid #8a1c1c; color: #8a1c1c; margin: 1.4em 0 .5em; text-transform: uppercase; letter-spacing: .05em; }}
  h3 {{ font-size: 1em; margin: .8em 0 .2em; }}
  table {{ border-collapse: collapse; width: 100%; margin: .3em 0 .8em; }}
  th, td {{ border-bottom: 1px solid #ddd; padding: .2em .4em; text-align: left; vertical-align: top; }}
  th {{ font-size: .85em; color: #666; font-weight: normal; text-transform: uppercase; }}
  dl {{ display: grid; grid-template-columns: max-content 1fr; gap: .15em 1em; margin: .3em 0 .8em; }}
  dt {{ font-weight: bold; }} dd {{ margin: 0; }}
  ul {{ columns: 2; margin: .3em 0; }}
  .feature p {{ margin: .2em 0; }}
  section, .feature, tr {{ break-inside: avoid; }}
  @media print {{ body {{ margin: 0; max-width: none; }} }}
</style>
</head>
<body>
{body}</body>
</html>
"#,
        title = esc(&d.title)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use dnt_rules::character::{Character, InventoryItem};
    use dnt_rules::overlay::OverlaySet;
    use dnt_store::CharacterFile;
    use serde_json::json;

    #[test]
    fn exports_the_sheet() {
        let mut lib = Library::new();
        lib.load_json(&json!({"baseitem": [{"name": "Dagger", "source": "T", "type": "M|T", "weapon": true, "weaponCategory": "simple",
                                             "property": ["F|T"], "dmg1": "1d4", "dmgType": "P", "weight": 1}]}));
        let c = Character {
            name: "Ana <the Bold>".into(),
            inventory: vec![InventoryItem { item: Some("item:Dagger|T".into()), quantity: 2, ..Default::default() }],
            ..Character::default()
        };
        let file = CharacterFile { slug: "ana".into(), path: "/tmp/ana.toml".into(), character: c };
        let sheet = CharacterSession::new(file, &lib, &OverlaySet::default(), &Default::default());
        let d = character(&lib, &sheet);
        let md = to_markdown(&d);
        assert!(md.starts_with("# Ana <the Bold>"));
        for s in [
            "## Abilities",
            "## Skills",
            "## Attacks",
            "## Inventory",
            "| Dagger | +0 | 1d4 piercing | 5 ft. | Finesse, not proficient |",
            "**Armor Class:** 10",
        ] {
            assert!(md.contains(s), "{s} missing in\n{md}");
        }
        let html = to_html(&d);
        assert!(html.contains("<h1>Ana &lt;the Bold&gt;</h1>") && html.contains("<th>Attack</th>"));
    }
}
