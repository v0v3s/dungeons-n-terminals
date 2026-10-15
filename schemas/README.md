# The dnt data format

dnt ships no game content. It reads rules data from a directory you
provide, and homebrew from JSON files, in the format described here. The
JSON Schemas in this folder (draft 2020-12) describe the fields dnt uses;
data may carry more fields, which dnt keeps but ignores.

| Schema | Describes |
| --- | --- |
| [`data-file.schema.json`](data-file.schema.json) | a file of entities: the content files of a data directory, a campaign's `homebrew.json`, global homebrew |
| [`books.schema.json`](books.schema.json) | `books.json` and `adventures.json`: the sources |
| [`index.schema.json`](index.schema.json) | `class/index.json` and `spells/index.json` |
| [`spell-classes.schema.json`](spell-classes.schema.json) | `spells/sources.json`: which classes have each spell |
| [`overlay.schema.json`](overlay.schema.json) | rules files (TOML) in `library/overlays/` |

To check a data directory or homebrew files against them:

```sh
cargo run --release -p dnt-data --example validate -- /path/to/data [homebrew.json …]
```

Editors that understand JSON Schema can use them too: add
`"$schema": "…/schemas/data-file.schema.json"` to a homebrew file, or point
your TOML editor (e.g. Taplo) at `overlay.schema.json` for rules files.

## The data directory

`--data` points at the directory, or at a folder that holds it as `data/`.
dnt reads, all at every start, without changing anything:

```
books.json              the sources (required: it marks the directory)
adventures.json         more sources (optional)
races.json  backgrounds.json  feats.json  optionalfeatures.json
items.json  items-base.json  magicvariants.json
conditionsdiseases.json  languages.json  skills.json  senses.json
actions.json  variantrules.json
class/index.json        → the class files to read, e.g. {"XPHB": "class-xphb.json"}
class/*.json
spells/index.json       → the spell files to read
spells/*.json
spells/sources.json     which classes have each spell on their list
```

`books.json` and the two `index.json` files must exist; content files
that are missing or not valid JSON are reported as warnings and skipped.

## Data files

A data file is an object whose keys are kinds of entity, each a list:

| Key | Entity |
| --- | --- |
| `class`, `subclass`, `classFeature`, `subclassFeature` | classes and their features |
| `spell` | spells |
| `race`, `subrace` | species and their lineages |
| `background`, `feat`, `optionalfeature` | backgrounds, feats, invocations, maneuvers, fighting styles, … |
| `item`, `baseitem`, `itemGroup`, `magicvariant`, `itemMastery` | equipment and magic items |
| `condition`, `disease`, `status`, `language`, `skill`, `sense`, `action`, `variantrule` | rules text |

Other keys are ignored. Homebrew files start with a `_meta` block naming
the sources their entities use:

```json
{
  "_meta": {"sources": [{"json": "CoS-HB", "full": "Curse of Strahd (campaign)", "version": "1"}]},
  "item": [
    {"name": "Sword of Tallow", "source": "CoS-HB", "type": "M|XPHB", "rarity": "uncommon",
     "reqAttune": true, "weight": 3, "dmg1": "1d8", "dmgType": "S", "bonusWeapon": "+1",
     "entries": ["This blade glows faintly when undead are within 120 feet."]}
  ]
}
```

### Every entity

- `name` and `source` (a source id from `books.json` or `_meta`) identify
  it; `page` locates it.
- `entries` is its text (below).
- `edition: "one"` marks the 2024 rules (`classic`: 2014). It decides how
  a character built on a class plays (rests, exhaustion) and which version
  of a feature applies (Martial Arts, Dual Wielder, Great Weapon
  Fighting). Entities without it follow their class (class features), else
  count as 2024 when their source is `XPHB`.
- `reprintedAs` lists newer versions; with "hide reprints" on, only the
  newest shows.

### References

Entities refer to each other with pipe-separated references, compared
without regard to case: `name|source` for most kinds (`Fireball|XPHB`),
`name|class|classSource|level` for class features
(`Rage|Barbarian|XPHB|1`), `name|class|classSource|subclass|subclassSource|level`
for subclass features. dnt uses the same references with a type prefix in
character files and rules: `spell:Fireball|XPHB`,
`classFeature:Rage|Barbarian|XPHB|1`.

### Inheritance: `_copy`

An entity with `_copy` is built from another one: the copied entity's
fields, then its own (a `null` field removes the inherited one), then the
changes in `_copy._mod`. Fields such as `page` and `srd` are not
inherited unless named in `_preserve` (`"*"` keeps them all).

```json
{"name": "Barovian Longsword", "source": "HB",
 "_copy": {"name": "Longsword", "source": "XPHB",
           "_mod": {"*": {"mode": "replaceTxt", "replace": "longsword", "with": "Barovian longsword", "flags": "i"},
                    "entries": {"mode": "appendArr", "items": "Its pommel bears a raven."}}},
 "value": null}
```

`_mod` modes: `replaceTxt` (regular expressions, `*` for all text),
`setProp` (`_` for the entity itself), `appendArr`, `prependArr`,
`insertArr`, `appendIfNotExistsArr`, `replaceArr`, `replaceOrAppendArr`,
`removeArr`, `renameArr`, or `"remove"`.

A **subrace** is layered over its race the same way: its fields replace
the race's, and `null` removes one.

**`_versions`** generates named variants of an entity (lineages, feat
versions): each version is a `_copy` of it, or `_abstract` and
`_implementations` fill a template's `{{variables}}`.

### Magic variants

A `magicvariant` is a generic item ("+1 Weapon"). dnt turns it into a
specific item for every base item it applies to: `requires` lists
alternatives of base item fields that must match (`{"weapon": true}`,
`{"type": "HA|XPHB"}`), `excludes` fields that rule a base out, and
`inherits` the fields laid over the base, with `namePrefix`/`nameSuffix`
and `{=bonusWeapon}`-style placeholders filled from the item.

### Text: `entries` and inline tags

`entries` is a list of paragraphs (strings) and blocks
(`{"type": "...", ...}`): `entries` and `section` (with `name` and nested
`entries`), `list` (with `items`; `item` entries have a `name` and
`entry`), `table` (`colLabels`, `rows`), `tableGroup`, `inset`, `quote`,
`variant`, `hr`, `abilityDc`, `abilityAttackMod`, and references to
features (`refClassFeature`, `refSubclassFeature`, `refOptionalfeature`).
Other block types show their entries.

Inside text, inline tags format and link: `{@tag text}`, with arguments
separated by `|`.

| Tags | Shown as |
| --- | --- |
| `b`, `i`, `u`, `s`, `code`, `note` | bold, italic, underline, struck, code, a note |
| `spell`, `item`, `feat`, `class`, `subclass`, `classFeature`, `subclassFeature`, `race`, `background`, `optfeature`, `condition`, `disease`, `status`, `skill`, `sense`, `action`, `language`, `variantrule`, `itemMastery` | a link: `{@spell Fireball\|XPHB\|fireball}` (name, source, display text) |
| `dice`, `damage`, `d20`, `hit`, `chance`, `recharge`, `scaledice`, `scaledamage` | dice and numbers; dnt reads `damage`, `dice`, `scaledamage` and `scaledice` to roll spells |
| `dc`, `atk`, `atkr`, `h`, `m` | "DC 15", "Melee Weapon Attack:", "Hit:" … |

Unknown tags show their text.

### dnt's own fields

- `"_dnt": {"replaces": true}` on a campaign entity with `_copy`: within
  the campaign it replaces the entity it copies (a house-ruled version).
- Character files, notes and rules files are dnt's own formats: characters
  are TOML (see `crates/dnt-rules/src/character.rs`), notes Markdown, and
  rules TOML ([`overlay.schema.json`](overlay.schema.json)).
