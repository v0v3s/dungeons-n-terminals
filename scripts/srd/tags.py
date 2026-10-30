"""Inline tags in text (`{@damage 2d6}`, `{@condition Prone|SRD52}`, …)
and the facts the data format keeps beside the text: damage types,
saving throws, conditions.
"""

import re

SOURCE = "SRD52"

DAMAGE_TYPES = [
    "Acid", "Bludgeoning", "Cold", "Fire", "Force", "Lightning", "Necrotic",
    "Piercing", "Poison", "Psychic", "Radiant", "Slashing", "Thunder",
]
CONDITIONS = [
    "Blinded", "Charmed", "Deafened", "Exhaustion", "Frightened", "Grappled",
    "Incapacitated", "Invisible", "Paralyzed", "Petrified", "Poisoned", "Prone",
    "Restrained", "Stunned", "Unconscious",
]
ABILITIES = ["Strength", "Dexterity", "Constitution", "Intelligence", "Wisdom", "Charisma"]

DICE = r"\d*d\d+(?:\s?[+−-]\s?\d+)?"
_TYPES = "|".join(DAMAGE_TYPES)


def _dice(d: str) -> str:
    return d.replace("−", "-").replace(" ", "")


def tag(text: str, names=None) -> str:
    """Tag a paragraph. `names`: {"spell": {...}} entity names known so far,
    tagged where they appear in italics (marked `_Name_` by the reader)."""
    # Dice followed by a damage type are damage; other dice are dice.
    text = re.sub(
        rf"\b({DICE})(\)?\s(?:{_TYPES})(?:\s(?:or\s(?:{_TYPES})\s)?)?damage)",
        lambda m: "{@damage " + _dice(m.group(1)) + "}" + m.group(2),
        text,
    )
    text = re.sub(
        rf"(?<![{{@\w])\b({DICE})\b(?![^{{]*}})",
        lambda m: "{@dice " + _dice(m.group(1)) + "}",
        text,
    )
    # Conditions, named as such ("the Prone condition", "is Frightened").
    text = re.sub(
        rf"\b({'|'.join(CONDITIONS)})\b(?=\scondition)",
        lambda m: "{@condition " + m.group(1) + "|" + SOURCE + "}",
        text,
    )
    # Italic names of other entities.
    if names:

        def italic(m):
            name = m.group(1)
            for kind, known in names.items():
                if name.lower() in known:
                    return "{@" + kind + " " + name + "|" + SOURCE + "}"
            return name

        text = re.sub(r"_([^_]+)_", italic, text)
    return text.replace("_", "")


def damage_types(text: str) -> list:
    found = set()
    for m in re.finditer(rf"\b(?:{DICE}\)?\s)?((?:{_TYPES})(?:\sor\s(?:{_TYPES}))?)\sdamage", text):
        for t in re.split(r"\sor\s", m.group(1)):
            found.add(t.lower())
    return sorted(found)


def saving_throws(text: str) -> list:
    return sorted({m.group(1).lower() for m in re.finditer(rf"\b({'|'.join(ABILITIES)}) saving throw", text)})


def conditions(text: str) -> list:
    return sorted({m.group(1).lower() for m in re.finditer(rf"\b({'|'.join(CONDITIONS)})\scondition", text)})


def walk_text(entries):
    """Every string in entries."""
    for e in entries:
        if isinstance(e, str):
            yield e
        elif isinstance(e, dict):
            for k in ("entries", "items", "rows"):
                if k in e:
                    yield from walk_text(e[k] if k != "rows" else [c for r in e[k] for c in r])
            if "entry" in e:
                yield from walk_text([e["entry"]])


def map_text(entries, f):
    """Entries with `f` applied to every string."""
    out = []
    for e in entries:
        if isinstance(e, str):
            out.append(f(e))
        elif isinstance(e, dict):
            e = dict(e)
            for k in ("entries", "items"):
                if k in e:
                    e[k] = map_text(e[k], f)
            if "rows" in e:
                e["rows"] = [[f(c) for c in r] for r in e["rows"]]
            out.append(e)
        else:
            out.append(e)
    return out
