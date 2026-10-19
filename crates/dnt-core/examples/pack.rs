//! Pack a data folder and/or a homebrew collection into a zip for the
//! app's import (see `scripts/pack-data.sh`):
//! `cargo run --release -p dnt-core --example pack -- [--data DIR] [--homebrew DIR] OUT.zip`.

use std::path::PathBuf;
use std::time::Instant;

fn main() {
    let (mut data, mut homebrew, mut out) = (None, None, None);
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--data" => data = args.next().map(PathBuf::from),
            "--homebrew" => homebrew = args.next().map(PathBuf::from),
            _ => out = Some(PathBuf::from(arg)),
        }
    }
    let (Some(out), true) = (out, data.is_some() || homebrew.is_some()) else {
        eprintln!("usage: pack [--data DIR] [--homebrew DIR] OUT.zip");
        std::process::exit(2);
    };
    let t = Instant::now();
    let file = std::fs::File::create(&out).unwrap_or_else(|e| {
        eprintln!("{}: {e}", out.display());
        std::process::exit(1);
    });
    match dnt_core::pack::pack(data.as_deref(), homebrew.as_deref(), std::io::BufWriter::new(file)) {
        Ok(p) => {
            let size = std::fs::metadata(&out).map_or(0, |m| m.len());
            println!(
                "{}: {} data files, {} homebrew files ({} without anything dnt reads), {:.1} MB in {:.1?}",
                out.display(),
                p.data_files,
                p.homebrew_files,
                p.skipped,
                size as f64 / 1e6,
                t.elapsed()
            );
        }
        Err(e) => {
            let _ = std::fs::remove_file(&out);
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
