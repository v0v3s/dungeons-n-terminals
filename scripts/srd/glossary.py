"""The SRD's Rules Glossary (conditions, actions, senses, the other rules),
its skills and its languages, in the data format."""

import re

import tags
from tags import SOURCE
from text import entries, tables

HEADING = "#88191f"
SENSES = {"Blindsight", "Darkvision", "Tremorsense", "Truesight"}
ABILITY = {a: a[:3].lower() for a in tags.ABILITIES}
# How long an action takes, when not an action.
TIMES = {"Ready": "action", "Magic": "action"}


def glossary(lines, pdf_path):
    """{"condition": […], "action": […], "sense": […], "variantrule": […]}
    from the Rules Definitions."""
    start = next(k for k, l in enumerate(lines) if l.text.strip() == "Rules Definitions")
    chunks = []
    for l in lines[start + 1 :]:
        if l.color == HEADING and l.size >= 27:
            break
        if l.color == HEADING and l.size == 18:
            chunks.append([l])
        elif chunks:
            chunks[-1].append(l)
    out = {"condition": [], "action": [], "sense": [], "variantrule": []}
    for c in chunks:
        title = c[0].text.strip()
        m = re.fullmatch(r"(.+?) \[(.+)\]", title)
        name, kind = (m.group(1), m.group(2)) if m else (title, None)
        e = {"name": name, "source": SOURCE, "page": c[0].page, "entries": entries(c[1:], pdf_path)}
        if kind == "Condition":
            out["condition"].append(e)
        elif kind == "Action":
            e["time"] = [{"number": 1, "unit": "action"}]
            out["action"].append(e)
        elif name in SENSES:
            out["sense"].append(e)
        else:
            if kind:
                e["name"] = f"{name} ({kind})" if kind != "Area of Effect" else name
            e["ruleType"] = "C"
            out["variantrule"].append(e)
    return out


def _table(lines, pdf_path, caption):
    k = next(i for i, l in enumerate(lines) if l.sans and l.size == 16 and l.text.strip() == caption)
    j = k + 1
    while j < len(lines) and lines[j].sans and lines[j].column == lines[k].column and not (lines[j].size == 16 and j > k + 1):
        j += 1
    return tables(lines[k:j], pdf_path)[0]


def skills(lines, pdf_path):
    t = _table(lines, pdf_path, "Skills")
    return [
        {"name": skill, "source": SOURCE, "page": lines[0].page, "ability": ABILITY[ability], "entries": [uses]}
        for skill, ability, uses in t["rows"]
    ]


def languages(lines, pdf_path):
    out = []
    for caption, kind in (("Standard Languages", "standard"), ("Rare Languages", "rare")):
        t = _table(lines, pdf_path, caption)
        cells = [r[-1] for r in t["rows"]] if kind == "standard" else [c for r in t["rows"] for c in r]
        for name in cells:
            name = name.rstrip("*").strip()
            # The table's note on Primordial's dialects is not a language.
            if not name or len(name.split()) > 3 or name[0].islower() or name.startswith(("*", "Creatures", "Auran")):
                continue
            entry = {"name": name, "source": SOURCE, "type": kind, "entries": [f"A {kind} language."]}
            if name == "Primordial":
                entry["entries"] = ["Primordial includes the Aquan, Auran, Ignan, and Terran dialects. Creatures that know one of these dialects can communicate with those that know a different one."]
            out.append(entry)
    return out
