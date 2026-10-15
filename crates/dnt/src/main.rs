mod app;
mod app_homebrew;
mod campaigns;
mod characters;
mod compendium;
mod entry_editor;
mod export;
mod help;
mod levelup;
mod notes_view;
mod overlay_editor;
mod picker;
mod render;
mod rolls;
mod spells_tab;
mod theme;
mod ui;
mod ui_editor;
mod ui_notes;
mod ui_sheet;
mod wizard;

use std::path::{Path, PathBuf};
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

/// The base library: books, global homebrew (`<data>/library/homebrew/`)
/// and extra homebrew files. Campaign homebrew is layered on a clone of it
/// (see App::prepare_library).
fn load_base(data: &Path, extra: &[PathBuf], store: &Store) -> Result<Library> {
    let mut base = Library::load_data_dir(data).with_context(|| format!("loading {}", data.display()))?;
    for path in store.global_homebrew_files().iter().chain(extra) {
        base.load_file(path).with_context(|| format!("loading {}", path.display()))?;
    }
    Ok(base)
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
    let extra: Vec<PathBuf> = config.library.homebrew.iter().chain(&args.homebrew).cloned().collect();
    let mut base = load_base(&data, &extra, &store)?;
    let mut status = format!("Loaded {} entries in {} ms", base.entities().len(), start.elapsed().as_millis());
    if !base.warnings.is_empty() {
        status.push_str(&format!(" ({} warnings)", base.warnings.len()));
    }

    let override_filter =
        (!args.sources.is_empty()).then(|| SourceFilter { hide_reprinted: !args.show_reprinted, ..SourceFilter::only(&args.sources) });

    let mut app = App::new(&base, store, config, override_filter, status);
    let mut lib = app.prepare_library(&base);
    app.library_reloaded(&lib);
    let theme = theme::Theme::default();
    ratatui::run(|terminal| -> Result<()> {
        while !app.quit {
            terminal.draw(|f| ui::draw(f, &lib, &mut app, &theme))?;
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    app.handle_key(&lib, key);
                }
            }
            if let Some(path) = app.pending_edit.take() {
                ratatui::restore();
                let fallback = if cfg!(windows) { "notepad" } else { "vi" };
                let editor = std::env::var("VISUAL").or_else(|_| std::env::var("EDITOR")).unwrap_or_else(|_| fallback.into());
                // $EDITOR may carry arguments (`code --wait`), so let the shell split it;
                // the path is passed separately ("$1") to survive spaces.
                let status = if cfg!(windows) {
                    std::process::Command::new("cmd").arg("/C").arg(format!("{editor} \"{}\"", path.display())).status()
                } else {
                    std::process::Command::new("sh").arg("-c").arg(format!("{editor} \"$1\"")).arg("dnt").arg(&path).status()
                };
                *terminal = ratatui::init();
                match status {
                    Ok(_) => app.after_edit(&lib, &path),
                    Err(e) => app.status = format!("Could not run {editor}: {e}"),
                }
            }
            if std::mem::take(&mut app.pending_base_reload) {
                match load_base(&data, &extra, &app.store) {
                    Ok(b) => base = b,
                    Err(e) => app.status = format!("Could not reload the library: {e:#}"),
                }
                app.pending_reload = true;
            }
            if std::mem::take(&mut app.pending_reload) {
                lib = app.prepare_library(&base);
                app.library_reloaded(&lib);
            }
        }
        Ok(())
    })
}
