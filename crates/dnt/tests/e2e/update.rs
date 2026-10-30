use crate::harness::*;
use crate::seed::{self, Home};

/// A release of the tests (e2e/update): `newer`, `older` or `broken`.
fn release(which: &str) -> (&'static str, String) {
    ("DNT_UPDATE_URL", seed::e2e_dir().join("update").join(which).display().to_string())
}

/// The release file this system updates to.
fn file() -> &'static str {
    if cfg!(windows) {
        "dnt-windows-x86_64.exe"
    } else {
        "dnt-linux-x86_64"
    }
}

fn say(out: &std::process::Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr))
}

#[test]
fn a_newer_dnt_and_what_is_new_in_it() {
    let home = Home::empty();
    let said = say(&run_with_env("update.check", &home, &["update", "--check"], &[release("newer")]));
    assert!(said.contains(&format!("dnt 99.0.0 is out (you have {})", env!("CARGO_PKG_VERSION"))), "{said}");
    assert!(said.contains("- Everything the tests need.") && said.contains("Update with: dnt update"), "{said}");
    let said = say(&run("update.check", &home, &["update", "--check"]));
    assert!(said.contains(&format!("dnt {} is the newest", env!("CARGO_PKG_VERSION"))), "{said}");
}

#[test]
fn updating_replaces_the_program_with_the_checked_download() {
    let home = Home::empty();
    // A copy of dnt stands in for the installed one.
    let exe = home.path().join(if cfg!(windows) { "dnt.exe" } else { "dnt" });
    std::fs::copy(env!("CARGO_BIN_EXE_dnt"), &exe).unwrap();
    let to = ("DNT_UPDATE_EXE", exe.display().to_string());
    // A download that does not match its SHA256SUMS is not installed.
    let said = say(&run_with_env("update.install", &home, &["update"], &[release("broken"), to.clone()]));
    assert!(said.contains(&format!("{} does not match its SHA256SUMS: not installed", file())), "{said}");
    assert_eq!(std::fs::read(&exe).unwrap(), std::fs::read(env!("CARGO_BIN_EXE_dnt")).unwrap());
    // The real one replaces it.
    let said = say(&run_with_env("update.install", &home, &["update"], &[release("newer"), to]));
    assert!(said.contains(&format!("Updated to dnt 99.0.0: {}", exe.display())), "{said}");
    let new = seed::e2e_dir().join("update/newer").join(file());
    assert_eq!(std::fs::read(&exe).unwrap(), std::fs::read(new).unwrap());
}

#[test]
fn the_daily_check_says_a_newer_dnt_is_out_and_can_be_turned_off() {
    let home = Home::empty();
    let mut t = Tui::with_env("update.auto", home, &[release("newer")]);
    t.wait("dnt 99.0.0 is out: dnt update");
    t.quit();
    // Once a day: not again today.
    let home = std::mem::replace(&mut t.home, Home::empty());
    let config = std::fs::read_to_string(home.path().join("config.toml")).unwrap();
    assert!(config.contains("checked = "), "{config}");
    let t = Tui::with_env("update.auto", home, &[release("newer")]);
    assert!(!t.shows("is out"), "{}", t.screen());
    drop(t);
    // Off: no check at all.
    let home = Home::empty();
    let said = say(&run_with_env("update.auto", &home, &["update", "--auto", "off"], &[]));
    assert!(said.contains("dnt no longer checks for a newer version by itself"), "{said}");
    let t = Tui::with_env("update.auto", home, &[release("newer")]);
    t.settle();
    assert!(!t.shows("is out"), "{}", t.screen());
    let config = std::fs::read_to_string(t.home.path().join("config.toml")).unwrap();
    assert!(config.contains("auto = false") && !config.contains("checked"), "{config}");
}
