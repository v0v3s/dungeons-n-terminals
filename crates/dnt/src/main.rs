mod app;
mod campaigns;
mod compendium;
mod render;
mod theme;
mod ui;

use std::path::PathBuf;
use std::time::Instant;

use anyhow::{bail, Context, Result};
use clap::Parser;
use dnt_data::{Library, SourceFilter};
use dnt_store::Store;
use ratatui::crossterm::event::{self, Event, KeyEventKind};

use app::App;

/// Dungeons & Terminals: a D&D 5e TUI for players.
#[derive(Parser)]
#[command(version)]
struct Args {
    /// Path to a data data checkout (repository root or its `data/`
    /// directory). Remembered in the config file once given.
    #[arg(long, env = "DNT_DATA")]
    data: Option<PathBuf>,

    /// Additional homebrew JSON files to load (in addition to the config's).
    #[arg(long = "homebrew", value_name = "FILE")]
    homebrew: Vec<PathBuf>,

    /// Comma-separated source abbreviations to enable for this session,
    /// e.g. `XPHB,XDMG`, overriding the campaign's sources.
    #[arg(long, value_delimiter = ',')]
    sources: Vec<String>,

    /// With --sources: show entries even when a reprint of them is enabled.
    #[arg(long)]
    show_reprinted: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let store = Store::open_default()?;
    let mut config = store.load_config()?;

    let data = match (&args.data, &config.library.path) {
        (Some(path), _) => {
            if config.library.path.as_ref() != Some(path) {
                config.library.path = Some(path.clone());
                store.save_config(&config)?;
            }
            path.clone()
        }
        (None, Some(path)) => path.clone(),
        (None, None) => bail!(
            "no data data configured: run once with --data /path/to/dnd-data \
             (it is then remembered in {})",
            store.config_path().display()
        ),
    };

    let start = Instant::now();
    let mut lib = Library::load_data_dir(&data).with_context(|| format!("loading {}", data.display()))?;
    for path in config.library.homebrew.iter().chain(&args.homebrew) {
        lib.load_file(path).with_context(|| format!("loading {}", path.display()))?;
    }
    let mut status = format!("Loaded {} entries in {} ms", lib.entities().len(), start.elapsed().as_millis());
    if !lib.warnings.is_empty() {
        status.push_str(&format!(" ({} warnings)", lib.warnings.len()));
    }

    let override_filter = (!args.sources.is_empty()).then(|| SourceFilter {
        hide_reprinted: !args.show_reprinted,
        ..SourceFilter::only(&args.sources)
    });

    let mut app = App::new(&lib, store, config, override_filter, status);
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
