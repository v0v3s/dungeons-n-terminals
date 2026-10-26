//! The known states a scenario starts from, made through the same core the
//! TUI uses: nothing, the test data imported, a campaign, the test party
//! (`e2e/characters.json`).

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use dnt_core::creation::NewCharacter;
use dnt_core::flow::{Answer, Question};
use dnt_core::workspace::Workspace;
use dnt_store::Store;
use serde_json::Value;

/// The repository's `e2e` folder: the data, the party, the scenarios.
pub fn e2e_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../e2e")
}

/// The test data and homebrew as one zip, packed once.
pub fn data_zip() -> &'static [u8] {
    static ZIP: OnceLock<Vec<u8>> = OnceLock::new();
    ZIP.get_or_init(|| {
        let dir = e2e_dir();
        let mut out = Cursor::new(Vec::new());
        dnt_core::pack::pack(Some(&dir.join("data")), Some(&dir.join("homebrew")), &mut out).expect("the test data packs");
        out.into_inner()
    })
}

/// A home for one run of `dnt` (`DNT_HOME`), deleted afterwards.
pub struct Home {
    dir: tempfile::TempDir,
}

impl Home {
    /// Nothing at all: the first start.
    pub fn empty() -> Home {
        Home { dir: tempfile::tempdir().expect("a temporary folder") }
    }

    /// The test data imported, no campaign yet.
    pub fn data() -> Home {
        let home = Home::empty();
        dnt_core::import::import_zip(&home.store(), data_zip().to_vec()).expect("the test data imports");
        home
    }

    /// The test data and the active campaign.
    pub fn campaign() -> Home {
        let home = Home::data();
        let mut ws = home.workspace();
        ws.create_campaign(party()["campaign"].as_str().unwrap(), Vec::new()).expect("the campaign");
        home
    }

    /// The campaign and the test party in it.
    pub fn party() -> Home {
        let home = Home::campaign();
        let ws = home.workspace();
        for c in party()["characters"].as_array().unwrap() {
            let prefer: Vec<&str> = c["prefer"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
            create(&ws, c["name"].as_str().unwrap(), &prefer);
        }
        home
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    pub fn store(&self) -> Store {
        Store::open(self.path().join("data"), self.path().join("config.toml"))
    }

    pub fn workspace(&self) -> Workspace {
        Workspace::open(self.store())
    }

    /// A file in the active campaign's folder (`notes/Inbox.md`, …).
    pub fn campaign_file(&self, rel: &str) -> PathBuf {
        let ws = self.workspace();
        ws.active_campaign().expect("an active campaign").dir.join(rel)
    }
}

fn party() -> &'static Value {
    static PARTY: OnceLock<Value> = OnceLock::new();
    PARTY.get_or_init(|| {
        let text = std::fs::read_to_string(e2e_dir().join("characters.json")).expect("e2e/characters.json");
        serde_json::from_str(&text).expect("e2e/characters.json is JSON")
    })
}

/// Make a character through character creation, answering as
/// `e2e/characters.json` describes.
fn create(ws: &Workspace, name: &str, prefer: &[&str]) {
    let lib = ws.lib.as_ref().expect("the library");
    let filter = ws.filter();
    let mut flow = NewCharacter::new(name.to_string(), lib, &filter);
    for _ in 0..60 {
        let answer = match flow.question().clone() {
            Question::Confirm => {
                let character = flow.finish().expect("the character is complete");
                let campaign = ws.active_campaign().expect("the campaign");
                ws.store.create_character(campaign, character).expect("the character is saved");
                return;
            }
            Question::Pick { choices, count, .. } => {
                let wanted = |label: &str| prefer.iter().any(|p| *p == label || (p.ends_with(':') && label.starts_with(p)));
                let mut values: Vec<String> = choices.iter().filter(|c| wanted(&c.label)).take(count).map(|c| c.value.clone()).collect();
                for c in &choices {
                    if values.len() >= count {
                        break;
                    }
                    if !values.contains(&c.value) {
                        values.push(c.value.clone());
                    }
                }
                Answer::Pick(values)
            }
            Question::Abilities { scores } => Answer::Scores(scores.scores),
            Question::Number { .. } => Answer::Number(1),
        };
        flow.answer(lib, &filter, answer).unwrap_or_else(|e| panic!("creating {name}: {e}"));
    }
    panic!("creating {name} did not finish");
}
