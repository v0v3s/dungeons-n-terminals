//! Build a level-5 Monk (2024) from a real data directory plus an overlay, and
//! print the computed stats with their breakdowns.
//!
//! cargo run -p dnt-rules --example sheet -- /path/to/data

use dnt_data::{Key, Kind, Library};
use dnt_rules::class_table::class_columns;
use dnt_rules::overlay::Overlay;
use dnt_rules::{standard, Model, Value};

const OVERLAY: &str = r#"
[[feature]]
id = "classFeature:Unarmored Defense|Monk|XPHB|1"
[[feature.effect]]
target = "ac"
max = "10 + mod.dex + mod.wis"
when = "armor.none and armor.shield == 0"

[[feature]]
id = "classFeature:Martial Arts|Monk|XPHB|1"
[[feature.effect]]
target = "attack.unarmed.damage"
set = "class.monk.martial_arts + mod.dex"

[[feature]]
id = "classFeature:Monk's Focus|Monk|XPHB|2"
[[feature.resource]]
key = "focus"
name = "Focus Points"
max = "class.monk.focus_points"
recharge = "short_rest"

[[feature]]
id = "classFeature:Unarmored Movement|Monk|XPHB|2"
[[feature.effect]]
target = "speed"
add = "class.monk.unarmored_movement"
when = "armor.none"
"#;

fn main() {
    let path = std::env::args().nth(1).expect("usage: sheet <data dir>");
    let lib = Library::load_dir(path.as_ref()).expect("load");
    let monk = lib.get(&Key::new(Kind::Class, &["Monk", "XPHB"])).expect("XPHB Monk");
    let level = 5;

    let mut model = Model { stats: standard::stats(), inputs: standard::default_inputs(), ..Model::default() };
    for (k, v) in [("level", 5), ("base.dex", 16), ("base.wis", 15), ("base.con", 14), ("hp.base", 8 + 5 * 4)] {
        model.inputs.insert(k.into(), Value::int(v));
    }
    model.inputs.insert("prof.save.str".into(), Value::int(1));
    model.inputs.insert("prof.save.dex".into(), Value::int(1));
    model.class_levels.insert("monk".into(), level);

    println!("Class table variables at level {level}:");
    for col in class_columns(&lib.entity(monk).data) {
        println!("  {:<36} {}", col.var, col.at(level as usize));
        model.inputs.insert(col.var.clone(), col.at(level as usize));
    }

    let (overlay, diags) = Overlay::parse(OVERLAY).expect("overlay TOML");
    for d in &diags {
        println!("overlay: {d}");
    }
    for id in overlay.unresolved(&lib) {
        println!("overlay: unknown feature {id}");
    }
    for f in &overlay.features {
        f.apply(&mut model);
    }

    let ev = model.evaluate();
    println!("\nStats:");
    for key in [
        "prof",
        "ac",
        "hp.max",
        "speed",
        "initiative",
        "save.dex",
        "skill.stealth",
        "passive.perception",
        "attack.unarmed.damage",
        "resource.focus.max",
    ] {
        println!("  {key:<24} {}", ev.stats[key].explain());
    }
    for (k, e) in ev.errors() {
        println!("  ERROR {k}: {e}");
    }
}
