//! Overlays shipped with dnt: the SRD 5.2.1's. Rules for other books are
//! in the rules library (see `dnt_core::rules_library`).

use crate::overlay::{Diagnostic, Overlay, OverlaySet};

/// (file name, contents) of every built-in overlay.
pub const FILES: &[(&str, &str)] =
    &[("core.toml", include_str!("../overlays/core.toml")), ("srd.toml", include_str!("../overlays/srd.toml"))];

/// Parse all built-in overlays. Diagnostics indicate a bug in dnt itself.
pub fn overlays() -> (OverlaySet, Vec<Diagnostic>) {
    let mut set = OverlaySet::default();
    let mut diags = Vec::new();
    for (name, text) in FILES {
        match Overlay::parse(text) {
            Ok((overlay, d)) => {
                set.add(overlay);
                diags.extend(d.into_iter().map(|d| Diagnostic { location: format!("{name}: {}", d.location), message: d.message }));
            }
            Err(e) => diags.push(Diagnostic { location: name.to_string(), message: e }),
        }
    }
    (set, diags)
}

/// The built-in TOML block for an entity, if there is one (a starting
/// point for a user's version).
pub fn source_for(key: &dnt_data::Key) -> Option<String> {
    for (_, text) in FILES {
        // Split into `[[feature]]` blocks (sub-tables stay with their feature).
        let mut blocks: Vec<String> = Vec::new();
        for line in text.lines() {
            if line.trim() == "[[feature]]" || blocks.is_empty() {
                blocks.push(String::new());
            }
            let b = blocks.last_mut().unwrap();
            b.push_str(line);
            b.push('\n');
        }
        for b in blocks {
            let Ok((overlay, _)) = Overlay::parse(&b) else { continue };
            if overlay.features.iter().any(|f| f.keys.contains(key)) {
                // Drop the next section's heading comment.
                let kept: Vec<&str> = b.trim_end().lines().collect();
                let end = kept.iter().rposition(|l| !l.trim_start().starts_with('#') && !l.trim().is_empty()).map_or(0, |i| i + 1);
                return Some(kept[..end].join("\n") + "\n");
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    #[test]
    fn builtins_parse_cleanly() {
        let (set, diags) = super::overlays();
        assert!(diags.is_empty(), "{diags:#?}");
        assert!(set.len() > 20);
    }

    #[test]
    fn source_blocks() {
        let key = dnt_data::Key::new(dnt_data::Kind::Feat, &["Alert", "SRD52"]);
        let block = super::source_for(&key).unwrap();
        assert!(block.starts_with("[[feature]]\nid = \"feat:Alert|SRD52\""), "{block}");
        assert!(!block.contains("Tough"));
    }

    #[test]
    fn only_the_srd_is_built_in() {
        use dnt_data::{Key, Kind};
        let (set, _) = super::overlays();
        let focus = Key::new(Kind::ClassFeature, &["Monk’s Focus", "Monk", "SRD52", "2", "SRD52"]);
        assert!(!set.for_entity(&focus).is_empty());
        assert!(set.for_entity(&Key::new(Kind::Feat, &["Alert", "XPHB"])).is_empty(), "the rules library has the books'");
    }
}
