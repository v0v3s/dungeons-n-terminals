mod app;
mod app_homebrew;
mod campaigns;
mod characters;
mod compendium;
mod entry_editor;
mod help;
mod levelup;
mod notes_view;
mod overlay_editor;
mod picker;
mod render;
mod setup;
mod theme;
mod ui;
mod ui_editor;
mod ui_notes;
mod ui_sheet;
mod wizard;

use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use dnt_data::{Library, SourceFilter};
use dnt_store::Store;
use ratatui::crossterm::event::{self, Event, KeyEventKind};

use app::App;

/// Dungeons & Terminals: a D&D 5e TUI for players.
#[derive(Parser)]
#[command(version)]
struct Args {
    #[command(subcommand)]
    command: Option<Cmd>,

    /// Path to the data directory (or a folder holding it as `data/`), in
    /// the format described in schemas/. Remembered once given.
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

#[derive(Subcommand)]
enum Cmd {
    /// Import a file: data or homebrew (a zip of a data folder, or files in
    /// any layout; importing again updates entries instead of doubling
    /// them), an exported character or campaign, or an entry (.json).
    Import {
        file: PathBuf,
        /// A character or campaign of that name is here: replace it.
        #[arg(long, conflicts_with = "keep_both")]
        replace: bool,
        /// A character or campaign of that name is here: keep both.
        #[arg(long)]
        keep_both: bool,
    },
    /// Back everything up (campaigns, the imported data, your homebrew and
    /// rules) into one zip; with --every, --keep or --folder, set the
    /// automatic backups.
    Backup {
        /// Where to write it (else the backups folder).
        file: Option<PathBuf>,
        /// Back up by itself at start: off, day, week or month.
        #[arg(long, value_parser = ["off", "day", "week", "month"])]
        every: Option<String>,
        /// How many automatic backups to keep.
        #[arg(long)]
        keep: Option<u32>,
        /// The backups folder, from now on (`--folder default`: back to
        /// `backups` in the data folder).
        #[arg(long)]
        folder: Option<PathBuf>,
    },
    /// List the backups in the backups folder, newest first.
    Backups,
    /// Replace everything with a backup (what is here is backed up first).
    Restore {
        file: PathBuf,
        /// Do not ask first.
        #[arg(long)]
        yes: bool,
    },
    /// Export to import elsewhere: a character (with the homebrew and rules
    /// they use), a campaign (with all of it), or an entry by id
    /// (`spell:Fireball|XPHB`).
    Export {
        #[command(subcommand)]
        what: ExportWhat,
    },
    /// List the imported sources, then the imported homebrew.
    Imported,
    /// Remove an imported source (or homebrew source) and everything in it;
    /// with --homebrew all imported homebrew, with --all everything imported.
    Forget {
        #[arg(required_unless_present_any = ["homebrew", "all"])]
        source: Option<String>,
        /// Remove all imported homebrew (the data stays).
        #[arg(long, conflicts_with = "all")]
        homebrew: bool,
        /// Remove everything imported, data and homebrew (campaigns stay).
        #[arg(long)]
        all: bool,
    },
    /// Start over: delete the imported data, every campaign (characters,
    /// notes, homebrew) and the configuration.
    Reset {
        /// Do not ask first.
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Subcommand)]
enum ExportWhat {
    /// A character of the active campaign (or --campaign), by name.
    Character {
        name: String,
        #[arg(long)]
        campaign: Option<String>,
        /// Where to write it (else `<name>.dnt.zip` here).
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
    /// A campaign, by name.
    Campaign {
        name: String,
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
    /// One entry, by id (`spell:Fireball|XPHB`), as a data file.
    Entry {
        id: String,
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
}

/// The workspace on the active campaign (or the one named).
fn workspace(store: Store, campaign: Option<&str>) -> Result<dnt_core::workspace::Workspace> {
    let mut ws = dnt_core::workspace::Workspace::open(store);
    if let Some(name) = campaign {
        // For this run only: the app still opens the campaign it did.
        ws.active = Some(
            ws.campaigns
                .iter()
                .position(|c| c.settings.name.eq_ignore_ascii_case(name) || c.slug == name)
                .with_context(|| format!("no campaign {name}"))?,
        );
        ws.prepare();
    }
    Ok(ws)
}

fn write_out(out: Option<PathBuf>, default: String, bytes: &[u8]) -> Result<()> {
    let path = out.unwrap_or_else(|| PathBuf::from(default));
    std::fs::write(&path, bytes).with_context(|| format!("writing {}", path.display()))?;
    println!("Exported to {}", path.display());
    Ok(())
}

fn export(store: Store, what: ExportWhat) -> Result<()> {
    use dnt_core::bundle;
    match what {
        ExportWhat::Character { name, campaign, out } => {
            let ws = workspace(store, campaign.as_deref())?;
            let file = ws
                .characters()
                .into_iter()
                .find(|f| f.character.name.eq_ignore_ascii_case(&name) || f.slug == name)
                .with_context(|| format!("no character {name} in this campaign"))?;
            let lib = ws.lib.as_ref().context("no data yet")?;
            let s = dnt_core::session::CharacterSession::new(file, lib, &ws.overlays, &ws.filter());
            let bytes = bundle::character(&ws, &s).map_err(anyhow::Error::msg)?;
            write_out(out, bundle::file_name(&s.character().name, "dnt.zip"), &bytes)
        }
        ExportWhat::Campaign { name, out } => {
            let ws = workspace(store, None)?;
            let c = ws
                .campaigns
                .iter()
                .find(|c| c.settings.name.eq_ignore_ascii_case(&name) || c.slug == name)
                .with_context(|| format!("no campaign {name}"))?;
            let bytes = bundle::campaign(&ws, &c.slug).map_err(anyhow::Error::msg)?;
            write_out(out, bundle::file_name(&c.settings.name, "dnt.zip"), &bytes)
        }
        ExportWhat::Entry { id, out } => {
            let ws = workspace(store, None)?;
            let lib = ws.lib.as_ref().context("no data yet")?;
            let e = dnt_rules::overlay::resolve_id(lib, &id).with_context(|| format!("no entry {id}"))?;
            let text = bundle::entry(&ws, e).map_err(anyhow::Error::msg)?;
            write_out(out, bundle::file_name(&lib.entity(e).name, "json"), text.as_bytes())
        }
    }
}

/// Run a subcommand (no TUI).
fn run_command(cmd: Cmd, store: Store, config: &dnt_store::Config) -> Result<()> {
    use dnt_core::{backup, bundle, import};
    let store = &store;
    match cmd {
        Cmd::Import { file, replace, keep_both } => {
            let bytes = std::fs::read(&file).with_context(|| format!("reading {}", file.display()))?;
            let name = file.file_name().unwrap_or_default().to_string_lossy().into_owned();
            let mut ws = dnt_core::workspace::Workspace::open(Store::open(store.data_dir(), store.config_path()));
            let plan = bundle::plan(&ws, &bytes, &name).map_err(anyhow::Error::msg)?;
            match plan.what.as_str() {
                "backup" => anyhow::bail!("{name} is a backup: `dnt restore {}` replaces everything with it", file.display()),
                "data" => {}
                _ => {
                    let on_clash = match (replace, keep_both) {
                        (true, _) => bundle::OnClash::Replace,
                        (_, true) => bundle::OnClash::KeepBoth,
                        _ => bundle::OnClash::Refuse,
                    };
                    if let (Some(here), bundle::OnClash::Refuse) = (&plan.clash, on_clash) {
                        anyhow::bail!("{here} is here already: import again with --replace or --keep-both");
                    }
                    println!("{}", bundle::apply(&mut ws, bytes, &name, on_clash).map_err(anyhow::Error::msg)?);
                    return Ok(());
                }
            }
            let report = import::import_zip(store, bytes).map_err(anyhow::Error::msg)?;
            println!("{}", report.summary());
            for (file, why) in &report.skipped {
                println!("  skipped {file}: {why}");
            }
            let brew = import::homebrew(store);
            if !brew.is_empty() {
                println!(
                    "Homebrew ({} sources) is off until a campaign enables it: list its ids under `homebrew` in the \
                     campaign's campaign.toml (`dnt imported` shows them), or tick them in the app.",
                    brew.len()
                );
            }
            if let Some(path) = &config.library.path {
                println!(
                    "Note: dnt reads the data folder {} (set in {}); remove library.path there to use the imported data.",
                    path.display(),
                    store.config_path().display()
                );
            }
        }
        Cmd::Backup { file, every, keep, folder } => {
            if every.is_some() || keep.is_some() || folder.is_some() {
                let mut config = store.load_config()?;
                if let Some(folder) = folder {
                    config.backup.folder = if folder.as_os_str() == "default" {
                        None
                    } else {
                        std::fs::create_dir_all(&folder).with_context(|| format!("creating {}", folder.display()))?;
                        Some(std::fs::canonicalize(&folder)?.display().to_string())
                    };
                }
                if let Some(every) = every {
                    config.backup.every = match every.as_str() {
                        "off" => dnt_store::Every::Off,
                        "day" => dnt_store::Every::Day,
                        "month" => dnt_store::Every::Month,
                        _ => dnt_store::Every::Week,
                    };
                }
                if let Some(keep) = keep {
                    config.backup.keep = keep.max(1);
                }
                store.save_config(&config)?;
                println!("Automatic backups: {}", backup::describe(config.backup.every, config.backup.keep));
                println!("Backups folder: {}", backup::Shelf::configured(store, &config).place);
                return Ok(());
            }
            let path = match file {
                Some(path) => {
                    std::fs::write(&path, backup::make(store).map_err(anyhow::Error::msg)?)?;
                    path
                }
                None => backup::save(store, &backup::Shelf::configured(store, config), "").map_err(anyhow::Error::msg)?,
            };
            println!("Backed up to {}", path.display());
        }
        Cmd::Backups => {
            let config = store.load_config()?;
            println!("Automatic backups: {}", backup::describe(config.backup.every, config.backup.keep));
            let shelf = backup::Shelf::configured(store, &config);
            println!("Backups folder: {}", shelf.place);
            for b in backup::list(&shelf) {
                println!("{}  {:.1} MB", b.name, b.bytes as f64 / 1_000_000.0);
            }
        }
        Cmd::Restore { file, yes } => {
            let bytes = std::fs::read(&file).with_context(|| format!("reading {}", file.display()))?;
            let m = backup::check(bytes.clone()).map_err(anyhow::Error::msg)?;
            if !yes {
                println!("This replaces everything here with the backup of {} (made by dnt {}). What is here now is backed up first. Type restore to go on:", m.made, m.dnt);
                let mut answer = String::new();
                std::io::stdin().read_line(&mut answer)?;
                if answer.trim() != "restore" {
                    println!("Nothing was changed.");
                    return Ok(());
                }
            }
            let safety = backup::restore(store, bytes, &backup::Shelf::configured(store, config)).map_err(anyhow::Error::msg)?;
            println!("Restored the backup of {}; what was here is in {}", m.made, safety.display());
        }
        Cmd::Export { what } => return export(Store::open(store.data_dir(), store.config_path()), what),
        Cmd::Imported => {
            for (source, count) in import::sources(store).map_err(anyhow::Error::msg)? {
                println!("{source:<16} {count} entries");
            }
            let brew = import::homebrew(store);
            if !brew.is_empty() {
                println!("\nHomebrew (enable per campaign):");
                for b in brew {
                    println!("{:<16} {} entries  {}", b.id, b.entries(), b.name);
                }
            }
        }
        Cmd::Forget { homebrew: true, .. } => {
            import::clear(store, true).map_err(anyhow::Error::msg)?;
            println!("Removed all imported homebrew");
        }
        Cmd::Forget { all: true, .. } => {
            import::clear(store, false).map_err(anyhow::Error::msg)?;
            println!("Removed all imported data; campaigns, characters and notes stay");
        }
        Cmd::Forget { source: Some(source), .. } => {
            if import::homebrew(store).iter().any(|b| b.id.eq_ignore_ascii_case(&source)) {
                import::remove_homebrew(store, &source).map_err(anyhow::Error::msg)?;
            } else {
                import::remove_source(store, &source).map_err(anyhow::Error::msg)?;
            }
            println!("Removed {source}");
        }
        Cmd::Forget { .. } => unreachable!("clap asks for a source"),
        Cmd::Reset { yes } => {
            if !yes {
                println!(
                    "This deletes everything dnt keeps: the imported data, every campaign with its characters, notes \
                     and homebrew, and the configuration ({}). Type reset to go on:",
                    store.config_path().display()
                );
                let mut answer = String::new();
                std::io::stdin().read_line(&mut answer)?;
                if answer.trim() != "reset" {
                    println!("Nothing was deleted.");
                    return Ok(());
                }
            }
            store.reset()?;
            println!("dnt is reset; it starts as new.");
        }
    }
    Ok(())
}

/// The base library: books, global homebrew (`<data>/library/homebrew/`)
/// and extra homebrew files. Campaign homebrew is layered on a clone of it
/// (see App::prepare_library).
fn load_base(data: Option<&Path>, extra: &[PathBuf], store: &Store) -> Result<Library> {
    let mut base = match data {
        Some(data) => Library::load_dir(data).with_context(|| format!("loading {}", data.display()))?,
        None => dnt_core::import::load(store).map_err(anyhow::Error::msg).context("loading the imported data")?,
    };
    for path in store.global_homebrew_files().iter().chain(extra) {
        base.load_file(path).with_context(|| format!("loading {}", path.display()))?;
    }
    Ok(base)
}

fn main() -> Result<()> {
    let args = Args::parse();
    let store = Store::open_default()?;
    if let Some(cmd) = args.command {
        let config = store.load_config()?;
        return run_command(cmd, store, &config);
    }
    // The automatic backup, before anything reads the settings it notes.
    let shelf = dnt_core::backup::Shelf::configured(&store, &store.load_config()?);
    let backed_up = dnt_core::backup::auto(&store, &shelf, &dnt_core::backup::now());
    let mut config = store.load_config()?;

    // A data folder if one is given or configured, else the imported data.
    let data = match (&args.data, &config.library.path) {
        (Some(path), _) => {
            if config.library.path.as_ref() != Some(path) {
                config.library.path = Some(path.clone());
                store.save_config(&config)?;
            }
            Some(path.clone())
        }
        (None, Some(path)) => Some(path.clone()),
        (None, None) if dnt_core::import::has_imported(&store) => None,
        // No data yet: getting started asks for it.
        (None, None) => match setup::run(&store)? {
            setup::Outcome::Imported => None,
            setup::Outcome::Folder(path) => {
                config.library.path = Some(path.clone());
                store.save_config(&config)?;
                Some(path)
            }
            setup::Outcome::Quit => return Ok(()),
        },
    };
    if data.is_none() {
        // Homebrew imported before it had a store of its own.
        let (mut campaigns, _) = store.campaigns();
        if let Some(p) = dnt_core::workspace::move_old_homebrew(&store, &mut campaigns).first() {
            eprintln!("{p}");
        }
    }

    let start = Instant::now();
    let extra: Vec<PathBuf> = config.library.homebrew.iter().chain(&args.homebrew).cloned().collect();
    let mut base = load_base(data.as_deref(), &extra, &store)?;
    let mut status = match backed_up {
        Some(Ok(done)) => format!("{done} · "),
        Some(Err(e)) => format!("The automatic backup failed: {e} · "),
        None => String::new(),
    };
    status.push_str(&format!("Loaded {} entries in {} ms", base.entities().len(), start.elapsed().as_millis()));
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
                let status = editor_command(&editor, &path).status();
                *terminal = ratatui::init();
                match status {
                    Ok(_) => app.after_edit(&lib, &path),
                    Err(e) => app.status = format!("Could not run {editor}: {e}"),
                }
            }
            if std::mem::take(&mut app.pending_base_reload) {
                match load_base(data.as_deref(), &extra, &app.store) {
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

/// The editor on a file, through the shell: $EDITOR may carry arguments
/// (`code --wait`). The path is passed separately ("$1") to survive spaces.
#[cfg(not(windows))]
fn editor_command(editor: &str, path: &std::path::Path) -> std::process::Command {
    let mut cmd = std::process::Command::new("sh");
    cmd.arg("-c").arg(format!("{editor} \"$1\"")).arg("dnt").arg(path);
    cmd
}

/// The editor on a file, through cmd: $EDITOR may carry arguments. The line
/// goes to cmd as it is, in the quotes cmd strips (Rust's own quoting would
/// escape the path's quotes, which cmd does not read).
#[cfg(windows)]
fn editor_command(editor: &str, path: &std::path::Path) -> std::process::Command {
    use std::os::windows::process::CommandExt;
    let mut cmd = std::process::Command::new("cmd");
    cmd.arg("/C").raw_arg(format!("\"{editor} \"{}\"\"", path.display()));
    cmd
}
