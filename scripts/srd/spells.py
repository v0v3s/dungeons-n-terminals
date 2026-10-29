"""The SRD's spells (its "Spell Descriptions") in the data format."""

import re

import tags
from pdf import join
from text import entries
from tags import SOURCE

SCHOOLS = {
    "Abjuration": "A", "Conjuration": "C", "Divination": "D", "Enchantment": "E",
    "Evocation": "V", "Illusion": "I", "Necromancy": "N", "Transmutation": "T",
}
HEADING = "#88191f"
KIND = re.compile(r"(Level \d \w+|\w+ Cantrip) \(")
LABELS = {"Casting Time", "Range", "Components", "Component", "Duration"}
# Headings the PDF sets in a way that does not come out as text.
BROKEN = {"A S": "Acid Splash"}


def spells(lines, pdf_path):
    """[(spell, classes)], from the chapter's lines."""
    chunks = []
    for k, l in enumerate(lines):
        if l.color == HEADING and l.size < 18 and chunks and chunks[-1][-1].color == HEADING:
            continue  # the rest of a broken heading
        nxt = next((x for x in lines[k + 1 :] if x.color != HEADING), None)
        if l.color == HEADING and l.size == 18 and nxt is not None and nxt.italic and KIND.match(nxt.text):
            chunks.append([l])
        elif chunks:
            chunks[-1].append(l)
    out = [parse(c, pdf_path) for c in chunks]
    names = {s["name"].lower() for s, _ in out}
    for s, _ in out:
        finish(s, {"spell": names})
    return out


def parse(chunk, pdf_path):
    name = chunk[0].text.strip()
    name = BROKEN.get(name, name)
    i = 1
    kind = ""
    while i < len(chunk) and chunk[i].italic and not chunk[i].bold:
        kind = join(kind, chunk[i].text)
        i += 1
    m = re.match(r"Level (\d) (\w+) \((.*)\)", kind) or re.match(r"(\w+) (Cantrip) \((.*)\)", kind)
    if not m:
        raise ValueError(f"{name}: {kind!r}")
    if m.group(2) == "Cantrip":
        level, school = 0, m.group(1)
    else:
        level, school = int(m.group(1)), m.group(2)
    classes = [c.strip() for c in m.group(3).split(",")]
    # The fields: label lines, each maybe going on to the next lines.
    fields = {}
    label = None
    while i < len(chunk):
        l = chunk[i]
        first = l.runs[0]
        if first.bold and first.text.strip()[:-1] in LABELS and first.text.strip().endswith(":"):
            label = first.text.strip()[:-1]
            label = "Components" if label == "Component" else label
            fields[label] = "".join(r.text for r in l.runs[1:]).strip()
        elif label and l.sans and not first.bold and len(fields) < 4:
            fields[label] = join(fields[label], l.text)
        else:
            break
        i += 1
    body = entries(chunk[i:], pdf_path)
    spell = {"name": name, "source": SOURCE, "page": chunk[0].page, "edition": "one", "level": level, "school": SCHOOLS[school]}
    spell.update(time(fields["Casting Time"]))
    spell["range"] = range_(fields["Range"])
    spell["components"] = components(fields["Components"])
    spell["duration"] = duration(fields["Duration"])
    higher = [e for e in body if isinstance(e, dict) and e.get("name") in ("Using a Higher-Level Spell Slot", "Cantrip Upgrade")]
    spell["entries"] = [e for e in body if e not in higher]
    if higher:
        spell["entriesHigherLevel"] = higher
    return spell, classes


def finish(spell, names):
    """Tags, and what the text says about damage, saves and attacks."""
    spell["entries"] = tags.map_text(spell["entries"], lambda t: tags.tag(t, names))
    text = " ".join(tags.walk_text(spell["entries"]))
    higher = spell.get("entriesHigherLevel")
    if higher:
        spell["entriesHigherLevel"] = tags.map_text(higher, lambda t: higher_level(t, spell))
    if m := re.search(r"\b(ranged|melee) spell attack", text):
        spell["spellAttack"] = [m.group(1)[0].upper()]
    for key, found in (
        ("damageInflict", tags.damage_types(text)),
        ("savingThrow", tags.saving_throws(text)),
        ("conditionInflict", tags.conditions(text)),
    ):
        if found:
            spell[key] = found
    if spell["level"] == 0 and higher:
        scaling = cantrip_scaling(spell)
        if scaling:
            spell["scalingLevelDice"] = scaling


def higher_level(text, spell):
    """`The damage increases by 1d6 for each spell slot level above 3`:
    the dice as {@scaledamage base|levels|step} when the spell has one
    damage roll."""
    base = re.findall(r"\{@damage ([^}]+)\}", " ".join(tags.walk_text(spell["entries"])))
    m = re.search(rf"(increases by )({tags.DICE})( for each spell slot level above (\d))", text)
    if m and len(set(base)) == 1 and spell["level"] > 0:
        dice = tags._dice(m.group(2))
        levels = f"{spell['level']}-9"
        text = text[: m.start(2)] + "{@scaledamage " + f"{base[0]}|{levels}|{dice}" + "}" + text[m.end(2) :]
        return tags.tag(text)
    return tags.tag(text)


def cantrip_scaling(spell):
    """`The damage increases by 1d10 when you reach levels 5 (2d10), 11
    (3d10), and 17 (4d10)`: the dice at each level."""
    text = " ".join(tags.walk_text(spell["entriesHigherLevel"]))
    steps = re.findall(r"(\d+) \((?:\{@dice )?(\d+d\d+)\}?\)", text)
    base = re.findall(r"\{@damage (\d+d\d+)[^}]*\}", " ".join(tags.walk_text(spell["entries"])))
    if len(steps) < 3 or not base:
        return None
    types = spell.get("damageInflict", [])
    label = (types[0].title() + " damage") if len(types) == 1 else "Damage"
    scaling = {"1": base[0]}
    scaling.update({lvl: dice for lvl, dice in steps})
    return {"label": label, "scaling": scaling}


# ---- the fields ----

UNITS = {"Action": "action", "Bonus Action": "bonus", "Reaction": "reaction"}


def time(text):
    out = {}
    text = text.strip()
    if text.endswith(" or Ritual"):
        out["meta"] = {"ritual": True}
        text = text[: -len(" or Ritual")]
    m = re.match(r"(Action|Bonus Action|Reaction)(?:, (.*))?$", text)
    if m:
        t = {"number": 1, "unit": UNITS[m.group(1)]}
        if m.group(2):
            t["condition"] = m.group(2)
        out["time"] = [t]
        return out
    # Two ways: `Action (Overgrowth) or 8 hours (Enrichment)`.
    two = re.match(r"(Action|\d+ \w+) \((\w+)\) or (Action|\d+ \w+) \((\w+)\)$", text)
    if two:
        out["time"] = [dict(time(two.group(k))["time"][0], note=two.group(k + 1)) for k in (1, 3)]
        return out
    m = re.match(r"(\d+) (minute|hour)s?$", text)
    if not m:
        raise ValueError(f"casting time {text!r}")
    out["time"] = [{"number": int(m.group(1)), "unit": m.group(2)}]
    return out


def range_(text):
    text = text.strip()
    if text == "Self":
        return {"type": "point", "distance": {"type": "self"}}
    if text == "Touch":
        return {"type": "point", "distance": {"type": "touch"}}
    if text in ("Sight", "Unlimited"):
        return {"type": "point", "distance": {"type": text.lower()}}
    if text == "Special":
        return {"type": "special"}
    m = re.match(r"Self \((\d+)-(foot|mile) ([\w-]+)\)$", text)
    if m:
        shape = m.group(3).lower()
        shape = {"emanation": "emanation"}.get(shape, shape)
        return {"type": shape, "distance": {"type": "feet" if m.group(2) == "foot" else "miles", "amount": int(m.group(1))}}
    m = re.match(r"([\d,]+) (feet|foot|miles?)$", text)
    if m:
        unit = "feet" if m.group(2) in ("feet", "foot") else "miles"
        return {"type": "point", "distance": {"type": unit, "amount": int(m.group(1).replace(",", ""))}}
    raise ValueError(f"range {text!r}")


def components(text):
    out = {}
    m = re.match(r"([VSM, ]+?)(?: \((.*)\))?$", text.strip())
    if not m:
        raise ValueError(f"components {text!r}")
    letters = [c.strip() for c in m.group(1).split(",") if c.strip()]
    for c in letters:
        out[c.lower()] = True
    if "M" in letters and m.group(2):
        material = m.group(2)
        cost = re.search(r"worth ([\d,]+)\+? (GP|SP|CP)", material)
        if cost or "consumes" in material:
            mat = {"text": material}
            if cost:
                mat["cost"] = int(cost.group(1).replace(",", "")) * {"GP": 100, "SP": 10, "CP": 1}[cost.group(2)]
            if "consumes" in material:
                mat["consume"] = True
            out["m"] = mat
        else:
            out["m"] = material
    return out


def duration(text):
    text = text.strip()
    if text == "Instantaneous":
        return [{"type": "instant"}]
    if text == "Special":
        return [{"type": "special"}]
    if text.startswith("Until dispelled"):
        ends = ["dispel"] + (["trigger"] if "triggered" in text else [])
        return [{"type": "permanent", "ends": ends}]
    text = re.sub(r"up to(\d)", r"up to \1", text)
    concentration = bool(re.match(r"Concentration,? up to ", text))
    text = re.sub(r"^Concentration,? up to ", "", text)
    up_to = text.startswith("Up to ")
    text = text.removeprefix("Up to ")
    m = re.match(r"(\d+) (round|minute|hour|day)s?$", text)
    if not m:
        raise ValueError(f"duration {text!r}")
    d = {"type": "timed", "duration": {"type": m.group(2), "amount": int(m.group(1))}}
    if concentration:
        d["concentration"] = True
    if up_to:
        d["duration"]["upTo"] = True
    return [d]
