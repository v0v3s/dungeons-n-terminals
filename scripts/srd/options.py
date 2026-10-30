"""A class's list of options (Eldritch Invocations, Metamagic) as the data
format's optional features, and how many of them the class picks."""

import re

from pdf import join
from tags import SOURCE
from text import entries

HEADING = "#88191f"
# Each list: its feature type, and the class that picks from it.
LISTS = {
    "Eldritch Invocation Options": ("EI", "Warlock", "Eldritch Invocations"),
    "Metamagic Options": ("MM", "Sorcerer", "Metamagic"),
}
# How many Metamagic options a Sorcerer has (said in the text, not a
# table column).
METAMAGIC = {"2": 2, "10": 4, "17": 6}


def options(lists, pdf_path):
    """[optional feature], from {"… Options": lines}."""
    out = []
    for title, lines in lists.items():
        kind = LISTS[title][0]
        chunks = []
        for l in lines:
            if l.color == HEADING and l.size == 18:
                chunks.append([l])
            elif chunks:
                chunks[-1].append(l)
        out.extend(one(c, kind, pdf_path) for c in chunks)
    return out


def one(chunk, kind, pdf_path):
    name = chunk[0].text.strip()
    i, note = 1, ""
    while i < len(chunk) and chunk[i].italic:
        note = join(note, chunk[i].text)
        i += 1
    option = {"name": name, "source": SOURCE, "page": chunk[0].page, "featureType": [kind]}
    text = entries(chunk[i:], pdf_path)
    if note.startswith("Prerequisite:"):
        option["prerequisite"] = [prerequisite(note.removeprefix("Prerequisite:").strip())]
    elif note:
        text.insert(0, note)  # `Cost: 1 Sorcery Point`
    option["entries"] = text
    return option


def prerequisite(text):
    """`Level 5+ Warlock, Pact of the Blade Invocation`."""
    out = {}
    for part in re.split(r",\s*", text):
        if m := re.fullmatch(r"Level (\d+)\+ (\w+)", part):
            out["level"] = {"level": int(m.group(1)), "class": {"name": m.group(2)}}
        elif m := re.fullmatch(r"(.+) Invocation", part):
            out.setdefault("optionalfeature", []).append(f"{m.group(1).lower()}|{SOURCE.lower()}")
        elif m := re.fullmatch(r"an? (\w+) Cantrip That Deals Damage( via an Attack Roll)?", part, re.I):
            summary = f"a {m.group(1)} cantrip that deals damage" + (" with an attack roll" if m.group(2) else "")
            out["spell"] = [{"choose": f"level=0|class={m.group(1)}", "entrySummary": summary}]
        else:
            raise ValueError(f"prerequisite {part!r}")
    return out


def progressions(cls):
    """The class's `optionalfeatureProgression`: invocations from the
    table's column, metamagic from the text."""
    for kind, name, feature in LISTS.values():
        if cls["name"] != name:
            continue
        if kind == "MM":
            progression = METAMAGIC
        else:
            group = cls["classTableGroups"][0]
            column = group["colLabels"].index(feature)
            progression = [row[column] for row in group["rows"]]
        cls["optionalfeatureProgression"] = [{"name": feature, "featureType": [kind], "progression": progression}]
