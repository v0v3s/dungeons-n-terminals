//! Resolution of data `_copy` inheritance: an entity declares
//! `"_copy": {"name", "source", "_mod": {...}, "_preserve": {...}}` and is
//! built from the referenced base plus modifications.

use regex::RegexBuilder;
use serde_json::{Map, Value};

/// Properties not inherited from the base unless listed in `_preserve`.
const NOT_INHERITED: &[&str] = &[
    "page",
    "otherSources",
    "additionalSources",
    "srd",
    "srd52",
    "basicRules",
    "basicRules2024",
    "reprintedAs",
    "isReprinted",
    "hasFluff",
    "hasFluffImages",
    "hasToken",
    "_versions",
    "legacy",
];

/// Merge `copy` (an entity with a `_copy` block) onto `base`.
pub fn merge(base: &Value, copy: &Value) -> Value {
    let (Some(base), Some(copy)) = (base.as_object(), copy.as_object()) else {
        return copy.clone();
    };
    let meta = copy.get("_copy").and_then(Value::as_object);
    let preserve = meta.and_then(|m| m.get("_preserve")).and_then(Value::as_object);

    let mut out: Map<String, Value> = base
        .iter()
        .filter(|(k, _)| {
            !NOT_INHERITED.contains(&k.as_str()) || preserve.is_some_and(|p| p.contains_key(*k) || p.contains_key("*"))
        })
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    for (k, v) in copy {
        if k != "_copy" {
            out.insert(k.clone(), v.clone());
        }
    }
    // `null` in the copy means "remove this inherited property".
    out.retain(|_, v| !v.is_null());

    let mut out = Value::Object(out);
    if let Some(mods) = meta.and_then(|m| m.get("_mod")).and_then(Value::as_object) {
        for (prop, ops) in mods {
            for op in one_or_many(ops) {
                apply_mod(&mut out, prop, op);
            }
        }
    }
    out
}

fn one_or_many(v: &Value) -> Vec<&Value> {
    match v {
        Value::Array(a) => a.iter().collect(),
        other => vec![other],
    }
}

fn apply_mod(root: &mut Value, prop: &str, op: &Value) {
    let op = match op {
        // Shorthand: `"prop": "remove"`.
        Value::String(s) if s == "remove" => {
            if let Some(o) = root.as_object_mut() {
                o.remove(prop);
            }
            return;
        }
        Value::Object(_) => op,
        _ => return,
    };
    let mode = op.get("mode").and_then(Value::as_str).unwrap_or("");
    let items: Vec<Value> = op.get("items").map(|v| one_or_many(v).into_iter().cloned().collect()).unwrap_or_default();

    match mode {
        "replaceTxt" => {
            if prop == "*" {
                if let Some(o) = root.as_object_mut() {
                    for (k, v) in o.iter_mut() {
                        if TEXT_PROPS.contains(&k.as_str()) {
                            replace_txt(v, op);
                        }
                    }
                }
            } else if let Some(v) = root.get_mut(prop) {
                replace_txt(v, op);
            }
        }
        "setProp" => {
            if let (Some(path), Some(value)) = (op.get("prop").and_then(Value::as_str), op.get("value")) {
                let base = if prop == "_" { root } else { entry_or_default(root, prop, Value::Object(Map::new())) };
                set_path(base, path, value.clone());
            }
        }
        "appendArr" | "prependArr" | "insertArr" | "appendIfNotExistsArr" | "replaceArr" | "replaceOrAppendArr"
        | "removeArr" | "renameArr" => {
            let arr = entry_or_default(root, prop, Value::Array(Vec::new()));
            if !arr.is_array() {
                *arr = Value::Array(vec![arr.take()]);
            }
            let Value::Array(arr) = arr else { unreachable!() };
            array_mod(arr, mode, op, items);
        }
        // Creature-specific and numeric modes are not needed for player data.
        _ => {}
    }
}

fn entry_or_default<'a>(root: &'a mut Value, prop: &str, default: Value) -> &'a mut Value {
    if !root.is_object() {
        *root = Value::Object(Map::new());
    }
    root.as_object_mut().unwrap().entry(prop.to_string()).or_insert(default)
}

fn array_mod(arr: &mut Vec<Value>, mode: &str, op: &Value, items: Vec<Value>) {
    match mode {
        "appendArr" => arr.extend(items),
        "prependArr" => {
            arr.splice(0..0, items);
        }
        "insertArr" => {
            let idx = op.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
            let idx = idx.min(arr.len());
            arr.splice(idx..idx, items);
        }
        "appendIfNotExistsArr" => {
            for item in items {
                if !arr.contains(&item) {
                    arr.push(item);
                }
            }
        }
        "replaceArr" | "replaceOrAppendArr" => {
            let found = op.get("replace").and_then(|r| find_index(arr, r));
            match found {
                Some(i) => {
                    arr.splice(i..=i, items);
                }
                None if mode == "replaceOrAppendArr" => arr.extend(items),
                None => {}
            }
        }
        "removeArr" => {
            if let Some(names) = op.get("names") {
                for name in one_or_many(names) {
                    if let Some(i) = find_index(arr, name) {
                        arr.remove(i);
                    }
                }
            }
            arr.retain(|v| !items.contains(v));
        }
        "renameArr" => {
            if let Some(renames) = op.get("renames") {
                for r in one_or_many(renames) {
                    let (Some(from), Some(to)) = (r.get("rename").and_then(Value::as_str), r.get("with")) else {
                        continue;
                    };
                    for v in arr.iter_mut() {
                        if v.get("name").and_then(Value::as_str) == Some(from) {
                            v["name"] = to.clone();
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

/// Find an array element by name/string equality, `{index}` or `{regex}`.
fn find_index(arr: &[Value], selector: &Value) -> Option<usize> {
    let matches_name = |v: &Value, name: &str| v.as_str() == Some(name) || v.get("name").and_then(Value::as_str) == Some(name);
    match selector {
        Value::String(name) => arr.iter().position(|v| matches_name(v, name)),
        Value::Object(o) => {
            if let Some(i) = o.get("index").and_then(Value::as_u64) {
                return ((i as usize) < arr.len()).then_some(i as usize);
            }
            let re = o.get("regex").and_then(Value::as_str)?;
            let re = RegexBuilder::new(re)
                .case_insensitive(o.get("flags").and_then(Value::as_str).is_some_and(|f| f.contains('i')))
                .build()
                .ok()?;
            arr.iter().position(|v| {
                v.as_str()
                    .or_else(|| v.get("name").and_then(Value::as_str))
                    .is_some_and(|s| re.is_match(s))
            })
        }
        _ => None,
    }
}

/// Props searched by a `"*"` replaceTxt.
const TEXT_PROPS: &[&str] = &["entries", "entriesHigherLevel", "items", "headerEntries", "footerEntries"];

fn replace_txt(target: &mut Value, op: &Value) {
    let Some(pattern) = op.get("replace").and_then(Value::as_str) else { return };
    let with = op.get("with").and_then(Value::as_str).unwrap_or("");
    let flags = op.get("flags").and_then(Value::as_str).unwrap_or("");
    let Ok(re) = RegexBuilder::new(pattern).case_insensitive(flags.contains('i')).build() else {
        return;
    };
    // JS replacement syntax `$1` matches the regex crate's; `$&` does not.
    let with = with.replace("$&", "${0}");
    let props: Vec<&str> = op
        .get("props")
        .and_then(Value::as_array)
        .map(|p| p.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    walk_strings(target, &mut |s| {
        if re.is_match(s) {
            *s = re.replace_all(s, with.as_str()).into_owned();
        }
    }, &props);
}

fn walk_strings(v: &mut Value, f: &mut dyn FnMut(&mut String), extra_props: &[&str]) {
    match v {
        Value::String(s) => f(s),
        Value::Array(a) => a.iter_mut().for_each(|x| walk_strings(x, f, extra_props)),
        Value::Object(o) => {
            for (k, x) in o.iter_mut() {
                if matches!(k.as_str(), "entries" | "entry" | "items" | "headerEntries" | "rows")
                    || extra_props.contains(&k.as_str())
                {
                    walk_strings(x, f, extra_props);
                }
            }
        }
        _ => {}
    }
}

fn set_path(root: &mut Value, path: &str, value: Value) {
    let mut cur = root;
    let mut parts = path.split('.').peekable();
    while let Some(part) = parts.next() {
        if !cur.is_object() {
            *cur = Value::Object(Map::new());
        }
        let obj = cur.as_object_mut().unwrap();
        if parts.peek().is_none() {
            obj.insert(part.to_string(), value);
            return;
        }
        cur = obj.entry(part.to_string()).or_insert_with(|| Value::Object(Map::new()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn inherits_and_overrides() {
        let base = json!({"name": "Goblin", "source": "MPMM", "page": 12, "size": ["S"], "entries": ["a", "b"]});
        let copy = json!({"name": "Boggart", "source": "LFL", "_copy": {"name": "Goblin", "source": "MPMM"}});
        let out = merge(&base, &copy);
        assert_eq!(out["name"], "Boggart");
        assert_eq!(out["size"], json!(["S"]));
        assert!(out.get("page").is_none());
        assert!(out.get("_copy").is_none());
    }

    #[test]
    fn array_mods() {
        let base = json!({"entries": [{"name": "A"}, {"name": "B"}, "plain"]});
        let copy = json!({"_copy": {"_mod": {"entries": [
            {"mode": "replaceArr", "replace": "A", "items": {"name": "A2"}},
            {"mode": "removeArr", "names": "B"},
            {"mode": "appendArr", "items": ["end"]},
            {"mode": "prependArr", "items": "start"}
        ]}}});
        let out = merge(&base, &copy);
        assert_eq!(out["entries"], json!(["start", {"name": "A2"}, "plain", "end"]));
    }

    #[test]
    fn replace_txt_everywhere() {
        let base = json!({"entries": ["the goblin", {"type": "entries", "name": "goblin", "entries": ["goblin!"]}]});
        let copy = json!({"_copy": {"_mod": {"*": {"mode": "replaceTxt", "replace": "goblin", "with": "boggart"}}}});
        let out = merge(&base, &copy);
        assert_eq!(out["entries"][0], "the boggart");
        assert_eq!(out["entries"][1]["entries"][0], "boggart!");
        // Names are not touched unless listed in `props`.
        assert_eq!(out["entries"][1]["name"], "goblin");
    }

    #[test]
    fn preserve_and_set_prop() {
        let base = json!({"reprintedAs": ["X|XPHB"], "speed": {"walk": 30}});
        let copy = json!({"_copy": {"_preserve": {"reprintedAs": true},
            "_mod": {"_": {"mode": "setProp", "prop": "speed.fly", "value": 30}}}});
        let out = merge(&base, &copy);
        assert_eq!(out["reprintedAs"], json!(["X|XPHB"]));
        assert_eq!(out["speed"], json!({"walk": 30, "fly": 30}));
    }
}
