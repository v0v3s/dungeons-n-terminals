//! Getting started: when there is no data yet, ask for a zip to import (or
//! a data folder to read) instead of stopping with an error.

use std::path::PathBuf;

use anyhow::Result;
use dnt_store::Store;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Flex, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Padding, Paragraph, Wrap};
use unicode_width::UnicodeWidthStr;

use crate::theme::Theme;
use crate::ui::{pane, pane_title};

/// How getting started ended.
pub enum Outcome {
    /// A zip was imported.
    Imported,
    /// Read this data folder (from now on).
    Folder(PathBuf),
    Quit,
}

/// `~/x` → the home folder's `x`.
fn expand(path: &str) -> PathBuf {
    let path = path.trim().trim_matches(['"', '\'']);
    match (path.strip_prefix("~/"), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) => PathBuf::from(home).join(rest),
        _ => PathBuf::from(path),
    }
}

pub fn run(store: &Store) -> Result<Outcome> {
    let theme = Theme::default();
    let mut input = String::new();
    let mut message: Option<(String, bool)> = None;
    ratatui::run(|terminal| -> Result<Outcome> {
        loop {
            terminal.draw(|frame| {
                let [area] = Layout::horizontal([Constraint::Max(84)]).flex(Flex::Center).areas(frame.area());
                let [area] = Layout::vertical([Constraint::Max(20)]).flex(Flex::Center).areas(area);
                let block = pane(pane_title("Getting started", true, &theme), true, &theme).padding(Padding::new(2, 2, 1, 1));
                let inner = block.inner(area);
                frame.render_widget(block, area);
                let [intro, prompt, rest] =
                    Layout::vertical([Constraint::Length(8), Constraint::Length(1), Constraint::Min(6)]).areas(inner);
                let intro_lines = vec![
                    Line::styled("dnt: a D&D 5e companion in the terminal", theme.title),
                    Line::default(),
                    Line::styled("▸ 1 · Import the data", theme.label),
                    Line::raw("dnt ships no game content: it reads the rules, spells, classes and items from data you import."),
                    Line::styled(
                        "scripts/pack-data.sh (in the project) makes a small zip of just what dnt reads. \
                         Give the zip's path, or a data folder's to read it in place:",
                        theme.dim,
                    ),
                ];
                frame.render_widget(Paragraph::new(intro_lines).wrap(Wrap { trim: false }), intro);
                frame.render_widget(
                    Paragraph::new(Line::from(vec![Span::styled("❯ ", Style::new().fg(theme.accent)), Span::raw(input.clone())])),
                    prompt,
                );
                frame.set_cursor_position((prompt.x + 2 + input.width() as u16, prompt.y));
                let key = |k: &str| Span::styled(format!(" {k} "), theme.keycap);
                let mut lines = vec![Line::default()];
                if let Some((m, ok)) = &message {
                    lines.push(Line::styled(m.clone(), if *ok { theme.label } else { Style::new().fg(Color::Red) }));
                    lines.push(Line::default());
                }
                lines.push(Line::styled("· 2 · Create a campaign    · 3 · Play", theme.dim));
                lines.push(Line::default());
                lines.push(Line::from(vec![
                    key("enter"),
                    Span::styled(" import  ", theme.dim),
                    key("esc"),
                    Span::styled(" quit", theme.dim),
                ]));
                frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), rest);
            })?;
            let Event::Key(key) = event::read()? else { continue };
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Esc => return Ok(Outcome::Quit),
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => return Ok(Outcome::Quit),
                KeyCode::Backspace => {
                    input.pop();
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => input.clear(),
                KeyCode::Char(c) => input.push(c),
                KeyCode::Enter if !input.trim().is_empty() => {
                    let path = expand(&input);
                    if path.is_dir() {
                        match dnt_data::source::DirSource::open(&path) {
                            Ok(_) => return Ok(Outcome::Folder(path)),
                            Err(e) => message = Some((e.to_string(), false)),
                        }
                        continue;
                    }
                    message = Some(("Importing…".into(), true));
                    terminal.draw(|_| {})?;
                    match std::fs::read(&path).map_err(|e| e.to_string()).and_then(|b| dnt_core::import::import_zip(store, b)) {
                        Ok(_) if dnt_core::import::has_imported(store) => return Ok(Outcome::Imported),
                        // Only homebrew: the data itself is still missing.
                        Ok(report) => message = Some((format!("{}; the data itself is still missing.", report.summary()), false)),
                        Err(e) => message = Some((format!("{}: {e}", path.display()), false)),
                    }
                }
                _ => {}
            }
        }
    })
}
