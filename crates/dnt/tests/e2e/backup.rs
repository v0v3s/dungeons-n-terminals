use std::io::Read;

use crate::harness::*;
use crate::seed::Home;

fn stdout(out: &std::process::Output) -> String {
    assert!(out.status.success(), "dnt failed: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &std::process::Output) -> String {
    assert!(!out.status.success(), "dnt should have refused: {}", String::from_utf8_lossy(&out.stdout));
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// The files in a zip, and one of them as text.
fn zip_names(path: &std::path::Path) -> Vec<String> {
    let zip = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
    zip.file_names().filter_map(Result::ok).map(|n| n.into_owned()).collect()
}

fn zip_text(path: &std::path::Path, name: &str) -> String {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
    let mut out = String::new();
    zip.by_name(name).unwrap().read_to_string(&mut out).unwrap();
    out
}

/// A character using the test homebrew's feat, in a campaign that turns
/// the homebrew on.
fn brewer(home: &Home) {
    let mut ws = home.workspace();
    let slug = ws.active_campaign().unwrap().slug.clone();
    ws.set_sources(&slug, Vec::new(), vec!["TSTB".into()], false).unwrap();
    let toml = "name = \"Brewer\"\nspecies = \"race:Stoneling|TST\"\nfeats = [\"feat:Brewed Luck|TSTB\"]\n\n[abilities]\nstr = 10\ndex = 10\ncon = 10\nint = 10\nwis = 10\ncha = 10\n\n[[levels]]\nclass = \"class:Wizard|SRD52\"\n";
    std::fs::write(home.campaign_file("characters").join("brewer.toml"), toml).unwrap();
}

#[test]
fn everything_is_backed_up_and_listed() {
    let home = Home::party();
    let said = stdout(&run("backup.now", &home, &["backup"]));
    assert!(said.starts_with("Backed up to ") && said.contains("dnt-backup-"), "{said}");
    let listed = stdout(&run("backup.now", &home, &["backups"]));
    assert!(listed.contains("Automatic backups: every week, keeping 5") && listed.contains("dnt-backup-"), "{listed}");
    // Somewhere else, by name: the party and the data in it.
    stdout(&run("backup.now", &home, &["backup", "all.zip"]));
    let names = zip_names(&home.path().join("all.zip"));
    for want in ["dnt.toml", "config.toml", "data/campaigns/e2e/characters/tester.toml", "data/library/imported/books.json"] {
        assert!(names.iter().any(|n| n == want), "no {want} in {names:?}");
    }
    assert!(!names.iter().any(|n| n.starts_with("data/backups/")), "backups are not backed up: {names:?}");
}

#[test]
fn a_backup_is_restored_and_what_was_there_is_kept() {
    let home = Home::party();
    stdout(&run("backup.restore", &home, &["backup", "before.zip"]));
    let tester = home.campaign_file("characters/tester.toml");
    std::fs::remove_file(&tester).unwrap();
    // A broken one changes nothing.
    std::fs::write(home.path().join("broken.zip"), b"not a zip").unwrap();
    let why = stderr(&run("backup.restore", &home, &["restore", "broken.zip", "--yes"]));
    assert!(why.contains("Not a zip"), "{why}");
    assert!(!tester.exists());
    let said = stdout(&run("backup.restore", &home, &["restore", "before.zip", "--yes"]));
    assert!(said.contains("Restored the backup of ") && said.contains("before-restore"), "{said}");
    assert!(tester.exists(), "Tester is back");
    let listed = stdout(&run("backup.restore", &home, &["backups"]));
    assert!(listed.contains("-before-restore.zip"), "what was there is backed up: {listed}");
    let mut t = Tui::scenario("backup.restore", home);
    t.send("2").wait("Tester");
}

#[test]
fn backups_are_made_at_start_on_schedule_keeping_the_last_few() {
    let home = Home::party();
    stdout(&run("backup.auto", &home, &["backup", "--every", "day", "--keep", "1"]));
    // The schedule began long ago: a backup at the next start.
    let mut home = home;
    for _ in 0..2 {
        let store = home.store();
        let mut config = store.load_config().unwrap();
        config.backup.last = Some("2020-01-01 10:00".into());
        store.save_config(&config).unwrap();
        let mut t = Tui::scenario("backup.auto", home);
        t.wait("Backed up to dnt-backup-");
        t.quit();
        home = std::mem::replace(&mut t.home, Home::empty());
    }
    let listed = stdout(&run("backup.auto", &home, &["backups"]));
    assert!(listed.contains("every day, keeping 1"), "{listed}");
    assert_eq!(listed.lines().filter(|l| l.starts_with("dnt-backup-")).count(), 1, "only the last one: {listed}");
}

#[test]
fn backups_go_to_the_folder_chosen() {
    let home = Home::party();
    let said = stdout(&run("backup.folder", &home, &["backup", "--folder", "elsewhere/dnt"]));
    let folder = home.path().join("elsewhere/dnt");
    assert!(said.contains(&format!("Backups folder: {}", folder.canonicalize().unwrap().display())), "{said}");
    stdout(&run("backup.folder", &home, &["backup"]));
    let listed = stdout(&run("backup.folder", &home, &["backups"]));
    assert_eq!(listed.lines().filter(|l| l.starts_with("dnt-backup-")).count(), 1, "{listed}");
    // The automatic one goes there too.
    let store = home.store();
    let mut config = store.load_config().unwrap();
    config.backup.last = Some("2020-01-01 10:00".into());
    store.save_config(&config).unwrap();
    let mut t = Tui::scenario("backup.folder", home);
    t.wait("Backed up to dnt-backup-");
    t.quit();
    let home = std::mem::replace(&mut t.home, Home::empty());
    let mine = |dir: &std::path::Path| std::fs::read_dir(dir).map(|d| d.count()).unwrap_or(0);
    assert_eq!(mine(&folder), 2);
    assert_eq!(mine(&home.path().join("data/backups")), 0, "nothing in the default folder");
    // Back to the default folder.
    let said = stdout(&run("backup.folder", &home, &["backup", "--folder", "default"]));
    let default = std::path::Path::new("data").join("backups");
    assert!(said.contains("Backups folder: ") && said.trim_end().ends_with(&*default.to_string_lossy()), "{said}");
}

#[test]
fn a_character_goes_with_what_they_use() {
    let home = Home::party();
    brewer(&home);
    stdout(&run("export.character", &home, &["export", "character", "Brewer", "-o", "brewer.dnt.zip"]));
    let zip = home.path().join("brewer.dnt.zip");
    let names = zip_names(&zip);
    assert!(names.contains(&"character.toml".to_string()), "{names:?}");
    let manifest = zip_text(&zip, "dnt.toml");
    assert!(manifest.contains("kind = \"character\"") && manifest.contains("needs = [\"SRD52\", \"TST\"]"), "{manifest}");
    // The homebrew feat, not the rest of that homebrew.
    let brew = zip_text(&zip, "homebrew/tstb.json");
    assert!(brew.contains("Brewed Luck") && !brew.contains("Sun Glint"), "{brew}");

    // Into another campaign of another device: the homebrew turns on.
    let other = Home::campaign();
    std::fs::copy(&zip, other.path().join("brewer.dnt.zip")).unwrap();
    let said = stdout(&run("export.character", &other, &["import", "brewer.dnt.zip"]));
    assert!(said.starts_with("Imported Brewer, 1 homebrew source"), "{said}");
    let campaign = std::fs::read_to_string(other.campaign_file("campaign.toml")).unwrap();
    assert!(campaign.contains("TSTB"), "{campaign}");
    let mut t = Tui::scenario("export.character", other);
    t.send("2").wait("Brewer").send(ENTER).wait("◆ Brewer").send(TAB).wait("Brewed Luck");
}

#[test]
fn a_campaign_moves_with_all_it_has() {
    let home = Home::party();
    stdout(&run("export.campaign", &home, &["export", "campaign", "E2E", "-o", "e2e.dnt.zip"]));
    let other = Home::data();
    std::fs::copy(home.path().join("e2e.dnt.zip"), other.path().join("e2e.dnt.zip")).unwrap();
    let said = stdout(&run("export.campaign", &other, &["import", "e2e.dnt.zip"]));
    assert!(said.starts_with("Imported the campaign E2E"), "{said}");
    // With no campaign there, it is the one open.
    let mut t = Tui::scenario("export.campaign", other);
    t.wait("◈ E2E");
    t.send("2").wait("Tester").wait("Hexa");
}

#[test]
fn an_entry_is_exported_and_imported() {
    let home = Home::party();
    brewer(&home);
    stdout(&run("export.entry", &home, &["export", "entry", "spell:Sun Glint|TSTB", "-o", "glint.json"]));
    let text = std::fs::read_to_string(home.path().join("glint.json")).unwrap();
    assert!(text.contains("\"Sun Glint\"") && text.contains("Proving Grounds Brew"), "{text}");
    // A campaign without that homebrew: it turns on there.
    let other = Home::campaign();
    std::fs::write(other.path().join("glint.json"), &text).unwrap();
    assert_eq!(stdout(&run("export.entry", &other, &["import", "glint.json"])).trim(), "Imported Sun Glint");
    assert!(std::fs::read_to_string(other.campaign_file("campaign.toml")).unwrap().contains("TSTB"));
    // A book's entry is there already.
    stdout(&run("export.entry", &home, &["export", "entry", "spell:Ray of Frost|SRD52", "-o", "ray.json"]));
    std::fs::copy(home.path().join("ray.json"), other.path().join("ray.json")).unwrap();
    assert_eq!(stdout(&run("export.entry", &other, &["import", "ray.json"])).trim(), "Ray of Frost is in your data already");
    let mut t = Tui::scenario("export.entry", other);
    t.send("1").wait("Browse").send("/").typed("sun glint").wait("Sun Glint");
}

#[test]
fn a_clash_is_replaced_or_both_kept() {
    let home = Home::party();
    stdout(&run("import.clash", &home, &["export", "character", "Tester", "-o", "tester.dnt.zip"]));
    let why = stderr(&run("import.clash", &home, &["import", "tester.dnt.zip"]));
    assert!(why.contains("Tester is here already: import again with --replace or --keep-both"), "{why}");
    let said = stdout(&run("import.clash", &home, &["import", "tester.dnt.zip", "--keep-both"]));
    assert!(said.starts_with("Imported Tester (2)"), "{said}");
    stdout(&run("import.clash", &home, &["import", "tester.dnt.zip", "--replace"]));
    let mut t = Tui::scenario("import.clash", home);
    t.send("2").wait("Tester (2)");
}
