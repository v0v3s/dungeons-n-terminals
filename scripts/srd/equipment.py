"""Starting equipment (`Choose A or B: (A) Greataxe, 4 Handaxes, …, and
15 GP; or (B) 75 GP`) as the data format's item lists."""

import re

from tags import SOURCE

# What is not one item: a kind to choose from.
KINDS = {
    "Holy Symbol": "focusSpellcastingHoly",
    "Musical Instrument of your choice": "instrumentMusical",
    "Gaming Set (same as above)": "setGaming",
}
FOCUS = re.compile(r"(?:Arcane|Druidic) Focus \((.+)\)$")
# `Book (prayers)`: a book about something; `Parchment (10 sheets)`: so many.
ABOUT = re.compile(r"(.+?) \((.+)\)$")
NUMBER = re.compile(r"(\d+) (.+)$")


def parse(text):
    """[{"A": [...], "B": [...]}]."""
    text = text.replace("Short sword", "Shortsword")  # the Rogue's, as printed
    text = re.sub(r"^Choose [A-C, or]+:\s*", "", text.strip())
    out = {}
    for m in re.finditer(r"\(([A-C])\) (.*?)(?=;? (?:or )?\([A-C]\)|$)", text):
        out[m.group(1)] = [entry(e) for e in _items(m.group(2))]
    return [out]


def _items(text):
    text = re.sub(r",? and (\d+ GP)$", r", \1", text.strip().rstrip(";"))
    return [e.strip() for e in re.split(r",\s*(?![^()]*\))", text) if e.strip()]


def ref(name):
    return f"{name.lower()}|{SOURCE.lower()}"


def entry(text):
    if m := re.fullmatch(r"([\d,]+) GP", text):
        return {"value": int(m.group(1).replace(",", "")) * 100}
    if text in KINDS:
        return {"equipmentType": KINDS[text]}
    if "chosen for" in text:
        return {"special": text}
    if m := FOCUS.match(text):
        return {"item": ref(m.group(1)), "displayName": text}
    if m := ABOUT.match(text):
        if n := re.fullmatch(r"(\d+) \w+", m.group(2)):
            return {"item": ref(m.group(1)), "quantity": int(n.group(1))}
        return {"item": ref(m.group(1)), "displayName": text}
    if m := NUMBER.match(text):
        return {"item": ref(singular(m.group(2))), "quantity": int(m.group(1))}
    return ref(text)


def singular(name):
    if name.endswith("ches"):
        return name[:-2]
    return name[:-1] if name.endswith("s") else name
