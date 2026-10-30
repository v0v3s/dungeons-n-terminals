"""The SRD's backgrounds in the data format."""

import re

import equipment
import tags
from pdf import join
from tags import SOURCE

HEADING = "#88191f"
ABILITY = {a: a[:3].lower() for a in tags.ABILITIES}


def backgrounds(lines):
    """The backgrounds under "Background Descriptions"."""
    start = next(k for k, l in enumerate(lines) if l.text.strip() == "Background Descriptions")
    chunks = []
    for l in lines[start + 1 :]:
        if l.color == HEADING and l.size > 18:
            break
        if l.color == HEADING and l.size == 18:
            chunks.append([l])
        elif chunks:
            chunks[-1].append(l)
    return [one(c) for c in chunks]


def one(chunk):
    fields, label = {}, None
    for l in chunk[1:]:
        first = l.runs[0]
        if first.bold and first.text.strip().endswith(":"):
            label = first.text.strip()[:-1]
            fields[label] = "".join(r.text for r in l.runs[1:]).strip()
        elif label:
            fields[label] = join(fields[label], l.text)
    fields = {k: re.sub(r"\s*\(see [^)]*\)", "", v) for k, v in fields.items()}
    abilities = [ABILITY[a.strip()] for a in fields["Ability Scores"].split(",")]
    feat = fields["Feat"]
    if m := re.fullmatch(r"(.+) \((.+)\)", feat):
        feat = f"{m.group(1)}; {m.group(2)}"
    return {
        "name": chunk[0].text.strip(),
        "source": SOURCE,
        "page": chunk[0].page,
        "edition": "one",
        "ability": [
            {"choose": {"weighted": {"from": abilities, "weights": [2, 1]}}},
            {"choose": {"weighted": {"from": abilities, "weights": [1, 1, 1]}}},
        ],
        "feats": [{f"{feat.lower()}|{SOURCE.lower()}": True}],
        "skillProficiencies": [{s.strip().lower(): True for s in fields["Skill Proficiencies"].split(" and ")}],
        "toolProficiencies": [_tool(fields["Tool Proficiency"])],
        "startingEquipment": equipment.parse(fields["Equipment"]),
        "entries": [{"type": "entries", "name": "Equipment", "entries": [fields["Equipment"]]}],
    }


def _tool(text):
    """`Calligrapher’s Supplies`; `Choose one kind of Gaming Set`."""
    if re.match(r"Choose one kind of Gaming Set", text):
        return {"gaming set": 1}
    return {f"{text.lower()}|{SOURCE.lower()}": True}
