//! The JSON schemas in `schemas/` describe what dnt reads: they must accept
//! the shapes the loader understands and everything dnt writes itself.

use dnt_data::homebrew::{self, CloneMode, Template};
use dnt_data::{Key, Kind, Library};
use serde_json::{json, Value};

fn schema(name: &str) -> jsonschema::Validator {
    let path = format!("{}/../../schemas/{name}", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let schema: Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}"));
    jsonschema::validator_for(&schema).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn assert_valid(v: &jsonschema::Validator, doc: &Value, what: &str) {
    let errors: Vec<String> = v.iter_errors(doc).map(|e| format!("{} at {}", e, e.instance_path())).collect();
    assert!(errors.is_empty(), "{what}:\n{}", errors.join("\n"));
}

/// One of each kind, as dnt reads them.
fn sample() -> Value {
    json!({
        "_meta": {"sources": [{"json": "HB", "abbreviation": "HB", "full": "Homebrew", "version": "1"}]},
        "class": [{
            "name": "Blade", "source": "HB", "edition": "one", "hd": {"number": 1, "faces": 10},
            "proficiency": ["str", "con"], "spellcastingAbility": "int", "casterProgression": "1/3",
            "startingProficiencies": {"weapons": ["simple", "martial"], "skills": [{"choose": {"from": ["athletics", "history"], "count": 2}}]},
            "multiclassing": {"requirements": {"or": [{"str": 13, "dex": 13}]}, "proficienciesGained": {"weapons": ["martial"]}},
            "classTableGroups": [{"colLabels": ["Rages"], "rows": [[2], [2]]}],
            "classFeatures": ["Cut|Blade|HB|1", {"classFeature": "Path|Blade|HB|3", "gainSubclassFeature": true}]
        }],
        "subclass": [{"name": "Path of Edges", "shortName": "Edges", "source": "HB", "className": "Blade", "classSource": "HB",
                      "subclassFeatures": ["Edges|Blade|HB|Edges|HB|3"],
                      "additionalSpells": [{"expanded": {"s1": ["shield"], "3": [{"all": "level=1|class=Wizard"}]}}]}],
        "classFeature": [{"name": "Cut", "source": "HB", "className": "Blade", "classSource": "HB", "level": 1,
                          "entries": ["You cut. {@damage 1d6} slashing.", {"type": "entries", "name": "More", "entries": ["Text."]}]}],
        "subclassFeature": [{"name": "Edges", "source": "HB", "className": "Blade", "classSource": "HB",
                             "subclassShortName": "Edges", "subclassSource": "HB", "level": 3, "entries": ["x"]}],
        "spell": [{
            "name": "Spark", "source": "HB", "level": 0, "school": "V",
            "time": [{"number": 1, "unit": "action"}],
            "range": {"type": "point", "distance": {"type": "feet", "amount": 60}},
            "components": {"v": true, "s": true, "m": {"text": "a flint", "cost": 10}},
            "duration": [{"type": "timed", "duration": {"type": "minute", "amount": 1}, "concentration": true}],
            "meta": {"ritual": false}, "spellAttack": ["R"], "damageInflict": ["fire"],
            "scalingLevelDice": {"label": "Fire damage", "scaling": {"1": "1d10", "5": "2d10"}},
            "classes": {"fromClassList": [{"name": "Wizard", "source": "XPHB"}]},
            "entries": ["Deals {@damage 1d10} Fire damage."]
        }],
        "race": [{"name": "Stoneborn", "source": "HB", "size": ["M"], "speed": {"walk": 30}, "darkvision": 60,
                  "ability": [{"con": 2}], "additionalSpells": [{"innate": {"3": {"daily": {"1": ["shield"]}}}}]}],
        "subrace": [{"name": "Deep", "source": "HB", "raceName": "Stoneborn", "raceSource": "HB"}],
        "background": [{"name": "Miner", "source": "HB", "skillProficiencies": [{"athletics": true}], "ability": [{"choose": {"weighted": {"from": ["str", "con"], "weights": [2, 1]}}}]}],
        "feat": [{"name": "Tough Hide", "source": "HB", "category": "G", "prerequisite": [{"level": 4}], "entries": ["x"]}],
        "optionalfeature": [{"name": "Glint", "source": "HB", "featureType": ["EI"],
                             "prerequisite": [{"level": {"level": 5, "class": {"name": "Warlock"}}, "optionalfeature": ["pact of the blade|xphb"]}],
                             "entries": ["x"]}],
        "baseitem": [{"name": "Pick", "source": "HB", "type": "M|HB", "weapon": true, "weaponCategory": "martial",
                      "dmg1": "1d8", "dmgType": "P", "property": ["V|HB", {"uid": "F|HB"}], "mastery": ["Sap|XPHB"], "weight": 4, "value": 500}],
        "item": [
            {"name": "Sack of Stones", "source": "HB", "type": "G", "rarity": "none", "weight": 0.5,
             "containerCapacity": {"weight": [30], "weightless": false}},
            {"name": "Miner's Pack", "source": "HB", "packContents": ["sack of stones|hb", {"item": "pick|hb", "quantity": 2}, {"special": "a lamp"}]},
            {"name": "+1 Pick", "source": "HB", "baseItem": "Pick|HB", "bonusWeapon": "+1", "reqAttune": "by a miner"}
        ],
        "magicvariant": [{"name": "+1 Weapon", "requires": [{"weapon": true}], "excludes": {"net": true},
                          "inherits": {"namePrefix": "+1 ", "source": "HB", "bonusWeapon": "+1", "entries": ["A {=bonusWeapon} bonus."]}}],
        "condition": [{"name": "Dusty", "source": "HB", "entries": ["Sneezing."]}],
        "skill": [{"name": "Mining", "source": "HB", "ability": "str", "entries": ["x"]}],
        "variantrule": [{"name": "Cave-ins", "source": "HB", "entries": [{"type": "table", "colLabels": ["d6", "Result"], "rows": [["1", "Rocks"]]}]}]
    })
}

#[test]
fn schemas_compile_and_accept_the_shapes_dnt_reads() {
    let data = schema("data-file.schema.json");
    let doc = sample();
    assert_valid(&data, &doc, "sample document");
    // dnt also loads it.
    let mut lib = Library::new();
    lib.load_json(&doc);
    assert!(lib.get(&Key::new(Kind::Item, &["+1 Pick", "HB"])).is_some());

    assert_valid(
        &schema("books.schema.json"),
        &json!({"book": [{"name": "Homebrew", "id": "HB", "source": "HB", "group": "core", "published": "2026-01-01"}]}),
        "books",
    );
    assert_valid(&schema("index.schema.json"), &json!({"HB": "spells-hb.json"}), "index");
    assert_valid(
        &schema("spell-classes.schema.json"),
        &json!({"HB": {"Spark": {"class": [{"name": "Wizard", "source": "XPHB"}]}}}),
        "spell classes",
    );
}

#[test]
fn what_dnt_writes_is_valid() {
    let data = schema("data-file.schema.json");
    // New entries from every template.
    let mut doc = json!({"_meta": {"sources": [{"json": "CoS-HB", "full": "Curse of Strahd (campaign)"}]}});
    for t in Template::ALL {
        homebrew::add_entry(&mut doc, t.kind(), t.entry(&format!("New {}", t.label()), "CoS-HB"));
    }
    // Clones: a replacing version and a separate copy.
    let lib = {
        let mut l = Library::new();
        l.load_json(&sample());
        l
    };
    let spark = lib.get(&Key::new(Kind::Spell, &["Spark", "HB"])).unwrap();
    homebrew::add_entry(&mut doc, Kind::Spell, homebrew::clone_entry(&lib, spark, "CoS-HB", &CloneMode::Replace));
    homebrew::add_entry(&mut doc, Kind::Spell, homebrew::clone_entry(&lib, spark, "CoS-HB", &CloneMode::Copy { name: "Spark II".into() }));
    let cut = lib.get(&Key::new(Kind::ClassFeature, &["Cut", "Blade", "HB", "1", "HB"])).unwrap();
    homebrew::add_entry(&mut doc, Kind::ClassFeature, homebrew::clone_entry(&lib, cut, "CoS-HB", &CloneMode::Replace));
    assert_valid(&data, &doc, "campaign homebrew written by dnt");
}

#[test]
fn schemas_reject_broken_entities() {
    let data = schema("data-file.schema.json");
    for (what, doc) in [
        ("no name", json!({"spell": [{"source": "HB", "level": 1}]})),
        ("no source", json!({"feat": [{"name": "Nameless"}]})),
        ("bad level", json!({"spell": [{"name": "X", "source": "HB", "level": 12}]})),
        ("feature without its class", json!({"classFeature": [{"name": "X", "source": "HB", "level": 1}]})),
        ("list of the wrong thing", json!({"item": {"name": "X"}})),
    ] {
        assert!(!data.is_valid(&doc), "{what} should be rejected");
    }
}
