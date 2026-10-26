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

**Versions**: character files, `campaign.toml` and the config start with
`format = 1`. A newer dnt reads what an older one wrote. A file synced from
a newer dnt than yours is left alone: the character list says it could not
be read and why; update dnt to open it.

**Starting over**: *Data › Start over* in the app, `dnt reset` in the
terminal (both ask first). Removing imported data keeps the campaigns.
