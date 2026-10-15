//! schemas/overlay.schema.json describes the rules files: the built-in
//! rules must match it, and it must catch common mistakes.

use serde_json::{json, Value};

fn validator() -> jsonschema::Validator {
    let path = format!("{}/../../schemas/overlay.schema.json", env!("CARGO_MANIFEST_DIR"));
    let schema: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    jsonschema::validator_for(&schema).unwrap()
}

#[test]
fn built_in_rules_match_the_schema() {
    let v = validator();
    for (name, text) in dnt_rules::builtin::FILES {
        let doc: toml::Value = toml::from_str(text).unwrap();
        let doc = serde_json::to_value(doc).unwrap();
        let errors: Vec<String> = v.iter_errors(&doc).map(|e| format!("{e} at {}", e.instance_path())).collect();
        assert!(errors.is_empty(), "{name}:\n{}", errors.join("\n"));
    }
}

#[test]
fn mistakes_are_caught() {
    let v = validator();
    for (what, doc) in [
        ("two operations", json!({"feature": [{"id": "feat:X|T", "effect": [{"target": "ac", "add": "1", "set": "2"}]}]})),
        ("no operation", json!({"feature": [{"id": "feat:X|T", "effect": [{"target": "ac"}]}]})),
        (
            "unknown recharge",
            json!({"feature": [{"id": "feat:X|T", "resource": [{"key": "x", "name": "X", "max": "1", "recharge": "weekly"}]}]}),
        ),
        ("misspelled field", json!({"feature": [{"id": "feat:X|T", "effects": []}]})),
        ("no id", json!({"feature": [{"name": "X"}]})),
    ] {
        assert!(!v.is_valid(&doc), "{what} should be rejected");
    }
    assert!(v.is_valid(&json!({"feature": [{"id": "feat:X|T", "effect": [{"target": "speed", "mul": "1 / 2", "when": "toggle('x')"}]}]})));
}
