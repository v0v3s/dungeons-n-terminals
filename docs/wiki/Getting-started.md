## The data

dnt reads the rules (spells, classes, items, …) from data you provide, in
the data format described in
[`schemas/`](https://github.com/v0v3s/dungeons-n-terminals/blob/main/schemas/README.md).
Make one small zip of just what dnt reads, from a data repository and/or a
homebrew collection:

```sh
scripts/pack-data.sh --data path/to/data-repo --homebrew path/to/homebrew -o dnt-data.zip
```

(It needs [Rust](https://rustup.rs) and a checkout of dnt.) The same zip
works for both apps.

## The first start

| The app | The terminal app |
| --- | --- |
| ![Getting started in the app](images/app-welcome.png) | ![Getting started in the terminal](images/tui-welcome.svg) |

- **The app** opens on *Getting started*: *Import a zip…*, then name your
  first campaign. On a phone, put the zip on the phone first.
- **The terminal app** asks for the zip's path (or a data folder, read in
  place). Or from the command line: `dnt import dnt-data.zip`.

Importing again (a newer zip, more homebrew) updates entries instead of
doubling them.

## Campaigns

A campaign holds your characters, notes and homebrew, and chooses the books
it uses. A new one uses all the books; change them with the compendium's
`@` button or the campaign's *Books & homebrew* (in the terminal: **4
Campaigns**, `tab` to *Sources*). Imported homebrew is off until a campaign
turns it on.

![Campaigns](images/app-campaigns.png)

Next: [[Characters]].
