//! Parser for data inline tags such as `{@spell fireball|XPHB}`,
//! `{@b bold {@i nested}}` or `{@damage 2d6}`.
//!
//! Text is flattened into styled [`Span`]s; reference tags become links that
//! the library can resolve to entities.

use crate::kind::Kind;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub code: bool,
    /// Game mechanics: dice, DCs, attack bonuses.
    pub mechanic: bool,
    /// Secondary information, such as a source abbreviation.
    pub muted: bool,
}

/// An unresolved reference from an inline tag (or a structured `ref*` entry).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub tag: String,
    /// The raw pipe-separated arguments, including any display text.
    pub args: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub style: Style,
    pub link: Option<Link>,
}

impl Span {
    pub fn plain(text: impl Into<String>) -> Span {
        Span {
            text: text.into(),
            style: Style::default(),
            link: None,
        }
    }

    pub fn styled(text: impl Into<String>, style: Style) -> Span {
        Span {
            text: text.into(),
            style,
            link: None,
        }
    }
}

/// Parse a string containing inline tags into spans.
pub fn parse(input: &str) -> Vec<Span> {
    let mut out = Vec::new();
    parse_into(input, Style::default(), &mut out);
    merge_adjacent(out)
}

/// Strip all tags, keeping only display text.
pub fn strip(input: &str) -> String {
    parse(input).into_iter().map(|s| s.text).collect()
}

fn parse_into(input: &str, style: Style, out: &mut Vec<Span>) {
    let mut rest = input;
    while let Some(start) = rest.find("{@") {
        if start > 0 {
            out.push(Span::styled(&rest[..start], style));
        }
        let after = &rest[start..];
        match matching_brace(after) {
            Some(end) => {
                emit_tag(&after[2..end], style, out);
                rest = &after[end + 1..];
            }
            None => {
                // Unbalanced: emit the remainder verbatim.
                out.push(Span::styled(after, style));
                return;
            }
        }
    }
    if !rest.is_empty() {
        out.push(Span::styled(rest, style));
    }
}

/// Byte index of the `}` closing the tag that starts at `s[0]`.
fn matching_brace(s: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (i, c) in s.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Split on `|` at nesting depth zero.
fn split_top(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut last = 0;
    for (i, c) in s.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => depth -= 1,
            '|' if depth == 0 => {
                parts.push(&s[last..i]);
                last = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&s[last..]);
    parts
}

fn emit_tag(inner: &str, style: Style, out: &mut Vec<Span>) {
    let (tag, body) = match inner.find(char::is_whitespace) {
        Some(i) => (&inner[..i], inner[i..].trim_start()),
        None => (inner, ""),
    };
    let parts = split_top(body);
    let first = parts.first().copied().unwrap_or("");
    let nth = |i: usize| parts.get(i).copied().filter(|p| !p.is_empty());

    let with = |f: fn(&mut Style)| {
        let mut s = style;
        f(&mut s);
        s
    };

    match tag {
        "b" | "bold" => parse_into(body, with(|s| s.bold = true), out),
        "i" | "italic" => parse_into(body, with(|s| s.italic = true), out),
        "u" | "underline" => parse_into(body, with(|s| s.underline = true), out),
        "s" | "strike" | "s2" | "strikeDouble" => parse_into(body, with(|s| s.strike = true), out),
        "code" => parse_into(body, with(|s| s.code = true), out),
        "note" | "tip" | "unit" => parse_into(first, with(|s| s.italic = true), out),
        "color" | "highlight" | "sup" | "sub" | "kbd" | "style" | "font" | "help" => parse_into(first, style, out),
        "dice" | "damage" | "autodice" | "d20" | "hit" | "chance" | "recharge" | "scaledice" | "scaledamage" => {
            let text = match tag {
                "scaledice" | "scaledamage" => nth(3).or(nth(2)).unwrap_or(first).to_string(),
                "hit" | "d20" => nth(1).map(str::to_string).unwrap_or_else(|| signed(first)),
                "chance" => nth(1).map(str::to_string).unwrap_or_else(|| format!("{first} percent")),
                "recharge" => format!("(Recharge {}–6)", if first.is_empty() { "6" } else { first }),
                _ => nth(1).unwrap_or(first).to_string(),
            };
            out.push(Span::styled(text, with(|s| s.mechanic = true)));
        }
        "dc" => out.push(Span::styled(
            format!("DC {}", nth(1).unwrap_or(first)),
            with(|s| s.mechanic = true),
        )),
        "atk" | "atkr" => out.push(Span::styled(attack_label(first), with(|s| s.italic = true))),
        "h" => out.push(Span::styled("Hit: ", with(|s| s.italic = true))),
        "m" => out.push(Span::styled("Miss: ", with(|s| s.italic = true))),
        _ => {
            if let Some(kind) = Kind::for_tag(tag).first() {
                let display_idx = crate::id::Key::from_ref(*kind, tag, &parts)
                    .and_then(|(_, d)| d)
                    .unwrap_or(2);
                let display = nth(display_idx).unwrap_or(first);
                let mut spans = Vec::new();
                parse_into(display, style, &mut spans);
                let link = Link {
                    tag: tag.to_string(),
                    args: body.to_string(),
                };
                for mut s in spans {
                    s.link = Some(link.clone());
                    out.push(s);
                }
            } else {
                // Unknown or unsupported tags (filter, book, data, …): show
                // the display text, which is the 3rd part for "page" tags.
                let display = match tag {
                    "filter" | "book" | "adventure" | "homepage" | "link" | "loader" => first,
                    "quickref" => nth(4).unwrap_or(first),
                    _ => nth(2).unwrap_or(first),
                };
                parse_into(display, style, out);
            }
        }
    }
}

fn signed(n: &str) -> String {
    if n.starts_with(['+', '-']) {
        n.to_string()
    } else {
        format!("+{n}")
    }
}

fn attack_label(code: &str) -> String {
    let labels: Vec<&str> = code
        .split(',')
        .map(|c| match c.trim() {
            "mw" => "Melee Weapon Attack:",
            "rw" => "Ranged Weapon Attack:",
            "mw,rw" => "Melee or Ranged Weapon Attack:",
            "ms" => "Melee Spell Attack:",
            "rs" => "Ranged Spell Attack:",
            "m" => "Melee Attack Roll:",
            "r" => "Ranged Attack Roll:",
            _ => "Attack:",
        })
        .collect();
    if labels.len() > 1 {
        "Melee or Ranged Attack:".to_string()
    } else {
        labels.into_iter().next().unwrap_or("Attack:").to_string()
    }
}

fn merge_adjacent(spans: Vec<Span>) -> Vec<Span> {
    let mut out: Vec<Span> = Vec::with_capacity(spans.len());
    for s in spans {
        if s.text.is_empty() {
            continue;
        }
        if let Some(last) = out.last_mut() {
            if last.style == s.style && last.link == s.link {
                last.text.push_str(&s.text);
                continue;
            }
        }
        out.push(s);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text() {
        assert_eq!(parse("hello"), vec![Span::plain("hello")]);
    }

    #[test]
    fn link_with_display_text() {
        let spans = parse("Cast {@spell fireball|XPHB|a fireball} now");
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[1].text, "a fireball");
        assert_eq!(spans[1].link.as_ref().unwrap().args, "fireball|XPHB|a fireball");
    }

    #[test]
    fn nested_formatting() {
        let spans = parse("{@b Bold {@i both}}");
        assert_eq!(spans[0].text, "Bold ");
        assert!(spans[0].style.bold && !spans[0].style.italic);
        assert!(spans[1].style.bold && spans[1].style.italic);
    }

    #[test]
    fn mechanics() {
        assert_eq!(strip("{@damage 8d6} fire, {@dc 15}, {@hit 5}"), "8d6 fire, DC 15, +5");
        assert_eq!(strip("{@scaledamage 8d6|3-9|1d6}"), "1d6");
    }

    #[test]
    fn class_feature_display_index() {
        assert_eq!(strip("{@classFeature Rage|Barbarian||1||raging}"), "raging");
        assert_eq!(strip("{@classFeature Rage|Barbarian||1}"), "Rage");
    }

    #[test]
    fn filter_shows_first_part() {
        assert_eq!(strip("{@filter Cantrips|spells|level=0|class=Wizard}"), "Cantrips");
    }

    #[test]
    fn unbalanced_is_kept() {
        assert_eq!(strip("oops {@b never closed"), "oops {@b never closed");
    }
}
