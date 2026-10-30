//! Overlays shipped with dnt.

use crate::overlay::{Diagnostic, Overlay, OverlaySet};

/// (file name, contents) of every built-in overlay.
pub const FILES: &[(&str, &str)] = &[("xphb.toml", include_str!("../overlays/xphb.toml"))];

/// Parse all built-in overlays. Diagnostics indicate a bug in dnt itself.
pub fn overlays() -> (OverlaySet, Vec<Diagnostic>) {
    let mut set = OverlaySet::default();
    let mut diags = Vec::new();
    for (name, text) in FILES {
        match Overlay::parse(text) {
            Ok((mut overlay, d)) => {
                for f in &mut overlay.features {
                    f.keys.extend(srd_keys(&f.keys));
                }
                set.add(overlay);
                diags.extend(d.into_iter().map(|d| Diagnostic { location: format!("{name}: {}", d.location), message: d.message }));
            }
            Err(e) => diags.push(Diagnostic { location: name.to_string(), message: e }),
        }
    }
    (set, diags)
}

/// The SRD 5.2.1 has the 2024 books' rules: a rule for a 2024 book's
/// entry is the SRD's entry's rule too (`feat:Alert|XPHB` →
/// `feat:Alert|SRD52`; the SRD writes `’` where the books write `'`).
fn srd_keys(keys: &[dnt_data::Key]) -> Vec<dnt_data::Key> {
    let srd = dnt_data::srd::SOURCE.to_lowercase();
    let mut out = Vec::new();
    for k in keys {
        let parts: Vec<String> =
            k.path.split('|').map(|p| if matches!(p, "xphb" | "xdmg") { srd.clone() } else { p.to_string() }).collect();
        let path = parts.join("|");
        if path == k.path {
            continue;
        }
        for path in [path.clone(), path.replace('\'', "’")] {
            let key = dnt_data::Key { kind: k.kind, path };
            if !out.contains(&key) {
                out.push(key);
            }
        }
    }
    out
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
            let own = overlay.features.iter().any(|f| f.keys.contains(key));
            if own || overlay.features.iter().any(|f| srd_keys(&f.keys).contains(key)) {
                // Drop the next section's heading comment.
                let kept: Vec<&str> = b.trim_end().lines().collect();
                let end = kept.iter().rposition(|l| !l.trim_start().starts_with('#') && !l.trim().is_empty()).map_or(0, |i| i + 1);
                let mut lines: Vec<String> = kept[..end].iter().map(|l| l.to_string()).collect();
                if !own {
                    // The SRD's entry: its own id.
                    for l in lines.iter_mut().filter(|l| l.starts_with("id = ")) {
                        *l = l.replace("|XPHB", "|SRD52").replace("|XDMG", "|SRD52");
                        if key.path.contains('’') {
                            *l = l.replace('\'', "’");
                        }
                    }
                }
                return Some(lines.join("\n") + "\n");
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
        let key = dnt_data::Key::new(dnt_data::Kind::Feat, &["Tough", "XPHB"]);
        let block = super::source_for(&key).unwrap();
        assert!(block.starts_with("[[feature]]\nid = \"feat:Tough|XPHB\""), "{block}");
        assert!(block.contains("2 * level") && !block.contains("Lucky"));
    }

    #[test]
    fn the_srd_has_the_2024_rules() {
        use dnt_data::{Key, Kind};
        let (set, _) = super::overlays();
        assert!(!set.for_entity(&Key::new(Kind::Feat, &["Alert", "SRD52"])).is_empty());
        let focus = Key::new(Kind::ClassFeature, &["Monk’s Focus", "Monk", "SRD52", "2", "SRD52"]);
        assert!(!set.for_entity(&focus).is_empty(), "with the SRD's apostrophe");
        let block = super::source_for(&focus).unwrap();
        assert!(block.contains("id = \"classFeature:Monk’s Focus|Monk|SRD52|2\""), "{block}");
    }
}
