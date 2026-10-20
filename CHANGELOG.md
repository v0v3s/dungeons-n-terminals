# Changelog

What changed in each release of dnt (the terminal app) and the dnt app
(Windows and Android). Pull requests add their changes under
**Unreleased**; a release names that section (see
[`scripts/release-notes.sh`](scripts/release-notes.sh)), and its notes
become the release page.

## Unreleased

### Added

- Terminal: **Getting started** when there is no data yet: give a zip to
  import (or a data folder) instead of an error.
- Terminal: the campaign's **Homebrew** (`tab` in Campaigns) turns imported
  homebrew on and off, searchable by name or author.
- Terminal: `D` deletes a campaign, after a confirmation.
- Terminal: `dnt forget --homebrew`, `dnt forget --all` and `dnt reset`
  start over.

### Changed

- Terminal: new campaigns use all official sources, and without a
  campaign only the newest printing of reprinted entries shows, as in the
  app.

### Fixed

- Terminal: homebrew imported before v0.7.0 is moved out of the data
  there too.

## v0.9.0 - 2026-10-20

### Added

- App: **Getting started**: a new app (or one just reset) walks through
  importing the data and creating a campaign, explaining each step, then
  offers to create a character or browse the compendium.
- App: **Start over** on the Data screen: remove the imported homebrew,
  remove all imported data (campaigns stay), or reset the app entirely,
  each after a confirmation saying what goes and what stays.

### Changed

- App: dropdowns are drawn in the app's style.

### Fixed

- The Windows app builds again (v0.8.2 shipped without its installer).

## v0.8.2 - 2026-10-20

### Added

- Each release page lists its changes, from this changelog.
- App: a loading screen says what is being loaded, and the Data screen
  shows how long each part of the last load took.
- A new logo: the app icon, the Android splash and the badge in the app.

### Changed

- App: the data is read in parallel, and the compendium is indexed while
  the app starts.
- App, phones: the compendium's sources button is in the top bar, so the
  categories have the whole width.
- App, notes: tapping a folder makes it where new notes go; *+ Note*
  creates the note there from the folder's template (in Sessions, today's
  session note). *Capture* is now *Quick note*, which adds a line to the
  Inbox note. New folders are under *⋯*.
- App, sources: presets are a dropdown that ticks sources (which you can
  then change); "all official" shows every source ticked; the list of
  ticked sources is gone and the reprint option explains itself behind a
  *?*. Sheets close when dragged down by their top.

### Fixed

- Homebrew imported before v0.7.0 sat with the official data and was loaded
  on every start, which made the app slow to open. It is moved to the
  homebrew store on the first start (where a campaign had it ticked, it
  stays enabled).
- Official sources that are not in the book list (some adventures and
  Unearthed Arcana) are no longer shown as homebrew.

## v0.8.1 - 2026-10-19

### Fixed

- App: creating, switching and deleting campaigns, saving sources and
  importing show what they are doing and ignore taps until done, so a
  second tap no longer creates a second campaign.
- App: campaign changes are faster; the status no longer re-reads every
  imported source.
- App: on phones, a campaign's buttons wrap instead of running off the
  screen.
- Reprinted magic variants (such as +1 Armor from the 2014 DMG) are hidden
  with "only the newest printing", like other reprints.

### Added

- App: campaigns can be deleted, after a confirmation.

## v0.8.0 - 2026-10-19

### Added

- `scripts/pack-data.sh`: packs just what dnt reads, from a data
  repository and/or a homebrew collection, into one small zip to import
  (26 MB instead of several hundred).
- Terminal: the imported homebrew a campaign enables is loaded;
  `dnt imported` lists homebrew sources and `dnt forget` removes them.

## v0.7.0 - 2026-10-19

### Added

- Imported homebrew is its own category: files that name their sources are
  stored apart and stay off until a campaign enables them, so a whole
  homebrew collection can be imported without slowing anything down.
- App: a Homebrew tab in the sources editor, searchable by name or author,
  with a filter for 2024 or 2014 rules.

### Changed

- New campaigns use all official sources, newest printings only, and no
  homebrew.

## v0.6.0 - 2026-10-19

### Added

- App: the compendium shows the campaign's sources and opens the sources
  editor from there; results update when the sources change.
- App: whole sections of sources can be ticked at once.

### Changed

- Without a campaign, entries reprinted in a newer book (PHB spells and
  XPHB, say) show only in their newest printing.

## v0.5.0 - 2026-10-19

### Added

- The dnt app for Windows (installer) and Android (phones and tablets),
  in the look of the terminal app: data import, the compendium, campaigns,
  character creation and sheets, level-ups and notes.

## v0.4.0 - 2026-10-19

### Added

- `dnt import FILE.zip`: import data from a zip, a whole data folder or
  homebrew files in any layout. Importing again updates entries instead of
  doubling them. `dnt imported` and `dnt forget` list and remove sources.

## v0.3.1 - 2026-10-16

### Fixed

- A removed spell is no longer kept as a favorite.

## v0.3.0 - 2026-10-15

### Added

- JSON schemas describing the data dnt reads (`schemas/`).

## v0.2.0 - 2026-10-15

The first release of dnt, the terminal app:

- A compendium with faceted, fuzzy search.
- Campaigns with their own sources, homebrew and notes.
- Character creation, level-ups, and a character sheet with tracking,
  rests, attacks, spells and inventory, driven by a rules engine.
- Exporting a character sheet.
