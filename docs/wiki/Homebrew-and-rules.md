## Homebrew

- **Imported homebrew** (files that name their sources) is off until a
  campaign turns it on (its *⋯ › Books & homebrew › Homebrew*).
- **Campaign homebrew**: clone any entry (a campaign version replaces the
  book's; a copy is a new entry), or make a new spell, item, feat, … from a
  template. Edit it field by field, or as JSON. *Make global* shares it with
  every campaign.

## Rules

A sheet knows a feature's numbers when there are rules for it. dnt ships
rules for many 2024 features; features with rules show `auto` in the
Features tab. For anything else, write them: the feature's *rules* button
(`O` in the terminal). The check shows what they change as you type.

```toml
[[feature]]
id = "classFeature:Brutal Strike|Barbarian|XPHB|9"

[[feature.resource]]
key = "brutal_strike"
name = "Brutal Strike"
max = "prof"
recharge = "long_rest"

[[feature.effect]]
target = "rider.brutal_strike"        # extra damage on a hit
set = "if(class.barbarian.level >= 17, 2d10, 1d10)"
```

Targets include `ac`, `speed`, `hp.max`, `initiative`, `save.dex`,
`skill.stealth`, `attack.bonus`, `rider.<key>` and `resource.<key>.max`;
effects `add`, `set`, `max`, `min` or `mul`, with an optional `when`.
A toggle (`[[feature.toggle]]`) turns effects on and off from the sheet.

Rules grant spells too: a spell, or a choice of spells (by the data's
filter), always prepared, known, or cast free. A free cast is `uses`
times a day, at will without `uses`, or paid with a `resource`.

```toml
[[feature]]
id = "classFeature:Mystic Arcanum|Warlock|XPHB|11"

[[feature.spell]]
choose = "level=6|class=Warlock"       # or: spell = "spell:Fireball|XPHB"
grant = "free"                         # prepared, known or free
uses = 1                               # or: resource = "Soul Fragment"
```

A class's features are its own. A book's class made from another (its
version of the Barbarian, say) shares that class's features, but not
its rules: it has rules of its own, which name it.

```toml
[[feature]]
id = "classFeature:Rage|Barbarian|XPHB|1"
class = "Shaman|WoW"                   # the class these rules are for
```

### The rules library

The [rules library](https://github.com/v0v3s/dnt-rules) has rules for the
SRD and for homebrew that dnt does not have built in, and takes pull
requests. Open it from *Data › Rules library* (the app), `L` on
**4 Campaigns** (the terminal app), or `dnt rules`. Tick a folder, a file
or single rules (`space` in the terminal), then import them (`enter`, or
`dnt rules import srd`): they become your rules. Each rule says whether it
is new, yours already, or differs from yours; rules that differ ask first,
one by one (*Overwrite* or *Keep*) or all at once.

![The rules library](images/app-rules-library.png)
