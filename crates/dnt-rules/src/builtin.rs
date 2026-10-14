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
            Ok((overlay, d)) => {
                set.add(overlay);
                diags.extend(d.into_iter().map(|d| Diagnostic {
                    location: format!("{name}: {}", d.location),
                    message: d.message,
                }));
            }
            Err(e) => diags.push(Diagnostic {
                location: name.to_string(),
                message: e,
            }),
        }
    }
    (set, diags)
}

#[cfg(test)]
mod tests {
    #[test]
    fn builtins_parse_cleanly() {
        let (set, diags) = super::overlays();
        assert!(diags.is_empty(), "{diags:#?}");
        assert!(set.len() > 20);
    }
}
