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
- Global fuzzy finder (`nucleo`) over names, with type/source facets.
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
future escape hatch).
- **Types**: integer, dice (`1d6`, a value — never rolled), string, boolean.
- **Operators**: `+ - * /` (integer, floor), comparison, `and or not`.
- **Variables**: `str`…`cha`, `mod(dex)`, `prof`, `level`, `level('Wizard')`,
  `class.<slug>.<column>`, `ac`, `hp.max`, `resource.<key>.max`,
  `toggle('<key>')`, `armor.heavy`, `armor.none`, `shield`, …
- **Functions**: `min`, `max`, `floor`, `ceil`, `if(c, a, b)`,
  `scale(n, {lvl: value, ...})`.
- Errors carry spans for inline display in the editor.

### Evaluation
- Each stat is a node; base formulas + effects define edges.
- Topological evaluation with **cycle detection** (reported, not crashing).
- Every result keeps its contributions → the breakdown view:
  `AC 17 = 10 base + 3 DEX + 2 Shield + 1 Defense + override`.
- Overrides apply last and are shown as such.

### Authoring
"Automate this feature" opens the editor with a template prefilled with the
feature ID and its text as comments; overlays are validated live.

## 4. Notes

- Campaign-scoped Markdown files under `campaigns/<slug>/notes/`.
- **Explorer tree** (Snacks-like): `a` add (trailing `/` = folder), `r`
  rename, `d` delete, `/` filter, Enter open; plus a fuzzy note picker.
- **Editor**: `edtui` with vim modes.
- **Links**: `[[Note Name]]` → note, `[[spell:Fireball]]` → compendium;
  `gd` follows, hover/`K` previews.
- Session journal: "new session note" creates `sessions/YYYY-MM-DD.md`.
- Quick capture hotkey: append a timestamped line to today's session note
  from anywhere.
- Tags (`#npc`) and full-text search.
- Pinned notes shown on the character sheet.

## 5. UX

- Vim-ish navigation everywhere, `:` command line, global fuzzy picker.
- Split panes: sheet + compendium/notes side by side.
- Themes; graceful layout on small terminals.

## Milestones

1. **M1 Compendium** — import, `_copy`, fuzzy search, entry rendering with
   tag links, source filtering. *Done, except campaign-scoped sources (comes
   with `dnt-store`; for now `--sources` on the command line).*
2. **M2 Rules engine** — formula language, stat graph, breakdowns, overrides,
   class-table variables.
3. **M3 Characters** — level-1 single-class creation, sheet, tracking, rests.
4. **M4 Notes** — explorer tree, edtui editor, links, session journal.
5. **M5** — level-up, multiclassing, inventory depth, overlay authoring flow,
   homebrew polish; later embedded nvim, export.
