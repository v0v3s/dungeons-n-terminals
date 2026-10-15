//! Campaign notes: plain Markdown files with YAML frontmatter and
//! `[[wiki links]]`, under `campaigns/<slug>/notes/`.
//!
//! Paths in this module are relative to the notes root and use `/`.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::{Result, StoreError};

/// Folders created for a new campaign, in their default order.
pub const DEFAULT_FOLDERS: &[&str] = &["Sessions", "NPCs", "Quests", "Places", "Party", "Factions", "Lore"];

/// Files at the notes root that always come first, in this order.
const PINNED: &[&str] = &["Campaign.md", "Inbox.md"];

/// Built-in templates: (name, contents). Folder templates are named after
/// their folder; `Campaign`, `Inbox`, `Party` and `Character` are used for
/// specific files and `Note` for anything else.
pub const TEMPLATES: &[(&str, &str)] = &[
    ("Campaign", include_str!("../templates/Campaign.md")),
    ("Inbox", include_str!("../templates/Inbox.md")),
    ("Sessions", include_str!("../templates/Sessions.md")),
    ("NPCs", include_str!("../templates/NPCs.md")),
    ("Quests", include_str!("../templates/Quests.md")),
    ("Places", include_str!("../templates/Places.md")),
    ("Factions", include_str!("../templates/Factions.md")),
    ("Lore", include_str!("../templates/Lore.md")),
    ("Party", include_str!("../templates/Party.md")),
    ("Character", include_str!("../templates/Character.md")),
    ("Note", include_str!("../templates/Note.md")),
];

/// Values substituted into templates (`{{title}}`, `{{date}}`, …).
#[derive(Debug, Clone, Default)]
pub struct Vars {
    pub title: String,
    pub date: String,
    pub campaign: String,
    pub character: String,
}

impl Vars {
    pub fn apply(&self, text: &str) -> String {
        text.replace("{{title}}", &self.title)
            .replace("{{date}}", &self.date)
            .replace("{{campaign}}", &self.campaign)
            .replace("{{character}}", &self.character)
    }
}

/// One entry in a folder listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Path relative to the notes root, e.g. `NPCs/Strahd.md`.
    pub path: String,
    /// File stem or folder name.
    pub name: String,
    pub is_dir: bool,
}

/// A campaign's notes folder.
#[derive(Debug, Clone)]
pub struct Notes {
    pub root: PathBuf,
    /// Global template overrides (`<data>/templates`).
    global_templates: PathBuf,
    /// Folder order (top level); unlisted folders follow alphabetically.
    pub order: Vec<String>,
}

fn io_err(path: &Path) -> impl FnOnce(io::Error) -> StoreError + '_ {
    move |source| StoreError::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// Characters not allowed in note file names.
pub fn file_safe(title: &str) -> String {
    title
        .chars()
        .map(|c| if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '#' | '^' | '[' | ']') { '-' } else { c })
        .collect::<String>()
        .trim()
        .trim_start_matches('.')
        .to_string()
}

impl Notes {
    pub fn new(root: impl Into<PathBuf>, global_templates: impl Into<PathBuf>, order: Vec<String>) -> Notes {
        Notes {
            root: root.into(),
            global_templates: global_templates.into(),
            order,
        }
    }

    pub fn abs(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    fn has_notes(&self) -> bool {
        fs::read_dir(&self.root)
            .map(|mut d| d.any(|e| e.is_ok_and(|e| !e.file_name().to_string_lossy().starts_with('.'))))
            .unwrap_or(false)
    }

    /// Create the default structure if the notes folder is empty.
    /// Returns whether anything was created.
    pub fn scaffold(&self, vars: &Vars) -> Result<bool> {
        if self.has_notes() {
            return Ok(false);
        }
        for folder in DEFAULT_FOLDERS {
            let dir = self.root.join(folder);
            fs::create_dir_all(&dir).map_err(io_err(&dir))?;
        }
        self.write_new("Campaign.md", &self.template("Campaign"), vars)?;
        self.write_new("Inbox.md", &self.template("Inbox"), vars)?;
        self.write_new("Party/Party.md", &self.template("Party"), vars)?;
        Ok(true)
    }

    fn write_new(&self, rel: &str, template: &str, vars: &Vars) -> Result<()> {
        let path = self.abs(rel);
        if !path.exists() {
            self.write(rel, &vars.apply(template))?;
        }
        Ok(())
    }

    /// The template for `name`: campaign override, then global override,
    /// then the built-in default (falling back to `Note`).
    pub fn template(&self, name: &str) -> String {
        for dir in [self.root.join(".templates"), self.global_templates.clone()] {
            if let Ok(t) = fs::read_to_string(dir.join(format!("{name}.md"))) {
                return t;
            }
        }
        TEMPLATES
            .iter()
            .find(|(n, _)| *n == name)
            .or_else(|| TEMPLATES.iter().find(|(n, _)| *n == "Note"))
            .map(|(_, t)| t.to_string())
            .unwrap_or_default()
    }

    /// Template used for a new note inside `folder` (its top-level folder).
    pub fn template_for(&self, folder: &str) -> String {
        let top = folder.split('/').next().unwrap_or("");
        let name = match top {
            "" => "Note",
            "Party" => "Character",
            other => other,
        };
        // A user folder without its own template uses the generic one.
        let custom = [self.root.join(".templates"), self.global_templates.clone()]
            .iter()
            .any(|d| d.join(format!("{name}.md")).is_file());
        if custom || TEMPLATES.iter().any(|(n, _)| *n == name) {
            self.template(name)
        } else {
            self.template("Note")
        }
    }

    /// Entries of a folder (`""` = root): folders first in the configured
    /// order, then notes (pinned root files first; sessions newest first).
    pub fn list(&self, folder: &str) -> Vec<Entry> {
        let dir = self.abs(folder);
        let mut dirs = Vec::new();
        let mut files = Vec::new();
        for e in fs::read_dir(&dir).into_iter().flatten().flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let path = if folder.is_empty() { name.clone() } else { format!("{folder}/{name}") };
            if e.path().is_dir() {
                dirs.push(Entry { path, name, is_dir: true });
            } else if name.ends_with(".md") {
                files.push(Entry {
                    path,
                    name: name.trim_end_matches(".md").to_string(),
                    is_dir: false,
                });
            }
        }
        let rank = |name: &str| self.order.iter().position(|o| o == name).unwrap_or(usize::MAX);
        if folder.is_empty() {
            dirs.sort_by(|a, b| rank(&a.name).cmp(&rank(&b.name)).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
        } else {
            dirs.sort_by_key(|d| d.name.to_lowercase());
        }
        let pinned = |p: &str| PINNED.iter().position(|x| *x == p).unwrap_or(usize::MAX);
        if folder.split('/').next() == Some("Sessions") {
            files.sort_by(|a, b| b.name.cmp(&a.name));
        } else {
            files.sort_by(|a, b| pinned(&a.path).cmp(&pinned(&b.path)).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
        }
        // At the root, the hub and inbox come before the folders.
        let (mut out, rest): (Vec<Entry>, Vec<Entry>) = files.into_iter().partition(|f| pinned(&f.path) != usize::MAX);
        out.extend(dirs);
        out.extend(rest);
        out
    }

    /// Top-level folders in display order.
    pub fn folders(&self) -> Vec<String> {
        self.list("").into_iter().filter(|e| e.is_dir).map(|e| e.name).collect()
    }

    /// Move a top-level folder up (`-1`) or down (`+1`); returns the new order.
    pub fn move_folder(&mut self, name: &str, delta: isize) -> Vec<String> {
        let mut order = self.folders();
        if let Some(i) = order.iter().position(|f| f == name) {
            let j = (i as isize + delta).clamp(0, order.len() as isize - 1) as usize;
            order.swap(i, j);
        }
        self.order = order.clone();
        order
    }

    pub fn read(&self, rel: &str) -> Result<String> {
        let path = self.abs(rel);
        fs::read_to_string(&path).map_err(io_err(&path))
    }

    pub fn write(&self, rel: &str, text: &str) -> Result<()> {
        let path = self.abs(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(io_err(parent))?;
        }
        let tmp = path.with_extension("md.tmp");
        fs::write(&tmp, text).map_err(io_err(&tmp))?;
        fs::rename(&tmp, &path).map_err(io_err(&path))
    }

    /// A path in `folder` for `title` that does not exist yet.
    fn free_path(&self, folder: &str, title: &str) -> String {
        let base = file_safe(title);
        let base = if base.is_empty() { "Untitled".to_string() } else { base };
        let join = |name: &str| if folder.is_empty() { format!("{name}.md") } else { format!("{folder}/{name}.md") };
        let mut rel = join(&base);
        let mut n = 2;
        while self.abs(&rel).exists() {
            rel = join(&format!("{base} ({n})"));
            n += 1;
        }
        rel
    }

    /// Create a note in `folder` from the folder's template. Returns its path.
    pub fn create(&self, folder: &str, vars: &Vars) -> Result<String> {
        let rel = self.free_path(folder, &vars.title);
        self.write(&rel, &vars.apply(&self.template_for(folder)))?;
        Ok(rel)
    }

    /// Create today's session note, named by date.
    pub fn new_session(&self, vars: &Vars) -> Result<String> {
        let vars = Vars {
            title: vars.date.clone(),
            ..vars.clone()
        };
        self.create("Sessions", &vars)
    }

    pub fn create_folder(&self, parent: &str, name: &str) -> Result<String> {
        let name = file_safe(name);
        let rel = if parent.is_empty() { name } else { format!("{parent}/{name}") };
        let path = self.abs(&rel);
        fs::create_dir_all(&path).map_err(io_err(&path))?;
        Ok(rel)
    }

    /// Rename a note (keeping its folder and `.md`) or a folder.
    pub fn rename(&self, rel: &str, new_name: &str) -> Result<String> {
        let from = self.abs(rel);
        let parent = rel.rsplit_once('/').map_or("", |(p, _)| p);
        let is_dir = from.is_dir();
        let name = file_safe(new_name);
        let to_rel = match (parent.is_empty(), is_dir) {
            (true, true) => name,
            (false, true) => format!("{parent}/{name}"),
            (true, false) => format!("{name}.md"),
            (false, false) => format!("{parent}/{name}.md"),
        };
        let to = self.abs(&to_rel);
        if to.exists() {
            return Err(StoreError::Io {
                path: to,
                source: io::Error::new(io::ErrorKind::AlreadyExists, "already exists"),
            });
        }
        fs::rename(&from, &to).map_err(io_err(&from))?;
        Ok(to_rel)
    }

    /// Delete a note, or a folder with everything in it.
    pub fn delete(&self, rel: &str) -> Result<()> {
        let path = self.abs(rel);
        if path.is_dir() {
            fs::remove_dir_all(&path).map_err(io_err(&path))
        } else {
            fs::remove_file(&path).map_err(io_err(&path))
        }
    }

    /// Append a timestamped line to `Inbox.md`.
    pub fn capture(&self, time: &str, text: &str) -> Result<()> {
        let mut body = self.read("Inbox.md").unwrap_or_else(|_| self.template("Inbox"));
        if !body.ends_with('\n') {
            body.push('\n');
        }
        body.push_str(&format!("- {time} {}\n", text.trim()));
        self.write("Inbox.md", &body)
    }

    /// Every note, recursively, with its parsed metadata.
    pub fn index(&self) -> Vec<NoteInfo> {
        let mut out = Vec::new();
        let mut stack = vec![String::new()];
        while let Some(folder) = stack.pop() {
            for e in self.list(&folder) {
                if e.is_dir {
                    stack.push(e.path);
                } else if let Ok(text) = self.read(&e.path) {
                    out.push(NoteInfo::parse(&e.path, &text));
                }
            }
        }
        out.sort_by(|a, b| a.path.cmp(&b.path));
        out
    }
}

/// A `[[link]]` in a note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WikiLink {
    /// Link target before any `|alias`, e.g. `Strahd` or `spell:Fireball`.
    pub target: String,
    pub alias: Option<String>,
    /// Line (0-based) and byte range of the whole `[[…]]` within it.
    pub line: usize,
    pub start: usize,
    pub end: usize,
}

impl WikiLink {
    /// For `kind:Name` compendium links, the kind prefix and the name.
    pub fn compendium(&self) -> Option<(&str, &str)> {
        let (kind, name) = self.target.split_once(':')?;
        let kind = kind.trim();
        (!kind.is_empty() && kind.chars().all(|c| c.is_ascii_alphabetic())).then_some((kind, name.trim()))
    }
}

/// All `[[links]]` in a text.
pub fn links(text: &str) -> Vec<WikiLink> {
    let mut out = Vec::new();
    for (line_no, line) in text.lines().enumerate() {
        let mut rest = 0;
        while let Some(open) = line[rest..].find("[[") {
            let start = rest + open;
            let Some(close) = line[start + 2..].find("]]") else { break };
            let end = start + 2 + close + 2;
            let inner = &line[start + 2..end - 2];
            let (target, alias) = match inner.split_once('|') {
                Some((t, a)) => (t.trim().to_string(), Some(a.trim().to_string())),
                None => (inner.trim().to_string(), None),
            };
            if !target.is_empty() {
                out.push(WikiLink {
                    target,
                    alias,
                    line: line_no,
                    start,
                    end,
                });
            }
            rest = end;
        }
    }
    out
}

/// Flat YAML frontmatter: `key: value` and `key: [a, b]` lines between
/// `---` fences. Values are returned as strings (lists joined by `, `).
pub fn frontmatter(text: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some("---") {
        return out;
    }
    for line in lines {
        if line.trim() == "---" {
            break;
        }
        let Some((k, v)) = line.split_once(':') else { continue };
        let v = v.split(" #").next().unwrap_or(v).trim();
        let unquote = |p: &str| p.trim().trim_matches('"').trim_matches('\'').to_string();
        // `[a, b]` is a list; `[[link]]` is not.
        let v = if v.starts_with('[') && !v.starts_with("[[") {
            v.trim_start_matches('[').trim_end_matches(']').split(',').map(unquote).collect::<Vec<_>>().join(", ")
        } else {
            unquote(v)
        };
        out.insert(k.trim().to_string(), v);
    }
    out
}

/// Metadata of one note, for backlinks and generated lists.
#[derive(Debug, Clone, PartialEq)]
pub struct NoteInfo {
    pub path: String,
    pub title: String,
    pub meta: BTreeMap<String, String>,
    /// Targets of note links (compendium links excluded).
    pub links: Vec<String>,
}

impl NoteInfo {
    fn parse(path: &str, text: &str) -> NoteInfo {
        let title = path.rsplit('/').next().unwrap_or(path).trim_end_matches(".md").to_string();
        NoteInfo {
            path: path.to_string(),
            title,
            meta: frontmatter(text),
            links: links(text).into_iter().filter(|l| l.compendium().is_none()).map(|l| l.target).collect(),
        }
    }
}

/// Find the note a link target names: by title (case-insensitive), or by
/// path (`NPCs/Strahd`).
pub fn resolve<'a>(index: &'a [NoteInfo], target: &str) -> Option<&'a NoteInfo> {
    let t = target.trim().trim_end_matches(".md").to_lowercase();
    index
        .iter()
        .find(|n| n.path.trim_end_matches(".md").to_lowercase() == t)
        .or_else(|| index.iter().find(|n| n.title.to_lowercase() == t))
}

/// Notes linking to `note`.
pub fn backlinks<'a>(index: &'a [NoteInfo], note: &NoteInfo) -> Vec<&'a NoteInfo> {
    index
        .iter()
        .filter(|n| n.path != note.path)
        .filter(|n| n.links.iter().any(|l| resolve(index, l).is_some_and(|r| r.path == note.path)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notes() -> (tempfile::TempDir, Notes) {
        let dir = tempfile::tempdir().unwrap();
        let n = Notes::new(dir.path().join("notes"), dir.path().join("templates"), Vec::new());
        (dir, n)
    }

    fn vars(title: &str) -> Vars {
        Vars {
            title: title.into(),
            date: "2026-10-15".into(),
            campaign: "Curse of Strahd".into(),
            character: String::new(),
        }
    }

    #[test]
    fn scaffold_and_order() {
        let (_tmp, mut n) = notes();
        assert!(n.scaffold(&vars("")).unwrap());
        assert!(!n.scaffold(&vars("")).unwrap(), "only once");
        assert!(n.read("Campaign.md").unwrap().contains("# Curse of Strahd"));
        // Default order comes from DEFAULT_FOLDERS when configured so.
        n.order = DEFAULT_FOLDERS.iter().map(|s| s.to_string()).collect();
        n.create_folder("", "Downtime").unwrap();
        assert_eq!(n.folders(), vec!["Sessions", "NPCs", "Quests", "Places", "Party", "Factions", "Lore", "Downtime"]);
        assert_eq!(n.move_folder("Quests", -1)[1], "Quests");
        assert_eq!(n.folders()[1], "Quests");
        let root: Vec<String> = n.list("").into_iter().filter(|e| !e.is_dir).map(|e| e.name).collect();
        assert_eq!(root, vec!["Campaign", "Inbox"]);
    }

    #[test]
    fn templates_and_sessions() {
        let (tmp, n) = notes();
        n.scaffold(&vars("")).unwrap();
        let npc = n.create("NPCs", &vars("Ismark Kolyanovich")).unwrap();
        assert_eq!(npc, "NPCs/Ismark Kolyanovich.md");
        let text = n.read(&npc).unwrap();
        assert!(text.contains("type: npc") && text.contains("# Ismark Kolyanovich"));

        let s1 = n.new_session(&vars("")).unwrap();
        let s2 = n.new_session(&vars("")).unwrap();
        assert_eq!((s1.as_str(), s2.as_str()), ("Sessions/2026-10-15.md", "Sessions/2026-10-15 (2).md"));

        // A campaign override wins over the built-in template.
        std::fs::create_dir_all(n.root.join(".templates")).unwrap();
        std::fs::write(n.root.join(".templates/NPCs.md"), "# {{title}} (custom)\n").unwrap();
        let custom = n.create("NPCs", &vars("Strahd")).unwrap();
        assert_eq!(n.read(&custom).unwrap(), "# Strahd (custom)\n");
        // A user folder without a template uses the generic note.
        n.create_folder("", "Downtime").unwrap();
        let d = n.create("Downtime", &vars("Crafting")).unwrap();
        assert_eq!(n.read(&d).unwrap(), "# Crafting\n\n");
        drop(tmp);
    }

    #[test]
    fn links_backlinks_and_frontmatter() {
        let text = "---\ntype: npc\ntags: [npc, villain]\nlocation: \"[[Castle Ravenloft]]\" # where\n---\nMet [[Strahd|the count]] and cast [[spell:Fireball]].";
        let ls = links(text);
        assert_eq!(ls.len(), 3);
        assert_eq!(ls[1].target, "Strahd");
        assert_eq!(ls[1].alias.as_deref(), Some("the count"));
        assert_eq!(ls[2].compendium(), Some(("spell", "Fireball")));
        let fm = frontmatter(text);
        assert_eq!(fm["tags"], "npc, villain");
        assert_eq!(fm["location"], "[[Castle Ravenloft]]");

        let (_tmp, n) = notes();
        n.scaffold(&vars("")).unwrap();
        n.write("NPCs/Strahd.md", "# Strahd").unwrap();
        n.write("Sessions/2026-10-15.md", "We met [[strahd]].").unwrap();
        let index = n.index();
        let strahd = resolve(&index, "Strahd").unwrap();
        let back: Vec<&str> = backlinks(&index, strahd).iter().map(|b| b.path.as_str()).collect();
        assert_eq!(back, vec!["Sessions/2026-10-15.md"]);
        assert!(resolve(&index, "NPCs/Strahd").is_some());
    }

    #[test]
    fn rename_delete_capture() {
        let (_tmp, n) = notes();
        n.scaffold(&vars("")).unwrap();
        let p = n.create("Places", &vars("Vallaki")).unwrap();
        let q = n.rename(&p, "Vallaki Town").unwrap();
        assert_eq!(q, "Places/Vallaki Town.md");
        n.delete(&q).unwrap();
        assert!(!n.abs(&q).exists());
        n.capture("19:30", "Ireena knows the burgomaster").unwrap();
        assert!(n.read("Inbox.md").unwrap().ends_with("- 19:30 Ireena knows the burgomaster\n"));
        assert_eq!(file_safe("A/B: c?"), "A-B- c-");
    }
}
