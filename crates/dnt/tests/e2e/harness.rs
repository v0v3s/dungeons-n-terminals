//! The terminal side: the real `dnt` binary in a pseudo-terminal, driven
//! by keys, its screen read back through a terminal emulator.

use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};

use crate::seed::Home;

pub const COLS: u16 = 140;
pub const ROWS: u16 = 45;
/// How long a test waits for the screen to show something.
const WAIT: Duration = Duration::from_secs(10);
/// How long a test waits for the editor ($EDITOR) and the screen after it.
const EDITOR_WAIT: Duration = Duration::from_secs(60);
/// The screen is settled when nothing was drawn for this long.
const QUIET: Duration = Duration::from_millis(80);

pub const ENTER: &str = "\r";
pub const ESC: &str = "\x1b";
pub const TAB: &str = "\t";
pub const BACKTAB: &str = "\x1b[Z";
pub const UP: &str = "\x1b[A";
pub const DOWN: &str = "\x1b[B";
pub const RIGHT: &str = "\x1b[C";
pub const LEFT: &str = "\x1b[D";
pub const BACKSPACE: &str = "\x7f";

/// Ctrl and a letter.
pub fn ctrl(c: char) -> String {
    ((c.to_ascii_lowercase() as u8 - b'a' + 1) as char).to_string()
}

/// Alt and a key.
pub fn alt(c: char) -> String {
    format!("\x1b{c}")
}

struct Screen {
    parser: vt100::Parser,
    /// When the program last drew something.
    drawn: Instant,
}

pub struct Tui {
    pub scenario: &'static str,
    pub home: Home,
    child: Box<dyn Child + Send + Sync>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    screen: Arc<Mutex<Screen>>,
    _master: Box<dyn MasterPty + Send>,
}

impl Tui {
    /// Run `dnt` for a scenario of `e2e/scenarios.toml`, in `home`.
    pub fn scenario(scenario: &'static str, home: Home) -> Tui {
        Tui::with_args(scenario, home, &[])
    }

    /// Run `dnt` for the documentation's screenshots, not a scenario.
    pub fn for_docs(home: Home) -> Tui {
        Tui::start("docs", home, &[], Vec::new())
    }

    pub fn with_args(scenario: &'static str, home: Home, args: &[&str]) -> Tui {
        Tui::start(scenario, home, args, Vec::new())
    }

    /// Run `dnt` with more environment (it wins over the tests' own).
    pub fn with_env(scenario: &'static str, home: Home, env: &[(&'static str, String)]) -> Tui {
        Tui::start(scenario, home, &[], env.to_vec())
    }

    /// Run `dnt` with an editor ($EDITOR) that replaces `from` with `to` in
    /// the file it is given, as a user editing it would.
    pub fn with_editor(scenario: &'static str, home: Home, from: &str, to: &str) -> Tui {
        let editor = editor_script(home.path(), from, to);
        Tui::start(scenario, home, &[], vec![("EDITOR", editor.clone()), ("VISUAL", editor)])
    }

    fn start(scenario: &'static str, home: Home, args: &[&str], extra: Vec<(&'static str, String)>) -> Tui {
        let pty = native_pty_system().openpty(PtySize { rows: ROWS, cols: COLS, pixel_width: 0, pixel_height: 0 }).expect("a terminal");
        let mut cmd = CommandBuilder::new(env!("CARGO_BIN_EXE_dnt"));
        cmd.args(args);
        cmd.cwd(home.path());
        for (k, v) in env(home.path()).into_iter().chain(extra) {
            cmd.env(k, v);
        }
        cmd.env_remove("DNT_DATA");
        let child = pty.slave.spawn_command(cmd).expect("dnt starts");
        drop(pty.slave);
        let writer = Arc::new(Mutex::new(pty.master.take_writer().expect("a writer")));
        let screen = Arc::new(Mutex::new(Screen {
            parser: vt100::Parser::new(ROWS.try_into().unwrap(), COLS.try_into().unwrap(), 0),
            drawn: Instant::now(),
        }));
        let mut reader = pty.master.try_clone_reader().expect("a reader");
        {
            let screen = screen.clone();
            let writer = writer.clone();
            std::thread::spawn(move || {
                let mut buf = [0u8; 8192];
                while let Ok(n) = reader.read(&mut buf) {
                    if n == 0 {
                        break;
                    }
                    let bytes = &buf[..n];
                    // A terminal answers where its cursor is (Windows asks).
                    if bytes.windows(4).any(|w| w == b"\x1b[6n") {
                        let _ = writer.lock().unwrap().write_all(b"\x1b[1;1R");
                    }
                    let mut s = screen.lock().unwrap();
                    s.parser.process(bytes);
                    s.drawn = Instant::now();
                }
            });
        }
        let tui = Tui { scenario, home, child, writer, screen, _master: pty.master };
        tui.settle();
        tui
    }

    /// The screen's text, row by row.
    pub fn screen(&self) -> String {
        self.screen.lock().unwrap().parser.screen().contents()
    }

    /// The screen as an SVG picture, in the colours of dnt's look (for the
    /// documentation's screenshots).
    pub fn svg(&self) -> String {
        let guard = self.screen.lock().unwrap();
        svg(guard.parser.screen())
    }

    /// Wait until nothing is drawn for a moment.
    pub fn settle(&self) {
        let start = Instant::now();
        while start.elapsed() < WAIT {
            if self.screen.lock().unwrap().drawn.elapsed() >= QUIET {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Send keys (text, or the key constants), then let the screen settle:
    /// the program redraws after every key, so wait for that redraw (or a
    /// moment, for a key that changes nothing), then for quiet.
    pub fn send(&mut self, keys: &str) -> &mut Tui {
        let sent = Instant::now();
        {
            let mut w = self.writer.lock().unwrap();
            w.write_all(keys.as_bytes()).unwrap();
            w.flush().unwrap();
        }
        while sent.elapsed() < Duration::from_millis(500) && self.screen.lock().unwrap().drawn < sent {
            std::thread::sleep(Duration::from_millis(5));
        }
        self.settle();
        self
    }

    /// Type text one key at a time (a lone Esc can otherwise join a key).
    pub fn typed(&mut self, text: &str) -> &mut Tui {
        for c in text.chars() {
            self.send(&c.to_string());
        }
        self
    }

    /// Wait for the screen to show `text`; fail with the screen otherwise.
    #[track_caller]
    pub fn wait(&mut self, text: &str) -> &mut Tui {
        self.wait_until(&format!("the screen shows {text:?}"), |s| s.contains(text))
    }

    /// Wait for `text` to go from the screen.
    #[track_caller]
    pub fn wait_gone(&mut self, text: &str) -> &mut Tui {
        self.wait_until(&format!("{text:?} goes from the screen"), |s| !s.contains(text))
    }

    /// Wait for `text` after a step that runs the editor ($EDITOR), which
    /// can take long to start (PowerShell, cold, on Windows).
    #[track_caller]
    pub fn wait_after_editor(&mut self, text: &str) -> &mut Tui {
        self.wait_for(&format!("the screen shows {text:?}"), EDITOR_WAIT, |s| s.contains(text))
    }

    #[track_caller]
    pub fn wait_until(&mut self, what: &str, ok: impl Fn(&str) -> bool) -> &mut Tui {
        self.wait_for(what, WAIT, ok)
    }

    #[track_caller]
    fn wait_for(&mut self, what: &str, wait: Duration, ok: impl Fn(&str) -> bool) -> &mut Tui {
        let start = Instant::now();
        loop {
            let s = self.screen();
            if ok(&s) {
                return self;
            }
            if start.elapsed() > wait {
                panic!("[{}] waited for {what}; the screen:\n{}", self.scenario, framed(&s));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// The screen's line holding `text`.
    #[track_caller]
    pub fn line(&self, text: &str) -> String {
        let s = self.screen();
        s.lines()
            .find(|l| l.contains(text))
            .map(str::to_string)
            .unwrap_or_else(|| panic!("[{}] no line with {text:?}; the screen:\n{}", self.scenario, framed(&s)))
    }

    /// Fail with the screen unless it shows `text` (no waiting).
    #[track_caller]
    pub fn shows(&self, text: &str) -> bool {
        self.screen().contains(text)
    }

    /// Quit (Ctrl+C) and wait for the program to end.
    pub fn quit(&mut self) {
        self.send(&ctrl('c'));
        let start = Instant::now();
        while start.elapsed() < WAIT {
            if let Ok(Some(_)) = self.child.try_wait() {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("[{}] dnt did not quit", self.scenario);
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

fn framed(screen: &str) -> String {
    let rule = "─".repeat(COLS as usize);
    format!("{rule}\n{screen}\n{rule}")
}

/// The environment `dnt` runs in: its own home, repeatable dice, and an
/// editor that does nothing.
pub fn env(home: &Path) -> Vec<(&'static str, String)> {
    let editor = if cfg!(windows) { "rem" } else { "true" };
    vec![
        ("DNT_HOME", home.display().to_string()),
        ("DNT_SEED", "1".into()),
        ("EDITOR", editor.into()),
        ("VISUAL", editor.into()),
        ("TERM", "xterm-256color".into()),
        // The rules library of the tests, not the one on the web.
        ("DNT_RULES_URL", crate::seed::e2e_dir().join("rules").display().to_string()),
        // A release older than dnt: nothing to update (see e2e/update).
        ("DNT_UPDATE_URL", crate::seed::e2e_dir().join("update/older").display().to_string()),
    ]
}

/// A script that edits the file it is given: `from` becomes `to` (plain
/// text, without quotes or slashes).
fn editor_script(dir: &Path, from: &str, to: &str) -> String {
    assert!(!format!("{from}{to}").contains(['\'', '"', '/', '\\']), "the editor replaces plain text");
    editor_file(dir, from, to).display().to_string()
}

#[cfg(windows)]
fn editor_file(dir: &Path, from: &str, to: &str) -> std::path::PathBuf {
    let path = dir.join("edit.cmd");
    let ps = format!("$f = '%~1'; $t = [IO.File]::ReadAllText($f); [IO.File]::WriteAllText($f, $t.Replace('{from}', '{to}'))");
    std::fs::write(&path, format!("@powershell -NoProfile -Command \"{ps}\"\r\n")).unwrap();
    path
}

#[cfg(unix)]
fn editor_file(dir: &Path, from: &str, to: &str) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join("edit.sh");
    std::fs::write(&path, format!("#!/bin/sh\nexec perl -0pi -e 's/\\Q{from}\\E/{to}/g' \"$1\"\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

/// Run `dnt` with arguments, not in a terminal (its subcommands), for a
/// scenario of `e2e/scenarios.toml`.
pub fn run(scenario: &'static str, home: &Home, args: &[&str]) -> std::process::Output {
    run_with_env(scenario, home, args, &[])
}

/// [`run`] with more environment (it wins over the tests' own).
pub fn run_with_env(_scenario: &'static str, home: &Home, args: &[&str], more: &[(&'static str, String)]) -> std::process::Output {
    let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_dnt"));
    cmd.args(args).current_dir(home.path()).env_remove("DNT_DATA");
    for (k, v) in env(home.path()).into_iter().chain(more.iter().cloned()) {
        cmd.env(k, v);
    }
    cmd.output().expect("dnt runs")
}

/// The 16 colours of a terminal, as dnt's look has them (Tokyo Night).
const PALETTE: [&str; 16] = [
    "#15161e", "#f7768e", "#9ece6a", "#e0af68", "#7aa2f7", "#bb9af7", "#7dcfff", "#a9b1d6", "#414868", "#f7768e", "#9ece6a", "#e0af68",
    "#7aa2f7", "#bb9af7", "#7dcfff", "#c0caf5",
];
const BACKGROUND: &str = "#1a1b26";
const FOREGROUND: &str = "#c0caf5";

fn colour(c: vt100::Color, default: &str) -> String {
    match c {
        vt100::Color::Default => default.to_string(),
        vt100::Color::Idx(i) if i < 16 => PALETTE[i as usize].to_string(),
        // The 6×6×6 cube and the greys of 256-colour terminals.
        vt100::Color::Idx(i) if i < 232 => {
            let i = i - 16;
            let level = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            format!("#{:02x}{:02x}{:02x}", level(i / 36), level(i / 6 % 6), level(i % 6))
        }
        vt100::Color::Idx(i) => {
            let g = 8 + (i - 232) * 10;
            format!("#{g:02x}{g:02x}{g:02x}")
        }
        vt100::Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
    }
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// A terminal screen as SVG: a cell is `W`×`H`, runs of cells alike are one
/// piece of text on their background.
fn svg(screen: &vt100::Screen) -> String {
    const W: f32 = 8.4;
    const H: f32 = 18.0;
    let (rows, cols) = screen.size();
    let (rows, cols) = (rows.get(), cols.get());
    let mut back = String::new();
    let mut text = String::new();
    for row in 0..rows {
        let mut col = 0;
        while col < cols {
            let Some(cell) = screen.cell(row, col) else { break };
            let (mut fg, mut bg) = (colour(cell.fgcolor(), FOREGROUND), colour(cell.bgcolor(), BACKGROUND));
            if cell.inverse() {
                std::mem::swap(&mut fg, &mut bg);
            }
            let bold = cell.bold();
            // A run of cells with the same look.
            let start = col;
            let mut run = String::new();
            while col < cols {
                let Some(c) = screen.cell(row, col) else { break };
                let (mut f, mut b) = (colour(c.fgcolor(), FOREGROUND), colour(c.bgcolor(), BACKGROUND));
                if c.inverse() {
                    std::mem::swap(&mut f, &mut b);
                }
                if f != fg || b != bg || c.bold() != bold {
                    break;
                }
                if !c.is_wide_continuation() {
                    run.push_str(if c.contents().is_empty() { " " } else { c.contents() });
                }
                col += 1;
            }
            let (x, y) = (start as f32 * W, row as f32 * H);
            if bg != BACKGROUND {
                back.push_str(&format!(r#"<rect x="{x}" y="{y}" width="{}" height="{H}" fill="{bg}"/>"#, (col - start) as f32 * W));
            }
            if !run.trim().is_empty() {
                let weight = if bold { r#" font-weight="bold""# } else { "" };
                text.push_str(&format!(
                    r#"<text x="{x}" y="{}" fill="{fg}"{weight} textLength="{}" lengthAdjust="spacingAndGlyphs" xml:space="preserve">{}</text>"#,
                    y + H * 0.75,
                    (col - start) as f32 * W,
                    escape(&run)
                ));
            }
        }
    }
    let (width, height) = (cols as f32 * W, rows as f32 * H);
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" width="{width}" height="{height}" font-family="'JetBrains Mono', 'DejaVu Sans Mono', monospace" font-size="14"><rect width="100%" height="100%" fill="{BACKGROUND}"/>{back}{text}</svg>"#
    )
}
