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

    pub fn with_args(scenario: &'static str, home: Home, args: &[&str]) -> Tui {
        let pty = native_pty_system().openpty(PtySize { rows: ROWS, cols: COLS, pixel_width: 0, pixel_height: 0 }).expect("a terminal");
        let mut cmd = CommandBuilder::new(env!("CARGO_BIN_EXE_dnt"));
        cmd.args(args);
        cmd.cwd(home.path());
        for (k, v) in env(home.path()) {
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

    /// Send keys (text, or the key constants), then let the screen settle.
    pub fn send(&mut self, keys: &str) -> &mut Tui {
        {
            let mut w = self.writer.lock().unwrap();
            w.write_all(keys.as_bytes()).unwrap();
            w.flush().unwrap();
        }
        // Make sure the program saw the keys before waiting for quiet.
        std::thread::sleep(Duration::from_millis(30));
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

    #[track_caller]
    pub fn wait_until(&mut self, what: &str, ok: impl Fn(&str) -> bool) -> &mut Tui {
        let start = Instant::now();
        loop {
            let s = self.screen();
            if ok(&s) {
                return self;
            }
            if start.elapsed() > WAIT {
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
    ]
}

/// Run `dnt` with arguments, not in a terminal (its subcommands).
pub fn run(home: &Home, args: &[&str]) -> std::process::Output {
    let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_dnt"));
    cmd.args(args).current_dir(home.path()).env_remove("DNT_DATA");
    for (k, v) in env(home.path()) {
        cmd.env(k, v);
    }
    cmd.output().expect("dnt runs")
}
