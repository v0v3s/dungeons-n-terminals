"""The SRD's feats in the data format."""

import re

import tags
from pdf import join
from tags import SOURCE
from text import entries

HEADING = "#88191f"
CATEGORIES = {"Origin": "O", "General": "G", "Fighting Style": "FS", "Epic Boon": "EB"}
ABILITY = {a: a[:3].lower() for a in tags.ABILITIES}
# Magic Initiate's spell lists.
INITIATE = ("Cleric", "Druid", "Wizard")


def feats(lines, pdf_path):
    chunks = []
    for k, l in enumerate(lines):
        nxt = lines[k + 1] if k + 1 < len(lines) else None
        if l.color == HEADING and l.size == 18 and nxt is not None and nxt.italic and " Feat" in nxt.text:
            chunks.append([l])
        elif chunks:
            if l.color == HEADING and l.size >= 21:
                chunks.append([])  # a section heading: not a feat's text
            else:
                chunks[-1].append(l)
    return [one(c, pdf_path) for c in chunks if c]


def one(chunk, pdf_path):
    name = chunk[0].text.strip()
    i, kind = 1, ""
    while i < len(chunk) and chunk[i].italic:
        kind = join(kind, chunk[i].text)
        i += 1
    m = re.match(r"(.+?) Feat(?: \(Prerequisite: (.*)\))?$", kind.strip())
    feat = {"name": name, "source": SOURCE, "page": chunk[0].page, "edition": "one", "category": CATEGORIES[m.group(1)]}
    if m.group(2):
        feat["prerequisite"] = [prerequisite(m.group(2))]
    text = entries(chunk[i:], pdf_path)
    increase = next((e for e in text if isinstance(e, dict) and e.get("name") == "Ability Score Increase"), None)
    if increase:
        ability = _increase(" ".join(tags.walk_text([increase])))
        if ability:
            feat["ability"] = [ability]
    if any(isinstance(e, dict) and e.get("name") == "Repeatable" for e in text):
        feat["repeatable"] = True
    feat["entries"] = text
    if name == "Magic Initiate":
        feat["_versions"] = [
            {
                "name": f"Magic Initiate; {c}",
                "source": SOURCE,
                "additionalSpells": [{"known": {"_": [{"choose": f"level=0|class={c}", "count": 2}, {"choose": f"level=1|class={c}"}]}}],
            }
            for c in INITIATE
        ]
    return feat


def prerequisite(text):
    """`Level 4+, Strength or Dexterity 13+`; `Fighting Style Feature`."""
    out = {}
    for part in re.split(r",\s*", text):
        if m := re.fullmatch(r"Level (\d+)\+", part):
            out["level"] = int(m.group(1))
        elif m := re.fullmatch(rf"((?:{'|'.join(tags.ABILITIES)})(?: or (?:{'|'.join(tags.ABILITIES)}))*) (\d+)\+", part):
            out["ability"] = [{ABILITY[a]: int(m.group(2))} for a in m.group(1).split(" or ")]
        elif part == "Spellcasting Feature":
            out["spellcasting2020"] = True
        else:
            out["other"] = part
    return out


def _increase(text):
    """`Increase your Strength or Dexterity score by 1, to a maximum of
    20`; `Increase one ability score of your choice by 2`."""
    names = re.findall("|".join(tags.ABILITIES), text)
    amount = re.search(r"by (\d)", text)
    if not amount:
        return None
    n = int(amount.group(1))
    if "of your choice" in text and not names:
        return {"choose": {"from": list(ABILITY.values()), "amount": n}}
    if len(names) == 1:
        return {ABILITY[names[0]]: n}
    if names:
        return {"choose": {"from": [ABILITY[a] for a in names], "amount": n}}
    return None
