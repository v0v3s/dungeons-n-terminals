mod app;
mod compendium;
mod render;
mod theme;
mod ui;

use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result};
use clap::Parser;
use dnt_data::{Library, SourceFilter};
use ratatui::crossterm::event::{self, Event, KeyEventKind};

use app::App;

/// Dungeons & Terminals: a D&D 5e TUI for players.
#[derive(Parser)]
#[command(version)]
struct Args {
    /// Path to a data data checkout (repository root or its `data/` directory).
    #[arg(long, env = "DNT_DATA")]
    data: PathBuf,

    /// Additional homebrew JSON files to load.
    #[arg(long = "homebrew", value_name = "FILE")]
    homebrew: Vec<PathBuf>,

    /// Comma-separated source abbreviations to enable, e.g. `XPHB,XDMG`.
    /// All sources are enabled when omitted.
    #[arg(long, value_delimiter = ',')]
    sources: Vec<String>,

    /// Show entries even when a reprint of them is enabled.
    #[arg(long)]
    show_reprinted: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let start = Instant::now();
    let mut lib = Library::load_data_dir(&args.data).with_context(|| format!("loading {}", args.data.display()))?;
    for path in &args.homebrew {
        lib.load_file(path).with_context(|| format!("loading {}", path.display()))?;
    }
    let mut status = format!("Loaded {} entries in {} ms", lib.entities().len(), start.elapsed().as_millis());
    if !lib.warnings.is_empty() {
        status.push_str(&format!(" ({} warnings)", lib.warnings.len()));
    }

    let filter = if args.sources.is_empty() {
        SourceFilter {
            enabled: None,
            hide_reprinted: !args.show_reprinted,
        }
    } else {
        SourceFilter {
            hide_reprinted: !args.show_reprinted,
            ..SourceFilter::only(&args.sources)
        }
    };

    let mut app = App::new(&lib, filter, status);
    let theme = theme::Theme::default();
    ratatui::run(|terminal| -> Result<()> {
        while !app.quit {
            terminal.draw(|f| ui::draw(f, &lib, &mut app, &theme))?;
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    app.handle_key(&lib, key);
                }
            }
        }
        Ok(())
    })
}
