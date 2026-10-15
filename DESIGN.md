# dungeons-n-terminals (`dnt`) — Design

A keyboard-driven TUI for D&D 5e **players**: compendium, assisted character
management, and campaign-scoped notes. Data comes from user-supplied
[data](https://data) JSON (official + homebrew). No dice rolling (yet).

## Goals

- **Fast**: instant startup, fuzzy search across the whole library while typing.
- **Assisted, not enforced**: the app computes and suggests; the player can
  override anything. Every computed value can explain itself.
- **Extensible automation**: homebrew (e.g. Blood Hunter) is automated through
  overlay files with a small formula language, without touching source data.
- **Plain files**: characters and overlays are TOML, notes are Markdown — all
  git/sync friendly and editable outside the app.
- **No bundled game data**: the user imports their own data data.

## Non-goals (for now)

- Dice rolling, DM tools (encounters, monsters as a focus), online sync,
  multiplayer.

## Stack

| Concern            | Choice                                   |
|--------------------|------------------------------------------|
| Language           | Rust                                     |
| TUI                | `ratatui` + `crossterm`                  |
| Fuzzy search       | `nucleo`                                 |
| JSON               | `serde` / `serde_json`                   |
| Config/characters  | TOML (`toml`)                            |
| Library cache      | none for now: full load is ~150 ms (release); revisit if homebrew grows |
| Note editor        | `edtui` (vim modes); embedded nvim later |

## Workspace layout

```
crates/
  dnt-data/    data model, import, _copy resolution, tag parsing, entry rendering model, search index
  dnt-rules/   formula language (lexer/parser/evaluator), overlays, effect/stat dependency graph
  dnt-store/   on-disk layout: library, campaigns, characters, notes
  dnt/         the TUI binary
```

Dependency direction: `dnt` → `dnt-store` → `dnt-rules` → `dnt-data`.
UI-free crates stay testable without a terminal.

## On-disk layout

```
$XDG_DATA_HOME/dnt/            (~/.local/share/dnt)
  library/
    sources/      raw imported data JSON, untouched
    overlays/     automation overlays (built-in + user), *.toml
    cache/        parsed/resolved library + search index (rebuildable)
  campaigns/
    <slug>/
      campaign.toml           name, enabled sources, settings
      characters/<slug>.toml  choices + state + overrides
      notes/**/*.md           free-form tree
$XDG_CONFIG_HOME/dnt/config.toml   keybindings, theme, paths
```

## 1. Compendium

### Import
- Local directory (clone of the data data repo), single files, homebrew
  files (`_meta` block).
- Entity types: `class`, `subclass`, `classFeature`, `subclassFeature`, `spell`,
  `race`/`subrace`, `background`, `feat`, `optionalfeature`, `item`,
  `baseitem`, `condition`, `language`, `skill`, `action`.
- Resolve `_copy` (with `_mod`) at import time; record `reprintedAs`.
- Spell ↔ class lists come from the separate lookup files, not the spell entry.
- Result is cached; re-import only on source change.

### Identity
Entities are addressed with data-style IDs:
`<type>:<name>|<source>[|...]`, compared case-insensitively, e.g.
`spell:fireball|xphb`, `classFeature:Crimson Rite|Blood Hunter|BH|1`.

### Rendering
- `entries` trees (strings, `entries`, `list`, `table`, `inset`, `quote`, …) →
  an intermediate document model in `dnt-data` → ratatui widgets in `dnt`.
- Inline tags (`{@spell fireball|XPHB}`, `{@condition prone}`, `{@damage 2d6}`)
  parsed into spans; reference tags are **followable links** (Enter / `gd`),
  with a back stack.

### Sources and rulesets
- Every campaign has `enabled_sources`. All views (compendium, creation
  choices, link resolution) filter through it.
- A "ruleset" is just a named source preset (e.g. *2024 Core* = XPHB, XDMG, XMM).
- When both an original and its `reprintedAs` are enabled, prefer the reprint
  (toggle to show both).

### Search
- Fuzzy finder (`nucleo`) over names, narrowed by category tabs.
- Query terms: `@source`, `#tag`, `*damage`, `^level` / `^1-3`, `"text"`
  (full text), `!` to negate; `_` stands for a space. `#` means whatever
  is meaningful on the current tab (class/school for spells, type/rarity
  for items, category for feats, …). Tab completes facet values.
- Type-specific filters: spell level/school/class/ritual/concentration, item
  rarity/type, etc.
- Bookmarks.

## 2. Characters

### Character file = choices + state, never derived values
```toml
name = "Elara"
campaign = "curse-of-strahd"

[abilities]          # base scores (point buy / standard array / manual)
str = 8
dex = 15
# ...

[[levels]]           # level-up history, in order
class = "class:Blood Hunter|BH"
hp = "average"       # or a number
choices = { skills = ["skill:Athletics|XPHB", "skill:Survival|XPHB"] }

[state]
hp_current = 23
hp_temp = 0
resources = { blood_maledict = 1 }
toggles = { crimson_rite_active = true }
conditions = ["condition:Prone|XPHB"]

[overrides]          # manual overrides, kept separate from computed values
ac = 17
```
Because stats are always recomputed, fixing an overlay fixes every character.

### Creation wizard
Ruleset/sources (from campaign) → Species → Class → Background → Ability
scores (point buy, standard array, manual) → Skills/proficiencies → Equipment
→ Spells → Details. Choices encoded in data data (`skillProficiencies`
`choose`, `additionalSpells`, optional feature progressions, …) become wizard
steps.

### Level up
HP (average or manual), new features, subclass at the right level, ASI/feat,
spells/slots. Multiclassing (prerequisites, combined slot table) after MVP.

### Sheet
Tabs: Overview / Combat / Spells / Inventory / Features / Notes.
- Computed: modifiers, proficiency, saves, skills (incl. expertise), passives,
  initiative, AC, spell DC/attack, carrying capacity.
- Tracking: HP/temp HP, hit dice, death saves, spell & pact slots, prepared
  spells, class resources, toggles, conditions/exhaustion.
- **Short Rest / Long Rest** actions reset resources by their recharge rule.
- Inventory: compendium-linked items, quantity, weight, equipped, attunement
  (max 3), currency, containers.
- **Breakdown view** for any stat; **override** any stat.

### Multiclassing
No limit on the number of classes; nothing in the model assumes one class.

- **Source of truth** is the ordered `levels` list: one entry per character
  level, naming the class taken. Class levels, the starting class (first
  entry) and total level are derived from it. Subclasses are chosen per class.
- **Starting class only**: saving throw proficiencies, full starting
  proficiencies, max hit die at level 1. Later classes grant
  `multiclassing.proficienciesGained` instead.
- **Requirements** are checked and shown as warnings (assisted, not
  enforced): 2014 classes use `multiclassing.requirements`, 2024 classes
  require 13 in their `primaryAbility`. Both the current classes and the new
  one must qualify.
- **Per-class scoping**: features, subclass features and class-table
  variables use that class's level (`class.<slug>.<column>`,
  `level('Ranger')`); `level` is the total. Subclass tables (e.g. Eldritch
  Knight slots) extend the same namespace.
- **Spellcasting** is per class: `spell.<class>.mod/attack/dc`, prepared
  spells and cantrips from that class's columns, a spell list per class.
- **Spell slots**: with one slot-granting class, use its own table (needed
  for third-casters). With several, sum caster levels — full = level,
  `1/2` = half rounded down, `artificer` = half rounded up (2024 Paladin and
  Ranger use this), `1/3` = third rounded down — and read the multiclass
  slot table. **Pact Magic** stays separate (`pact.slots`, `pact.level`).
- **Hit points and hit dice**: per level from that level's class hit die;
  hit dice are pooled by die size (`hit_dice.d10`, …).
- **Non-stacking features** (Extra Attack, Unarmored Defense from two
  classes) are expressed in overlays with `max`/`set` rather than `add`.
- **Level cap**: totals above 20 produce a warning, not an error (epic /
  homebrew play).
- **UI**: the sheet header lists classes (`Fighter 5 / Wizard 3 / Rogue 1`);
  features and spells are grouped per class; level-up asks which class
  (existing or new) to advance, showing requirement warnings.

## 3. Rules engine (`dnt-rules`)

### Sources of automation
1. **Free, from structured data**: proficiencies, ASIs, spell slots,
   `additionalSpells`, and **class table columns** (`classTableGroups`) —
   every column becomes a level-scaled variable, e.g.
   `class.blood_hunter.hemocraft_die`. Covers much homebrew with zero work.
2. **Overlays**: TOML files attaching resources, toggles and effects to
   entity IDs. Shipped for common official content; user-written for
   homebrew; shareable.

### Overlay format
```toml
[[feature]]
id = "classFeature:Blood Maledict|Blood Hunter|BH|2"

[[feature.resource]]
key      = "blood_maledict"
name     = "Blood Maledict"
max      = "scale(level('Blood Hunter'), {2: 1, 6: 2, 13: 3, 17: 4})"
recharge = "short_rest"          # short_rest | long_rest | dawn | manual

[[feature]]
id = "classFeature:Crimson Rite|Blood Hunter|BH|1"

[[feature.toggle]]
key  = "crimson_rite_active"
name = "Crimson Rite"

[[feature.effect]]
when   = "toggle('crimson_rite_active')"
target = "attack.weapon.damage_dice"
add    = "class.blood_hunter.hemocraft_die"

[[feature.effect]]
target = "ac"
add    = "1"
when   = "not armor.heavy"
```

### Formula language
Small, pure, side-effect free expression language (custom; `rhai` only as a
future escape hatch). Implemented in `dnt-rules::formula`.
- **Types**: numbers are exact fractions (so `level / 2` stays ½ until the
  final 5e round-down), dice (`1d6`, `d8` — values, never rolled; `2 * d6`
  is `2d6`, `1d8 + 3` stays dice), booleans, strings.
- **Operators**: `+ - * /`, `== != < <= > >=`, `and or not` (also `&& || !`).
  `and`/`or` short-circuit; `if` evaluates only the chosen branch.
- **Variables** are dotted paths (see stat names below), plus
  `class.<class>.<column>` from class tables and `toggle('<key>')`.
- **Functions**: `mod(score)`, `min`, `max`, `floor`, `ceil`, `abs`,
  `if(c, a, b)`, `scale(n, {lvl: value, …})`, `dice(count, faces)`,
  `average(dice)`, `level()` / `level('Wizard')`.
- Errors carry byte spans; `Error::display` draws a caret under the culprit.

### Stat names (`dnt-rules::standard`)
| Stat | Formula |
|------|---------|
| `str` … `cha` | `base.<ab>` (+ effects) |
| `mod.<ab>` | `mod(<ab>)` |
| `prof` | `2 + floor((level - 1) / 4)` |
| `save.<ab>` | `mod.<ab> + prof * prof.save.<ab>` |
| `skill.<skill>` | `mod.<ab> + prof * prof.skill.<skill>` (0, ½, 1, 2) |
| `passive.perception/investigation/insight` | `10 + skill.<x>` |
| `initiative` | `mod.dex` |
| `ac` | armor category aware; + `armor.shield` |
| `hp.max` | `hp.base + mod.con * level` |
| `speed` | `speed.base` |
| `spell.mod/attack/dc` | per casting ability (`standard::spellcasting`) |
| `resource.<key>.max` | from overlays |

Effects may also target new keys (e.g. `attack.unarmed.damage`); such
implicit stats start at the input of the same name, or 0.

### Evaluation
- Each stat is a node; base formulas + effects define edges.
- Topological evaluation with **cycle detection** (reported per stat; other
  stats still evaluate). Errors in one effect don't break the stat.
- Order per stat: base and `max` candidates (highest wins) → last `set` →
  `add`s → `min` caps → round down → manual override.
- Every result keeps its parts, including inactive (`when` false) and
  losing ones, for the breakdown view:
  `AC 17 = 15 Unarmored Defense + 2 Shield overridden to 17`.

### Overlay diagnostics
TOML structure errors fail the file; invalid ids, keys and formulas are
reported per item (`feature[0].effect[1].add: …`) and only that item is
skipped. `Overlay::unresolved` lists ids not found in the library.

### Authoring
"Automate this feature" opens the editor with a template prefilled with the
feature ID and its text as comments; overlays are validated live.

## 4. Notes

Plain Markdown with YAML frontmatter and `[[wiki links]]`, so a campaign's
`notes/` folder also opens as an Obsidian vault.

### Default structure (created with every campaign)

```
campaigns/<slug>/notes/
├── Campaign.md     hub: premise, house rules, party, goals; generated
│                   sections for open quests, recent sessions, recent NPCs
├── Inbox.md        quick capture lands here
├── Sessions/       one note per session, named by date: 2026-10-15.md
│                   (second on a day: "2026-10-15 (2).md"); newest first
├── NPCs/
├── Quests/         status (active · done · failed) in frontmatter, not folders
├── Places/
├── Party/          <character>.md per character (the sheet's Notes tab),
│                   Party.md for shared fund, cart, marching order
├── Factions/
└── Lore/           history, gods, legends, rumours, prophecies
```

- **No Loot folder**: items live in the inventory; party-wide things in
  `Party/Party.md`.
- **Folder order is the user's**: the default above is only a starting
  point. Folders are reordered in the tree (`J`/`K`) and the order is saved
  per campaign (`[notes] order = [...]` in `campaign.toml`); folders not
  listed follow alphabetically. The default for new campaigns can be set
  in the global config.
- Sessions are not numbered; the title defaults to the date.

### Templates

Each folder has a template (frontmatter + headings), used when creating a
note there. Shipped defaults can be overridden globally
(`<data>/templates/<Folder>.md`) and per campaign
(`notes/.templates/<Folder>.md`); the most specific wins. Placeholders:
`{{title}}`, `{{date}}`, `{{campaign}}`, `{{character}}`.

```markdown
---
type: npc
tags: [npc]
status: alive          # alive · dead · unknown
location:
faction:
attitude: neutral      # friendly · neutral · hostile
first_seen: "[[{{date}}]]"
---
# {{title}}

**Who:**
**Wants:**
**Knows / told us:**
**Our dealings:**
```

Session template sections: Recap · Met · Went · Got · Learned · Open
threads · Next time.

### Features

- **Explorer tree** (Snacks-like): `a` add (in a folder: from its
  template; trailing `/` = folder), `r` rename, `d` delete, `J`/`K` move
  folder, `/` filter, Enter open; plus a fuzzy note picker.
- **Editor**: `edtui` with vim modes.
- **Links**: `[[Note Name]]` → note, `[[spell:Fireball]]` → compendium;
  `gd` follows, `K` previews. **Backlinks** ("mentioned in") per note.
- Session journal: "new session" creates today's session note.
- Quick capture hotkey: append a timestamped line to `Inbox.md` (or
  today's session note) from anywhere.
- Tags (`#npc`, frontmatter `tags`) and full-text search.
- Character sheet Notes tab shows `Party/<character>.md`.

## 5. Campaign layer: homebrew, clones and overrides

Imported data (data, homebrew files) is never edited. Each campaign has
its own homebrew file, `campaigns/<slug>/homebrew.json` (standard data
homebrew format), with a campaign source (e.g. `CoS`) that is always
enabled for that campaign.

- **Create** new entities (items first, then spells, feats, optional
  features, class/subclass features, species, backgrounds, later whole
  subclasses/classes).
- **Clone** any entity, stored as a diff via data `_copy` (only changed
  fields are written; upstream fixes still flow in; data/Foundry can
  load the file). Two modes:
  - **Replace the original** (`"_dnt": {"replaces": true}`): within the
    campaign the clone is used everywhere instead of the original —
    compendium, search, note links, pickers and existing characters.
    Clones can be as small as one class feature.
  - **Separate copy**: a new entity under its own name.
- Compendium marks campaign versions ("Campaign version · changed:
  duration") with a key to view the original.
- **Editor**: per-kind form for common fields, description written as
  Markdown in the built-in editor (converted to data entries), raw JSON
  in `$EDITOR` as an escape hatch (validated on save).
- Entry points: Compendium `c` clone / `n` new (edit/delete only for
  campaign entities), Inventory "Create new item…".
- Entities can be promoted to a global homebrew library
  (`<data>/library/homebrew/`) to use them in every campaign.

## 6. UX

- Vim-ish navigation everywhere, `:` command line, global fuzzy picker.
- Split panes: sheet + compendium/notes side by side.
- Themes; graceful layout on small terminals.

## Milestones

1. **M1 Compendium** — import, `_copy`, fuzzy search, entry rendering with
   tag links, faceted queries, campaigns with per-campaign sources. *Done.*
2. **M2 Rules engine** — formula language, stat graph, breakdowns, overrides,
   class-table variables, overlays, standard 5e stats. *Done* (no UI; the
   sheet in M3 is its first consumer).
3. **M3 Characters** — character files, multiclass-aware builder, built-in
   2024 overlays, roster and sheet (Overview, Features), tracking, rests.
   Level-up flow (class incl. multiclass with requirement warnings,
   subclass when due, HP average/rolled/max, ASI / feat / Epic Boon) and
   undo of the last level; per-level choices live on the level entry.
   Creation wizard: species, subrace, background, ability scores
   (standard array / point buy / manual), species/background increases
   (fixed, choose, 2024 weighted), origin feats, class, level-1 subclass,
   class and origin skills, armor and shield, review with live preview;
   Esc steps back.
   Class feature choices (dnt-rules::choices) in the wizard and level-up:
   2024 Fighting Style feats (`featProgression`), invocations/maneuvers/
   metamagic (`optionalfeatureProgression`), option blocks inside
   features (Divine Order, Hunter's Prey, Pact Boon), Expertise and
   Weapon Mastery; stored per level in `picks`. Missing choices on
   existing characters are flagged and filled with `c`.
   `_versions` are expanded into entities (2024 lineages/legacies/
   ancestries, Dragonborn colors, feat variants like Magic Initiate;
   Cleric) and offered in the wizard.
   Starting equipment from class and background packages (2024 A/B/C and
   2014 per-group a/b), "any martial weapon"-style picks, auto-equip.
   Inventory tab: items (library or custom), quantity, equip (one armor,
   one shield), attunement (limit 3 warned), weight vs Str × 15, coins.
   Equipped armor/shield and worn magic items (bonusAc,
   bonusSavingThrow; attunement required where stated) feed the stats.
   Spells tab (dnt-rules::spells): per-class limits (cantrips; 2024
   prepared tables, 2014 prepared formulas, known tables; Wizard
   spellbook), max spell level from the class's own slots or pact level,
   class lists by class name, `additionalSpells` from subclasses,
   species, backgrounds and feats (always prepared, known, innate with
   free casts as long-rest resources, and "choose" grants), casting from
   a chosen slot or pact slot or as a ritual, concentration tracking.
   *M3 done. Next: M4 Notes.*
4. **M4 Notes** — notes store, default structure and folder order,
   templates, explorer tree, edtui editor, links and backlinks, session
   journal, quick capture, sheet Notes tab. *Done* (tags/full-text
   search and a richer hub to follow).
5. **M5 Campaign layer** — campaign homebrew file and source, clone
   (replace / separate), creator and editor forms (items, spells first),
   compendium markers, inventory shortcut.
6. **Later** — overlay authoring flow, expanded spell lists, embedded nvim,
   export.
