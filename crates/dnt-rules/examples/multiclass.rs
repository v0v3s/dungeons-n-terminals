//! Build a 4-class character from real data data and print the result.
//!
//! cargo run -p dnt-rules --example multiclass -- /path/to/dnd-data

use dnt_data::Library;
use dnt_rules::builder::build;
use dnt_rules::character::Character;
use dnt_rules::overlay::OverlaySet;

const CHARACTER: &str = r#"
name = "Kitchen Sink"
species = "race:Human|XPHB"
background = "background:Soldier|XPHB"

[abilities]
str = 15
dex = 10
con = 14
int = 13
wis = 8
cha = 15

[proficiencies]
skills = ["athletics", "intimidation", "persuasion"]

[[levels]]
class = "class:Paladin|XPHB"
[[levels]]
class = "class:Paladin|XPHB"
[[levels]]
class = "class:Paladin|XPHB"
[[levels]]
class = "class:Sorcerer|XPHB"
[[levels]]
class = "class:Warlock|XPHB"
[[levels]]
class = "class:Warlock|XPHB"
[[levels]]
class = "class:Fighter|XPHB"
[[levels]]
class = "class:Fighter|XPHB"
[[levels]]
class = "class:Fighter|XPHB"

[subclasses]
"class:Fighter|XPHB" = "subclass:Eldritch Knight|Fighter|XPHB|XPHB"
"class:Paladin|XPHB" = "subclass:Glory|Paladin|XPHB|XPHB"

[equipped]
armor = "item:Chain Mail|XPHB"
shield = true
"#;

fn main() {
    let path = std::env::args().nth(1).expect("usage: multiclass <data dir>");
    let lib = Library::load_data_dir(path.as_ref()).expect("load");
    let character = Character::from_toml(CHARACTER).expect("character");
    let b = build(&character, &lib, &OverlaySet::default());
    println!("{} — {}", character.name, b.class_summary());
    for w in &b.warnings {
        println!("  warning: {w}");
    }
    let ev = b.evaluate();
    for (k, r) in &ev.stats {
        let show = k.starts_with("slots.")
            || k.starts_with("pact.")
            || k.starts_with("spell.")
            || k.starts_with("hit_dice.")
            || matches!(k.as_str(), "hp.max" | "ac" | "prof" | "save.wis" | "save.cha" | "speed");
        if show {
            println!("  {k:<22} {}", r.explain());
        }
    }
    println!("  pact.level             {:?}", b.model.inputs.get("pact.level").map(|v| v.to_string()));
    println!("  features: {}", b.features.len());
    for f in b.features.iter().take(40) {
        println!("    {:?} L{} {}", f.source, f.level, lib.entity(f.entity).name);
    }
}
