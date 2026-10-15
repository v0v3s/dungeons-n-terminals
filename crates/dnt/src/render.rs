//! Lays out a [`dnt_data::doc`] document into terminal lines: word wrapping,
//! lists, tables, insets, and numbered links that can be focused.

use dnt_data::doc::{Block, Table};
use dnt_data::tags::{Link, Span as DocSpan};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::theme::Theme;

pub struct Rendered {
    pub lines: Vec<Line<'static>>,
    /// Links in reading order, with the line each one starts on.
    pub links: Vec<(Link, usize)>,
}

/// Space between table columns.
const GAP: usize = 3;

/// A styled run of text, optionally part of link number `link`.
#[derive(Clone)]
struct Run {
    text: String,
    style: Style,
    link: Option<usize>,
}

pub fn render(blocks: &[Block], width: u16, focused_link: Option<usize>, theme: &Theme) -> Rendered {
    let mut r = Renderer { width: width.max(10) as usize, lines: Vec::new(), links: Vec::new(), focused: focused_link, theme };
    r.blocks(blocks, &[], &[]);
    while r.lines.last().is_some_and(|l| l.spans.is_empty()) {
        r.lines.pop();
    }
    Rendered { lines: r.lines, links: r.links }
}

struct Renderer<'t> {
    width: usize,
    lines: Vec<Line<'static>>,
    links: Vec<(Link, usize)>,
    focused: Option<usize>,
    theme: &'t Theme,
}

impl Renderer<'_> {
    fn blank(&mut self) {
        if self.lines.last().is_some_and(|l| !l.spans.is_empty()) {
            self.lines.push(Line::default());
        }
    }

    /// Convert document spans to runs, assigning link numbers.
    fn runs(&mut self, spans: &[DocSpan], base: Style) -> Vec<Run> {
        let mut out = Vec::with_capacity(spans.len());
        let mut prev: Option<&Link> = None;
        for s in spans {
            let mut style = base;
            if s.style.bold {
                style = style.add_modifier(Modifier::BOLD);
            }
            if s.style.italic {
                style = style.add_modifier(Modifier::ITALIC);
            }
            if s.style.underline {
                style = style.add_modifier(Modifier::UNDERLINED);
            }
            if s.style.strike {
                style = style.add_modifier(Modifier::CROSSED_OUT);
            }
            if s.style.code {
                style = style.fg(self.theme.code);
            }
            if s.style.muted {
                style = style.patch(self.theme.dim);
            }
            if s.style.mechanic {
                style = style.fg(self.theme.mechanic);
            }
            let link = s.link.as_ref().map(|l| {
                // Consecutive spans of the same link (e.g. partially bold
                // display text) share one number. The line is fixed up when
                // the run is placed.
                if prev != Some(l) {
                    self.links.push((l.clone(), usize::MAX));
                }
                self.links.len() - 1
            });
            prev = s.link.as_ref();
            if let Some(n) = link {
                style = if Some(n) == self.focused { style.patch(self.theme.link_focused) } else { style.patch(self.theme.link) };
            }
            out.push(Run { text: s.text.clone(), style, link });
        }
        out
    }

    /// Word-wrap runs into lines, with prefixes for the first and later lines.
    fn wrap(&mut self, runs: Vec<Run>, first: &[Span<'static>], rest: &[Span<'static>]) {
        let prefix_w = |p: &[Span<'static>]| p.iter().map(|s| s.content.width()).sum::<usize>();
        let mut line: Vec<Span<'static>> = first.to_vec();
        let mut col = prefix_w(first);
        let mut avail = self.width.saturating_sub(col).max(1);
        let mut used = 0usize;
        let mut started = false;

        // Tokenise into words and spaces, keeping style per token.
        let mut tokens: Vec<(String, Style, Option<usize>, bool)> = Vec::new();
        for run in &runs {
            let mut cur = String::new();
            let mut cur_space = None;
            for ch in run.text.chars() {
                let ch = if ch == '\n' { ' ' } else { ch };
                let is_space = ch == ' ';
                if cur_space.is_some_and(|s| s != is_space) {
                    tokens.push((std::mem::take(&mut cur), run.style, run.link, cur_space.unwrap()));
                }
                cur.push(ch);
                cur_space = Some(is_space);
            }
            if !cur.is_empty() {
                tokens.push((cur, run.style, run.link, cur_space.unwrap_or(false)));
            }
        }

        let mut i = 0;
        while i < tokens.len() {
            let (text, style, link, is_space) = &tokens[i];
            if *is_space {
                // Collapse spaces at line starts; otherwise emit (they may be
                // dropped if the next word wraps).
                if used > 0 && used < avail {
                    line.push(Span::styled(" ", base_style(*style, *link)));
                    used += 1;
                }
                i += 1;
                continue;
            }
            // Width of this word including directly attached tokens
            // (e.g. a link followed by a comma).
            let mut word_w = text.width();
            let mut j = i + 1;
            while j < tokens.len() && !tokens[j].3 {
                word_w += tokens[j].0.width();
                j += 1;
            }
            if used > 0 && used + word_w > avail {
                // Drop trailing space and wrap.
                if line.last().is_some_and(|s| s.content == " ") {
                    line.pop();
                }
                self.lines.push(Line::from(std::mem::take(&mut line)));
                line = rest.to_vec();
                col = prefix_w(rest);
                avail = self.width.saturating_sub(col).max(1);
                used = 0;
            }
            for (text, style, link, _) in &tokens[i..j] {
                if let Some(n) = link {
                    if self.links[*n].1 == usize::MAX {
                        self.links[*n].1 = self.lines.len();
                    }
                }
                // Hard-break words longer than the line.
                let mut chunk = String::new();
                for ch in text.chars() {
                    let w = ch.width().unwrap_or(0);
                    if used + w > avail && used > 0 {
                        line.push(Span::styled(std::mem::take(&mut chunk), *style));
                        self.lines.push(Line::from(std::mem::take(&mut line)));
                        line = rest.to_vec();
                        avail = self.width.saturating_sub(prefix_w(rest)).max(1);
                        used = 0;
                    }
                    chunk.push(ch);
                    used += w;
                }
                if !chunk.is_empty() {
                    line.push(Span::styled(chunk, *style));
                }
            }
            started = true;
            i = j;
        }
        if started || !line.is_empty() {
            self.lines.push(Line::from(line));
        }
    }

    fn blocks(&mut self, blocks: &[Block], first: &[Span<'static>], rest: &[Span<'static>]) {
        for (i, b) in blocks.iter().enumerate() {
            let prefix = if i == 0 { first } else { rest };
            self.block(b, prefix, rest);
        }
    }

    fn block(&mut self, block: &Block, first: &[Span<'static>], rest: &[Span<'static>]) {
        match block {
            Block::Heading { level, text } => {
                self.blank();
                if *level <= 1 {
                    let runs = self.runs(text, self.theme.heading);
                    let bar = with(first, Span::styled("▍", self.theme.heading_bar));
                    let hang = with(rest, Span::raw(" "));
                    self.wrap(runs, &bar, &hang);
                } else {
                    let runs = self.runs(text, self.theme.subheading);
                    self.wrap(runs, first, rest);
                }
            }
            Block::Paragraph(spans) => {
                let runs = self.runs(spans, Style::default());
                self.wrap(runs, first, rest);
                self.lines.push(Line::default());
            }
            Block::List(items) => {
                let bullet: Vec<Span<'static>> = with(first, Span::styled("• ", self.theme.bullet));
                let hang: Vec<Span<'static>> = with(rest, Span::raw("  "));
                for item in items {
                    let before = self.lines.len();
                    self.blocks(item, &bullet, &hang);
                    // Keep list items tight: drop the paragraph gap inside items.
                    if self.lines.len() > before && self.lines.last().is_some_and(|l| l.spans.is_empty()) {
                        self.lines.pop();
                    }
                }
                self.lines.push(Line::default());
            }
            Block::Table(t) => {
                self.table(t, rest);
                self.lines.push(Line::default());
            }
            Block::Inset { title, body } => {
                // A rounded box: ╭─ Title ─╮ / │ body │ / ╰───╯
                let border = self.theme.inset_border;
                let indent_w: usize = rest.iter().map(|s| s.content.width()).sum();
                let inner_w = self.width.saturating_sub(indent_w + 2);
                let title_text: String = title.as_ref().map(|t| t.iter().map(|s| s.text.as_str()).collect()).unwrap_or_default();
                let mut top = rest.to_vec();
                top.push(Span::styled("╭─", border));
                if !title_text.is_empty() {
                    let title_text = format!(" {title_text} ");
                    let fill = inner_w.saturating_sub(title_text.width() + 1);
                    top.push(Span::styled(title_text, self.theme.subheading));
                    top.push(Span::styled("─".repeat(fill), border));
                } else {
                    top.push(Span::styled("─".repeat(inner_w.saturating_sub(1)), border));
                }
                top.push(Span::styled("╮", border));
                self.lines.push(Line::from(top));

                let bar: Vec<Span<'static>> = with(rest, Span::styled("│ ", border));
                let saved = self.width;
                self.width = saved.saturating_sub(2);
                let start = self.lines.len();
                self.blocks(body, &bar, &bar);
                while self.lines.len() > start && self.lines.last().is_some_and(|l| l.spans.len() <= bar.len()) {
                    self.lines.pop();
                }
                self.width = saved;
                // Close the right-hand side.
                for line in &mut self.lines[start..] {
                    if line.spans.is_empty() {
                        line.spans = bar.clone();
                    }
                    let used: usize = line.spans.iter().map(|s| s.content.width()).sum();
                    let pad = self.width.saturating_sub(used + 1);
                    line.spans.push(Span::raw(" ".repeat(pad)));
                    line.spans.push(Span::styled("│", border));
                }
                let mut bottom = rest.to_vec();
                bottom.push(Span::styled(format!("╰{}╯", "─".repeat(inner_w)), border));
                self.lines.push(Line::from(bottom));
                self.lines.push(Line::default());
            }
            Block::Quote { body, by } => {
                let bar: Vec<Span<'static>> = with(rest, Span::styled("┃ ", self.theme.quote_bar));
                let start = self.lines.len();
                self.blocks(body, &bar, &bar);
                for line in &mut self.lines[start..] {
                    line.spans.iter_mut().for_each(|s| s.style = s.style.add_modifier(Modifier::ITALIC));
                }
                if let Some(by) = by {
                    if self.lines.last().is_some_and(|l| l.spans.is_empty()) {
                        self.lines.pop();
                    }
                    let mut l = bar.clone();
                    l.push(Span::styled(format!("— {by}"), self.theme.dim));
                    self.lines.push(Line::from(l));
                    self.lines.push(Line::default());
                }
            }
            Block::Properties(props) => {
                // Two aligned columns: dim labels, then values.
                let label_of =
                    |spans: &[DocSpan]| spans.first().map(|s| s.text.trim_end_matches([':', ' ']).to_string()).unwrap_or_default();
                let label_w = props.iter().map(|p| label_of(p).width()).max().unwrap_or(0);
                let hang: Vec<Span<'static>> = with(rest, Span::raw(" ".repeat(label_w + 2)));
                for (i, spans) in props.iter().enumerate() {
                    let label = label_of(spans);
                    let lead = with(if i == 0 { first } else { rest }, Span::styled(format!("{label:<label_w$}  "), self.theme.label));
                    let runs = self.runs(spans.get(1..).unwrap_or_default(), Style::default());
                    self.wrap(runs, &lead, &hang);
                }
            }
            Block::Rule => {
                let w = self.width.saturating_sub(rest.iter().map(|s| s.content.width()).sum());
                let mut l = rest.to_vec();
                l.push(Span::styled("─".repeat(w), self.theme.dim));
                self.lines.push(Line::from(l));
            }
        }
    }

    fn table(&mut self, t: &Table, indent: &[Span<'static>]) {
        let indent_w: usize = indent.iter().map(|s| s.content.width()).sum();
        let ncols = t.header.len().max(t.rows.iter().map(Vec::len).max().unwrap_or(0));
        if ncols == 0 {
            return;
        }
        if let Some(caption) = &t.caption {
            let runs = self.runs(caption, self.theme.subheading);
            self.wrap(runs, indent, indent);
        }
        let text_w = |spans: &[DocSpan]| spans.iter().map(|s| s.text.width()).sum::<usize>();
        let word_w =
            |spans: &[DocSpan]| spans.iter().flat_map(|s| s.text.split_whitespace()).map(UnicodeWidthStr::width).max().unwrap_or(0);
        // Natural width (no wrapping) and minimum width (longest word) per column.
        let mut natural = vec![1usize; ncols];
        let mut minimum = vec![1usize; ncols];
        for row in std::iter::once(&t.header).chain(&t.rows) {
            for (c, cell) in row.iter().enumerate() {
                natural[c] = natural[c].max(text_w(cell));
                minimum[c] = minimum[c].max(word_w(cell));
            }
        }
        let fixed = GAP * (ncols - 1);
        let avail = self.width.saturating_sub(indent_w + fixed);
        let widths: Vec<usize> = if natural.iter().sum::<usize>() <= avail {
            natural
        } else {
            // Start from the minimums and share the remaining space in
            // proportion to how much each column would like to grow.
            let base: usize = minimum.iter().sum();
            let spare = avail.saturating_sub(base);
            let want: usize = natural.iter().zip(&minimum).map(|(n, m)| n - m).sum::<usize>().max(1);
            natural.iter().zip(&minimum).map(|(n, m)| m + (n - m) * spare / want).collect()
        };

        let header_style = self.theme.table_header;
        self.table_row(&t.header, &widths, indent, header_style);
        let total = widths.iter().sum::<usize>() + fixed;
        let mut rule = indent.to_vec();
        rule.push(Span::styled("─".repeat(total), self.theme.dim));
        self.lines.push(Line::from(rule));
        for (r, row) in t.rows.iter().enumerate() {
            let style = if r % 2 == 1 { self.theme.table_alt } else { Style::default() };
            self.table_row(row, &widths, indent, style);
        }
    }

    fn table_row(&mut self, cells: &[Vec<DocSpan>], widths: &[usize], indent: &[Span<'static>], style: Style) {
        // Lay each cell out independently, then zip the cell lines together.
        let mut cell_lines: Vec<Vec<Line<'static>>> = Vec::new();
        let base_line = self.lines.len();
        for (c, &w) in widths.iter().enumerate() {
            let first_new = self.links.len();
            let runs = cells.get(c).map(|cell| self.runs(cell, style)).unwrap_or_default();
            let mut sub =
                Renderer { width: w, lines: Vec::new(), links: std::mem::take(&mut self.links), focused: self.focused, theme: self.theme };
            sub.wrap(runs, &[], &[]);
            // Lines of links inside the cell are relative to the row; rebase them.
            for (_, line) in sub.links[first_new..].iter_mut() {
                if *line != usize::MAX {
                    *line += base_line;
                }
            }
            self.links = sub.links;
            cell_lines.push(sub.lines);
        }
        let height = cell_lines.iter().map(Vec::len).max().unwrap_or(1).max(1);
        for h in 0..height {
            let mut spans = indent.to_vec();
            for (c, &w) in widths.iter().enumerate() {
                let line = cell_lines[c].get(h);
                let used: usize = line.map(|l| l.spans.iter().map(|s| s.content.width()).sum()).unwrap_or(0);
                if let Some(line) = line {
                    spans.extend(line.spans.iter().cloned());
                }
                let pad = w.saturating_sub(used) + if c + 1 < widths.len() { GAP } else { 0 };
                spans.push(Span::styled(" ".repeat(pad), style));
            }
            self.lines.push(Line::from(spans));
        }
    }
}

/// Style for a space between words: keep link underline/colour only when
/// the space sits inside a link.
fn base_style(style: Style, link: Option<usize>) -> Style {
    if link.is_some() {
        style
    } else {
        Style::default().bg(style.bg.unwrap_or_default())
    }
}

fn with(prefix: &[Span<'static>], extra: Span<'static>) -> Vec<Span<'static>> {
    let mut v = prefix.to_vec();
    v.push(extra);
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use dnt_data::doc;
    use serde_json::json;

    fn text(r: &Rendered) -> Vec<String> {
        r.lines.iter().map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect()).collect()
    }

    #[test]
    fn wraps_words() {
        let blocks = doc::from_entries(&json!(["one two three four five"]));
        let r = render(&blocks, 10, None, &Theme::default());
        assert_eq!(text(&r), vec!["one two", "three four", "five"]);
    }

    #[test]
    fn records_link_lines() {
        let blocks = doc::from_entries(&json!(["aaaa bbbb {@spell fireball} {@condition prone}"]));
        let r = render(&blocks, 12, None, &Theme::default());
        assert_eq!(r.links.len(), 2);
        assert_eq!(r.links[0].1, 1);
        assert_eq!(r.links[1].1, 2);
    }

    #[test]
    fn lists_hang() {
        let blocks = doc::from_entries(&json!([{"type": "list", "items": ["alpha beta gamma"]}]));
        let r = render(&blocks, 12, None, &Theme::default());
        assert_eq!(text(&r), vec!["• alpha beta", "  gamma"]);
    }

    #[test]
    fn table_columns_align() {
        let blocks = doc::from_entries(&json!([{"type": "table", "colLabels": ["A", "B"], "rows": [["x", "y"], ["long", "z"]]}]));
        let r = render(&blocks, 40, None, &Theme::default());
        assert_eq!(text(&r), vec!["A      B", "────────", "x      y", "long   z"]);
    }
}
