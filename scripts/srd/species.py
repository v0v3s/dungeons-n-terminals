"""The SRD's species in the data format: each with its traits, and its
lineages (or ancestries, legacies) as versions."""

import re

from pdf import join
from tags import SOURCE
from text import entries

HEADING = "#88191f"
SIZES = {"Tiny": "T", "Small": "S", "Medium": "M", "Large": "L"}
CASTING = {"choose": ["int", "wis", "cha"]}


def ref(name, cantrip=False):
    return f"{name.lower()}|{SOURCE.lower()}" + ("#c" if cantrip else "")


def spells(cantrip, third=None, fifth=None):
    """A lineage's spells: a cantrip, then a spell at levels 3 and 5 once
    a day each."""
    out = {"known": {"1": [ref(c, True) for c in ([cantrip] if isinstance(cantrip, str) else cantrip)]}, "ability": CASTING}
    innate = {}
    for level, spell in (("3", third), ("5", fifth)):
        if spell:
            innate[level] = {"daily": {"1": [ref(spell)]}}
    if innate:
        out["innate"] = innate
    return [out]


# The choices the tables and the text offer: (trait they replace, name,
# what they change).
LINEAGES = {
    "Dragonborn": ("Draconic Ancestry", [
        (f"{color} Dragon Ancestry", {"resist": [damage]})
        for color, damage in (
            ("Black", "acid"), ("Blue", "lightning"), ("Brass", "fire"), ("Bronze", "lightning"), ("Copper", "acid"),
            ("Gold", "fire"), ("Green", "poison"), ("Red", "fire"), ("Silver", "cold"), ("White", "cold"),
        )
    ]),
    "Elf": ("Elven Lineage", [
        ("Drow Lineage", {"darkvision": 120, "additionalSpells": spells("Dancing Lights", "Faerie Fire", "Darkness")}),
        ("High Elf Lineage", {"additionalSpells": spells("Prestidigitation", "Detect Magic", "Misty Step")}),
        ("Wood Elf Lineage", {"speed": 35, "additionalSpells": spells("Druidcraft", "Longstrider", "Pass without Trace")}),
    ]),
    "Gnome": ("Gnomish Lineage", [
        ("Forest Gnome Lineage", {"additionalSpells": [{"known": {"1": [ref("Minor Illusion", True)]}, "prepared": {"1": [ref("Speak with Animals")]}, "ability": CASTING}]}),
        ("Rock Gnome Lineage", {"additionalSpells": spells(["Mending", "Prestidigitation"])}),
    ]),
    "Tiefling": ("Fiendish Legacy", [
        ("Abyssal Legacy", {"resist": ["poison"], "additionalSpells": spells("Poison Spray", "Ray of Sickness", "Hold Person")}),
        ("Chthonic Legacy", {"resist": ["necrotic"], "additionalSpells": spells("Chill Touch", "False Life", "Ray of Enfeeblement")}),
        ("Infernal Legacy", {"resist": ["fire"], "additionalSpells": spells("Fire Bolt", "Hellish Rebuke", "Darkness")}),
    ]),
}
# What else the traits give, said in words.
EXTRA = {
    "Dwarf": {"resist": ["poison"]},
    "Human": {"skillProficiencies": [{"any": 1}], "feats": [{"anyFromCategory": {"category": ["O"], "count": 1}}]},
    "Elf": {"skillProficiencies": [{"choose": {"from": ["insight", "perception", "survival"], "count": 1}}]},
    "Orc": {},
}


def species(lines, pdf_path):
    start = next(k for k, l in enumerate(lines) if l.text.strip() == "Species Descriptions")
    chunks = []
    for l in lines[start + 1 :]:
        if l.color == HEADING and l.size > 18:
            break
        if l.color == HEADING and l.size == 18:
            chunks.append([l])
        elif chunks:
            chunks[-1].append(l)
    out = [one(c, pdf_path) for c in chunks]
    _move_tables(out)
    return out


def _move_tables(races):
    """A table set at the foot of a page, after the next species: to the
    trait that names it (`the Fiendish Legacies table`)."""
    tables = {}
    for r in races:
        for e in r["entries"]:
            if isinstance(e, dict):
                for t in [x for x in e.get("entries", []) if isinstance(x, dict) and x.get("type") == "table" and x.get("caption")]:
                    tables[t["caption"]] = (e, t)
    for r in races:
        for e in r["entries"]:
            if not isinstance(e, dict) or e.get("type") != "entries":
                continue
            text = " ".join(x for x in e["entries"] if isinstance(x, str))
            for caption, (owner, table) in tables.items():
                if owner is not e and f"the {caption} table" in text and table not in e["entries"]:
                    owner["entries"].remove(table)
                    e["entries"].append(table)


def one(chunk, pdf_path):
    name = chunk[0].text.strip()
    fields, label, i = {}, None, 1
    while i < len(chunk) and chunk[i].sans and chunk[i].size == 15:
        l = chunk[i]
        first = l.runs[0]
        if first.bold:
            label = first.text.strip().rstrip(":")
            fields[label] = "".join(r.text for r in l.runs[1:]).strip()
        else:
            fields[label] = join(fields[label], l.text)
        i += 1
    text = entries(chunk[i:], pdf_path)
    race = {
        "name": name, "source": SOURCE, "page": chunk[0].page, "edition": "one",
        "size": [SIZES[s] for s in re.findall("|".join(SIZES), fields["Size"])],
        "speed": int(re.match(r"(\d+)", fields["Speed"]).group(1)),
        "creatureTypes": [fields["Creature Type"].lower()],
    }
    race["size"].sort(key=list(SIZES.values()).index)
    vision = next((e for e in text if isinstance(e, dict) and e.get("name") == "Darkvision"), None)
    if vision:
        race["darkvision"] = int(re.search(r"(\d+) feet", " ".join(vision["entries"])).group(1))
    race.update(EXTRA.get(name, {}))
    race["entries"] = text
    if name in LINEAGES:
        trait, versions = LINEAGES[name]
        race["_versions"] = [
            dict({"name": f"{name}; {v}", "source": SOURCE}, **changes, _mod={"entries": {"mode": "replaceArr", "replace": trait, "items": _chosen(text, trait, v)}})
            for v, changes in versions
        ]
    return race


def _chosen(text, trait, version):
    """The trait, named for the choice (`Elven Lineage (Drow)`)."""
    named = {e.get("name"): e for e in text if isinstance(e, dict)}
    choice = re.sub(r" (Lineage|Legacy|Dragon Ancestry)$", "", version)
    # The choice's own text, where it has some (`Forest Gnome.`).
    own = named.get(choice, {}).get("entries", [])
    return {"type": "entries", "name": f"{trait} ({choice})", "entries": named[trait]["entries"] + own}
