Everything is plain files you can read, edit and back up.

```
~/.config/dnt/config.toml            the data path and the last campaign
~/.local/share/dnt/
  campaigns/<campaign>/
    campaign.toml                    name, sources, notes order
    characters/<name>.toml           one file per character
    homebrew.json                    the campaign's homebrew
    notes/                           Markdown notes
    exports/                         exported sheets
  library/
    imported/                        data imported from zips
    homebrew/*.json                  homebrew shared by every campaign
    overlays/*.toml                  your rules for features
  templates/*.md                     your note templates
```

- On macOS both are under `~/Library/Application Support/dnt`; on Windows
  under `%APPDATA%\dnt`. `XDG_DATA_HOME` and `XDG_CONFIG_HOME` move them on
  Linux.
- **`DNT_HOME`** moves everything into one folder, on any system: the data
  into `$DNT_HOME/data`, the configuration to `$DNT_HOME/config.toml`. It
  works for the app on Windows too (a portable install, or a second setup to
  try things in).
- The app on Windows keeps its own in `%APPDATA%\cz.v0v3s.dnt`; on Android,
  in the app's storage.

**Starting over**: *Data › Start over* in the app, `dnt reset` in the
terminal (both ask first). Removing imported data keeps the campaigns.
