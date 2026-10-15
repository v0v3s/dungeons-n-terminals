//! Print the choices each class grants at given levels.
//!
//! cargo run -p dnt-rules --example choices -- /path/to/data

use dnt_data::{Library, SourceFilter};
use dnt_rules::character::Character;
use dnt_rules::choices::level_choices;
use dnt_rules::overlay::resolve_id;

fn main() {
    let lib = Library::load_dir(std::env::args().nth(1).expect("usage: choices <data dir>").as_ref()).expect("load");
    let filter = SourceFilter::only(["XPHB", "PHB", "XGE", "TCE"]);
    let skills: Vec<String> = ["stealth", "perception", "athletics"].map(String::from).to_vec();
    for (class, sub, level) in [
        ("class:Fighter|XPHB", None, 1),
        ("class:Fighter|XPHB", None, 4),
        ("class:Cleric|XPHB", None, 1),
        ("class:Druid|XPHB", None, 1),
        ("class:Warlock|XPHB", None, 1),
        ("class:Warlock|XPHB", None, 2),
        ("class:Rogue|XPHB", None, 1),
        ("class:Ranger|XPHB", None, 2),
        ("class:Fighter|PHB", Some("subclass:Battle Master|Fighter|PHB|PHB"), 3),
        ("class:Fighter|PHB", None, 1),
    ] {
        let class = resolve_id(&lib, class).expect(class);
        let subclass = sub.and_then(|s| resolve_id(&lib, s));
        let choices = level_choices(&lib, &Character::default(), class, subclass, level, &filter, &skills);
        let text: Vec<String> = choices
            .iter()
            .map(|c| {
                format!(
                    "{} ×{} [{} options: {}…]",
                    c.key,
                    c.count,
                    c.options.len(),
                    c.options.iter().take(3).map(|o| o.label.as_str()).collect::<Vec<_>>().join(", ")
                )
            })
            .collect();
        println!("{} {level}: {}", lib.entity(class).name, if text.is_empty() { "—".into() } else { text.join(" | ") });
    }
}
