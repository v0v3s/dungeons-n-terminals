//! The version of the files dnt writes (characters, campaigns, the
//! config): `format = 1` on their first line. A file without it is from
//! before formats were numbered and reads as format 1. A file from a newer
//! format is refused rather than read, so saving it again cannot lose what
//! this version does not know about.

use serde::Deserialize;

/// The format this version writes, and the newest it reads.
pub const FORMAT: u32 = 1;

#[derive(Deserialize)]
struct Stamp {
    #[serde(default = "first")]
    format: u32,
}

fn first() -> u32 {
    1
}

/// Refuse a file written in a newer format. A file that is not valid TOML
/// passes, for its own parser to report.
pub fn check(text: &str) -> Result<(), String> {
    match toml::from_str::<Stamp>(text) {
        Ok(Stamp { format }) if format > FORMAT => {
            Err(format!("written by a newer dnt (file format {format}; this one reads up to {FORMAT}): update dnt to open it"))
        }
        _ => Ok(()),
    }
}

/// A file's text with the format on its first line.
pub fn stamp(text: &str) -> String {
    format!("format = {FORMAT}\n{text}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unnumbered_and_current_files_read_newer_ones_do_not() {
        assert!(check("name = \"Old\"").is_ok());
        assert!(check(&stamp("name = \"Now\"")).is_ok());
        let newer = check("format = 2\nname = \"Later\"").unwrap_err();
        assert!(newer.contains("newer dnt"), "{newer}");
        assert!(check("not toml [").is_ok());
    }
}
