# Changelog

What changed in each release of dnt (the terminal app) and the dnt app
(Windows and Android). Pull requests add their changes under
**Unreleased**; a release names that section (see
[`scripts/release-notes.sh`](scripts/release-notes.sh)), and its notes
become the release page.

## Unreleased

## v1.3.0 - 2026-10-30

### Added

- The rules of the SRD 5.2.1 are built in: classes and subclasses,
  species, backgrounds, feats, spells, weapons, armor, gear, magic items
  and the rules glossary, from the first start, with nothing to import.
  Imported books and data folders load over them; where a book has an
  entry of the SRD, the book's shows while the book is on. The SRD's
  attribution is in *About*.
- The built-in rules of the 2024 entries work for the SRD's too (Alert,
  Rage, Monk's Focus, Arcane Recovery…).
- A class can grant spells: the Ranger's Hunter's Mark, the Paladin's
  Divine Smite, the Druid's Speak with Animals.
- Skilled (and any origin feat with a skill choice) asks for its skills
  at creation.
- The rules library (github.com/v0v3s/dnt-rules): rules for features of
  the SRD and of homebrew, browsed as a tree of folders, files and rules,
  and imported as your own rules, a whole folder or single rules at once.
  Rules that differ from yours ask to overwrite or keep, each or all.
  *Data › Rules library*; `L` on Campaigns; `dnt rules`.

### Changed

- The app's *Data* is in tabs: Import, Rules, Backups, Problems (with how
  many, only while there are some) and About; the menu lists them under
  Data.
- The first start no longer asks for data: it asks for a campaign. The
  terminal app's getting-started screen is gone (`dnt import` and
  `--data` add your books).

### Fixed

- Your rules for a feature of imported homebrew are no longer named as
  "not in the library" when the open campaign does not turn that
  homebrew on.
- In the app, after *Reset the app*, backups no longer go on to the
  folder chosen before the reset (until the app restarted).
- In the app, holding one of two lines of the same weapon (a Quarterstaff
  and an Arcane Focus (Quarterstaff)) holds that line, not the other.

## v1.2.0 - 2026-10-29

### Added

- Backups: everything (campaigns, characters, notes, the imported data,
  your homebrew and rules) in one zip that restores on another device. By
  hand, or by itself at start every day, week or month, keeping the last
  few, into a folder of your choosing (`dnt backup --folder`; on Android
  `Download/dnt` unless you pick another, such as an SD card or a cloud
  drive). Restoring replaces
  everything, after backing up what was there. *Data › Backups*; `dnt
  backup`, `dnt backups`, `dnt restore`.
- Export a character with only the homebrew and rules they use, a whole
  campaign, or a single entry (a spell, a monster, a class with its
  features), and import them on another device or into another campaign.
  A character or campaign that is there already asks: replace it, or keep
  both. *⋯ › Export for dnt*, a campaign's *⋯ › Export*, an entry's
  *⋯ › Export*, *Data › Import…*; `dnt export`, `dnt import`.

### Changed

- The app: a campaign card's *Books & homebrew*, *Rename* and *Delete*,
  and a compendium entry's *Edit* and *Clone into the campaign*, are in
  its *⋯* menu, leaving more room.

## v1.1.0 - 2026-10-28

### Changed

- App, on a phone: the app bar carries the open character (their name and
  level, the warnings and the sheet's ⋯ menu); the sheet's own header is
  gone, and the tabs are a row of icons.
- App: the sheet's tabs are Overview, Skills, Spells, Inventory, Features
  and Notes: abilities (score, check, save) and skills have their own tab.
- App: every number that rolls is a box, the same everywhere on the
  sheet.
- App: a roll shows big, its numbers first; an attack's to-hit and
  damage side by side. A tap puts it away.
- App: the hit dice are a bar under the hit points (tap it to roll one
  and heal); spell slots are only on the Spells tab, not under Tracking.
- App: a class's spells are sorted by level, each level under its own
  heading with its slots beside it; the class's save DC, attack bonus
  and highest level sit in boxes at the top. Preparing a spell is a box
  to tick.
- App: a spell with dice has a roll button on its row.
- App: an item you can wear or hold has a box to equip it on its row;
  the carried weight and the attuned items are boxes under the list.
- Creating a character: each choice in a list says what it gives (a
  species its increases and speed, a background its skills and feat, a
  class its hit die, main abilities and saves, a feat what it needs).
- Creating a character: the ability scores are one screen, the method
  picked on it (`m` in the terminal) instead of a step before.
- Creating a character: the steps list what was chosen in each. In the
  app, tap a finished step to choose it again (on a phone, the steps
  fold above the question, and the review is the whole list).
- App, on a phone: a ? on a species, background, class or equipment row
  shows its full text, with a button to choose it.
- App, on a phone: a one-time tip names each gesture the screen does not
  show (swiping the tabs or the categories, holding a number). It goes
  once dismissed or once the gesture is used; the sheet's ⋯ menu shows the
  tips again.
- App, on a phone: the compendium's search and category share one row,
  and the sort sits in the results' heading.

## v1.0.1 - 2026-10-26

### Added

- App: swipe the compendium's results sideways on a phone to change the
  category, as the sheet's tabs do.
- App: the menu's sections unfold (▸) to what is inside them: the
  compendium's categories and the open character's tabs (Characters ›
  Spells from anywhere).

### Changed

- App: while swiping, the gap the sheet (or the results) leaves names the
  tab or category you are going to, lit once letting go opens it, instead
  of showing a piece of it.
- App: the sheet's tabs and the compendium's categories are a select,
  every one in its list (on a phone, Notes and the last categories no
  longer hide off the side of a bar).

## v1.0.0 - 2026-10-26

dnt 1.0: from now on the files you keep (characters, campaigns, notes,
homebrew, overlays) and the command line stay compatible; a change that
breaks them comes in a new major version. See *Compatibility* in the
README.

### Added

- Character files, `campaign.toml` and the config start with
  `format = 1`. A file from a newer dnt is left alone, not read in part
  and saved over.
- Character files that cannot be read are no longer silently missing
  from the list: the terminal's status line and the app's character list
  say which and why.

### Fixed

- Terminal: a mistake in your rules (overlays) is named when it starts,
  with its file and what is wrong; the warning never showed before.
- App: problems found while loading (the data, homebrew, your rules) are
  listed under *Problems* on the Data screen; they were never shown.
- Terminal: the Attacks pane's "In hand" no longer runs into the keys'
  hint on narrow panes.

## v0.13.0 - 2026-10-26

### Added

- Terminal: trackers of your own on the Overview: `a` adds one
  (`Ring of Warmth, 3, dawn`), `x` removes it (asks first).
- Terminal: `h` on a dead character marks them not dead.
- Terminal: `r` renames a campaign.
- Terminal: `o` orders the compendium's results (best match, name A–Z
  or Z–A, source, level or rarity), as the app does.
- Terminal: an About section at the end of the help screen (`?`).
- App: hold a number (an ability check, save, skill or initiative), or
  right-click it, to see how it is made up; tap an ability's name for its
  score.
- App: inventory containers fold (▾/▸); a container's menu adds items
  straight into it; any line's menu sets its container settings
  (capacity, no limit, weightless) and, for weapons, whether it counts as
  a monk weapon.
- App: *+ Homebrew* in the inventory creates a campaign item from a
  template, adds it to the character and opens it for editing.
- App: note folders can be renamed, deleted (asks first) and the
  top-level ones moved up or down, from the Notes menu with a folder
  chosen.
- App: the About on the Data screen names the version, the licence and
  the repository, as the terminal's help does.
- App: on a phone, the sheet follows a sideways swipe with the next tab
  sliding in beside it; it opens past 60% of the way (or with a flick) and
  slides back otherwise. The tab bar keeps the open tab in sight.
- `DNT_HOME` keeps everything (data, campaigns, configuration) in one
  folder of your choice, on any system, for the terminal app and the app on
  Windows: a portable install, or a second setup to try things in.

### Fixed

- App: equipment packs (Explorer's Pack, Scholar's Pack, …) can be
  unpacked: *Unpack* was only offered on containers, which packs are not.
- A multiclass caster's spell rolls use the attack and DC of the class
  the spell belongs to (rolling it in the app, and the roll after
  casting it in both), not the best class's.
- App: deleting a note asks first.
- App: removing a tracker of your own asks first, as the terminal does.
- An innate spell with no free casts left no longer offers a free cast.
- App: a dialog opened right after the last one closed is a new one: a
  list no longer keeps the last one's filter ("Nothing matches" when adding
  spells one after another), nor the rules editor the last one's text.
- Terminal, on Windows: editing a character's file or a homebrew entry in
  `$EDITOR` (Notepad by default) opens the file; the editor was given a
  mangled path.
- App: "spells to choose" and "choices to make" no longer run their words
  together on wide screens.
- The count of spells to choose includes a feat's or a species' choice of
  spells, which the sheet's flag also leads to.
- App: on a phone, the inventory's buttons and the notes' Quick note no
  longer wrap their labels; the inventory leaves room for fold arrows only
  when something can fold, and the arrows are bigger.
- Restoring a feature use, a spell slot or a hit die that was not spent
  says it is full, instead of "Restored" with nothing changed.

## v0.12.0 - 2026-10-24

### Added

- Hit dice can be rolled: spending one rolls the die plus Constitution
  and heals that much (at least 1 under the 2024 rules). In the app, each
  die size has its own *Roll* button; in the terminal, `r` on a hit die.
- App: tapping a tracker offers what can be done with it: remove your own
  trackers; see a feature's, and change or remove the rules that keep it.
- App: the sheet's panes fold: tap a title (▾) to fold it to one line
  with a summary (hit points, what is in hand, how many items), and tap
  it (▸) again to open it. Folded panes stay folded.
- Characters can be deleted (their notes stay): in the app from the
  sheet's ⋯ menu, in the terminal with `D` on the roster; both ask first.

### Changed

- Character creation: a background's ability score increases can be
  skipped.
- App: a new character in progress stays while other sections are open (to
  look something up in the compendium); it ends when it is created or
  cancelled.
- App: *+ New character* and *+ New campaign* stand on their own above
  the list; each character and each campaign is a card of its own (a
  campaign's buttons in its card, the active one marked).
- App: in the top bar, the buttons (menu, back) are coloured and the `❯`
  before the place is grey, so what can be tapped stands out.

## v0.11.0 - 2026-10-23

### Added

- App: **homebrew editing**, as in the terminal. A book entry can be cloned
  into the campaign (a version replacing it, or a separate copy); a
  campaign version shows its book original. *+ new* (on phones *+* in the
  top bar) makes an entry from a template (magic item, weapon, armor,
  gear, spell, feat, optional feature, background, species). The editor
  lists the fields, changed ones marked; each edits in its own way (a
  choice, a line with a hint, Markdown or JSON), can go back to the book,
  and the whole entry can be edited as JSON, given a new field, made
  global (every campaign) or deleted.
- App: **export** a character sheet as a printable page (or save it as PDF
  from a browser) or as Markdown, from the sheet's ⋯.
- App: **edit the character file** (abilities, species, background, …)
  from the sheet's ⋯; it is checked before saving.
- App: **a feature's rules**: *rules* on a feature writes or changes them,
  checked as you type, listing what they change on the character.
- App: **rolls into the session note**, switched on from the sheet's ⋯.
- App: notes show what their ```dnt query blocks find (the campaign note
  lists open quests, recent sessions and people); comments and the
  blocks themselves are hidden when reading.
- **Death saves**: at 0 hit points the sheet rolls them (a natural 20
  brings 1 hit point back, a 1 fails twice; three successes: stable, three
  failures: dead). Damage at 0 is a failed save, and damage that leaves as
  much over 0 as the hit point maximum asks whether the character dies
  outright. Terminal: `D` rolls a death save.
- App: the compendium's results can be sorted (best match, name, source,
  level or rarity).
- App: the Spells tab shows the spell slots, with − and +.
- **Custom trackers**: *+ Tracker* in the sheet's Tracking keeps count of
  anything with uses (a ring's stored spell, a boon), refilled after a long
  rest, a short rest, at dawn or never.
- App: swiping sideways on the sheet changes the tab (not from the screen's
  edges, which are the system's).
- App, homebrew: backgrounds, species and feats have their own fields:
  ability score increases (fixed, chosen, or +2/+1 among some), skill, tool,
  weapon, armor and language proficiencies (ticked, or "any 2", "choose 2:
  …"), feats by name, size, speed and darkvision. New backgrounds start in
  the 2024 shape.
- App: icons from game-icons.net (by Lorc and Delapouite, CC BY 3.0) for
  the sections, the sheet's tabs, hit points, rolls, death and warnings;
  the Data screen credits them under About.

### Changed

- App, character sheet: less crowded. The name is in the top bar only (on
  phones), the numbers fit one row, *Level up* is in ⋯ only, and warnings
  (like unmet multiclass requirements) are behind a ⚠.
- App: *N spells to choose* opens the choosing, not just the Spells tab.
- App: the sheet's latest rolls are folded away, the latest one in view.
- App: the top bar keeps the menu next to Back.
- App: a note opened from a character goes back to the character.
- App, item editing: each kind of item has its own fields (a weapon's
  damage and properties, armor's AC and Strength), with names people use
  and what each is for. Damage must be dice, bonuses are +1 to +3, and
  properties are ticked from a list (custom ones can be added).

### Fixed

- App: naming a new homebrew entry like an existing one says so in the
  naming window, not behind it.
- Species written for customizing your origin (Monsters of the Multiverse
  and others, marked `lineage`) give their ability score increases at
  creation: +2/+1 or +1/+1/+1, or none when a 2024 background gives them.

## v0.10.0 - 2026-10-20

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
