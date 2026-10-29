"""The SRD's Magic Items A–Z in the data format: items, and magic variants
for those made from any of several base items (`Weapon (Any Simple or
Martial)`, `Armor, +1, +2, or +3`)."""

import re

from pdf import join
from tags import SOURCE
from text import entries

HEADING = "#88191f"
T = f"|{SOURCE}"
KINDS = {"Potion": "P", "Ring": "RG", "Rod": "RD", "Scroll": "SC", "Staff": "ST", "Wand": "WD", "Wondrous Item": "G"}
RARITIES = ["Common", "Uncommon", "Rare", "Very Rare", "Legendary", "Artifact"]
META = re.compile(r"(Armor|Weapon|Potion|Ring|Rod|Scroll|Staff|Wand|Wondrous Item)(?: \(([^)]*)\))?, (.*)$")
# What `Any …` means among the base items.
ANY = {
    "Any Light, Medium, or Heavy": [{"type": "LA" + T}, {"type": "MA" + T}, {"type": "HA" + T}],
    "Any Medium or Heavy, Except Hide Armor": [{"type": "MA" + T}, {"type": "HA" + T}],
    "Any Simple or Martial": [{"weapon": True}],
    "Any Melee Weapon": [{"type": "M" + T}],
    "Any Ammunition": [{"type": "A" + T}],
}


def magic_items(lines, pdf_path, bases):
    """(items, variants), `bases`: the base items by name."""
    start = next(k for k, l in enumerate(lines) if l.text.strip() == "Magic Items A–Z")
    chunks, i = [], start + 1
    lines = lines[i:]
    i = 0
    while i < len(lines):
        l = lines[i]
        if l.color == HEADING and l.size == 18:
            name, j = l.text.strip(), i + 1
            while j < len(lines) and lines[j].color == HEADING and lines[j].size == 18 and lines[j].top - lines[j - 1].top < 30:
                name += " " + lines[j].text.strip()
                j += 1
            meta = ""
            while j < len(lines) and lines[j].italic and lines[j].size == 15:
                meta = join(meta, lines[j].text)
                j += 1
            if meta:
                chunks.append((name, meta, l.page, []))
                i = j
                continue
        if chunks:
            chunks[-1][3].append(l)
        i += 1
    items, variants = [], []
    for name, meta, page, body in chunks:
        made = one(name, meta, page, entries(body, pdf_path), bases)
        (variants if made and made[0].get("requires") else items).extend(made)
    return items, variants


def _rarity(text):
    """`Rare`, `Very Rare (Requires Attunement by a Wizard)`: (rarity,
    attunement)."""
    attune = None
    if m := re.search(r"\(Requires Attunement(?: (by [^)]*))?\)", text):
        attune = m.group(1) or True
        text = text[: m.start()].strip()
    return text, attune


def one(name, meta, page, text, bases):
    m = META.match(meta)
    kind, sub, rest = m.groups()
    rest, attune = _rarity(rest)
    common = {"source": SOURCE, "page": page}
    if attune:
        common["reqAttune"] = attune
    # `Rare (+1), Very Rare (+2), or Legendary (+3)`: one each.
    steps = re.findall(r"(Uncommon|Rare|Very Rare|Legendary) \(\+(\d)\)", rest)
    names = re.split(r",\s*(?:or\s+)?|\s+or\s+", sub) if sub and not sub.startswith("Any") else []
    if kind in ("Armor", "Weapon") and (sub in ANY or len(names) > 1 or steps):
        requires = ANY.get(sub) or [{"name": n} for n in names]
        out = []
        if steps:
            base_name = re.sub(r",? \+1, \+2, or \+3$", "", name)
            for rarity, n in steps:
                bonus = {"bonusAc": f"+{n}"} if kind == "Armor" else {"bonusWeapon": f"+{n}"}
                out.append({
                    "name": f"{base_name} +{n}" if kind == "Armor" else f"+{n} {base_name}", "type": "GV" + T, "requires": requires, **common,
                    "inherits": dict(namePrefix=f"+{n} ", source=SOURCE, rarity=rarity.lower(), entries=text, **bonus, **({"reqAttune": attune} if attune else {})),
                })
            return out
        rarity = rest.lower()
        inherits = {"namePrefix": f"{name} (", "nameSuffix": ")", "source": SOURCE, "rarity": rarity, "entries": text}
        if attune:
            inherits["reqAttune"] = attune
        return [{"name": name, "type": "GV" + T, "requires": requires, **common, "inherits": inherits}]
    item = {"name": name, **common, "rarity": rest.lower() if rest in RARITIES else "varies"}
    if kind in ("Armor", "Weapon"):
        base = bases[sub]
        for k in ("type", "weapon", "weaponCategory", "dmg1", "dmg2", "dmgType", "property", "range", "mastery", "ac", "armor", "strength", "stealth", "weight"):
            if k in base:
                item[k] = base[k]
        item["baseItem"] = f"{sub}|{SOURCE}"
    else:
        item["type"] = KINDS[kind] + T
        if kind == "Wondrous Item":
            item["wondrous"] = True
    item["entries"] = text
    return [item]
