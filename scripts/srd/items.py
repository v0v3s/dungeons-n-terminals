"""The SRD's equipment in the data format: weapons and armor (base
items), weapon masteries, tools, adventuring gear, focuses, ammunition,
mounts and vehicles (items)."""

import re
from fractions import Fraction

import equipment

from pdf import join
from tags import SOURCE
from text import _phrases, boxes, entries

HEADING = "#88191f"
T = f"|{SOURCE}"
PROPERTIES = {
    "Ammunition": "A", "Finesse": "F", "Heavy": "H", "Light": "L", "Loading": "LD", "Reach": "R",
    "Thrown": "T", "Two-Handed": "2H", "Versatile": "V",
}
DAMAGE = {"Bludgeoning": "B", "Piercing": "P", "Slashing": "S"}
ARMOR = {"Light": "LA", "Medium": "MA", "Heavy": "HA", "Shield": "S"}
# The forms a focus takes: (table, scfType).
FOCUSES = {"Arcane Focuses": "arcane", "Druidic Focuses": "druid", "Holy Symbols": "holy"}
# Ammunition by the piece: the table sells bundles (`Arrows`, 20).
AMMUNITION = {"Arrows": "Arrow", "Bolts": "Bolt", "Bullets, Firearm": "Firearm Bullet", "Bullets, Sling": "Sling Bullet", "Needles": "Needle"}


def cost(text):
    """`25 GP` → 2500 (copper pieces); `Varies` → None."""
    m = re.fullmatch(r"([\d,]+) (GP|SP|CP|EP|PP)", text.strip())
    if not m:
        return None
    return int(m.group(1).replace(",", "")) * {"PP": 1000, "GP": 100, "EP": 50, "SP": 10, "CP": 1}[m.group(2)]


def weight(text):
    """`1 lb.`, `1/2 lb.`, `58½ lb.`, `5 lb. (full)`; `—` → None."""
    m = re.match(r"(\d*)(½|/\d+)?(\d*) lb", text.replace("1/2", "½").strip())
    if not m:
        return None
    whole = int(m.group(1) or 0)
    half = Fraction(1, 2) if m.group(2) == "½" else Fraction(0)
    w = whole + half
    return int(w) if w.denominator == 1 else float(w)


# ---- tables read by where their words are ----


def grid(pdf_path, page, title_top, header=None):
    """A table under a title (at `title_top`): its rows as {label: text},
    with lines that go on from a row joined to it. The columns are the
    phrases of the header line, or `header` (the labels, in order)."""
    lines = {}
    for x0, x1, y, w in boxes(pdf_path, page):
        if y > title_top + 5:
            key = next((k for k in lines if abs(k - y) <= 4), y)
            lines.setdefault(key, []).append((x0, x1, w))
    tops = sorted(lines)
    head = sorted(lines[tops[0]])
    columns = _phrases(head, gap=12)
    if header:
        # Labels set apart from their first word (`Armor Class (AC)`).
        starts = []
        for label in header:
            first = label.split()[0]
            starts.append(next(x0 for x0, _, w in head if w == first and all(x0 > s for s in starts)))
        columns = [(s, None, label) for s, label in zip(starts, header)]
    starts = [c[0] for c in columns]
    labels = [c[2] for c in columns]
    col = lambda x: max([k for k, s in enumerate(starts) if x >= s - 8] or [0])
    rows, last = [], tops[0]
    for t in tops[1:]:
        if t - last > 45 or _footer(lines[t]):
            break
        last = t
        cells = [""] * len(starts)
        for x0, _, w in sorted(lines[t]):
            cells[col(x0)] = (cells[col(x0)] + " " + w).strip()
        if rows and not cells[0]:
            rows[-1] = [(a + " " + b).strip() for a, b in zip(rows[-1], cells)]
        else:
            rows.append(cells)
    return [dict(zip(labels, _right_aligned(r))) for r in rows]


def _right_aligned(cells):
    """A cost set flush right starts left of its column: `20 lb. 400` |
    `GP` is `20 lb.` | `400 GP`."""
    for k in range(len(cells) - 1):
        m = re.search(r"\s([\d,]+)$", cells[k])
        if m and re.match(r"(GP|SP|CP)\b", cells[k + 1]):
            cells[k] = cells[k][: m.start()]
            cells[k + 1] = m.group(1) + " " + cells[k + 1]
    return cells


def _footer(words):
    return "Reference" in [w for _, _, w in words]


def _title(lines, text, page=None):
    return next(l for l in lines if l.sans and l.size == 16 and l.text.strip() == text and (page is None or l.page == page))


# ---- weapons and armor ----


def weapons(lines, pdf_path):
    title = _title(lines, "Weapons")
    out, category = [], None
    for r in grid(pdf_path, title.page, title.top, ["Name", "Damage", "Properties", "Mastery", "Weight", "Cost"]):
        if m := re.fullmatch(r"(Simple|Martial) (Melee|Ranged) Weapons", r["Name"]):
            category = (m.group(1).lower(), "M" if m.group(2) == "Melee" else "R")
            continue
        dice, kind = r["Damage"].rsplit(" ", 1)
        w = {
            "name": r["Name"], "source": SOURCE, "page": title.page, "type": category[1] + T, "weapon": True,
            "weaponCategory": category[0], "dmg1": dice, "dmgType": DAMAGE[kind], "rarity": "none",
        }
        props = []
        for p in re.split(r",\s*(?![^()]*\))", r["Properties"]):
            p = p.strip()
            if p in ("—", ""):
                continue
            name = re.match(r"[\w-]+", p).group(0)
            props.append(PROPERTIES[name] + T)
            if rng := re.search(r"Range (\d+/\d+)", p):
                w["range"] = rng.group(1)
            if v := re.fullmatch(r"Versatile \((\d+d\d+)\)", p):
                w["dmg2"] = v.group(1)
        if props:
            w["property"] = props
        w["mastery"] = [r["Mastery"] + T]
        if (lb := weight(r["Weight"])) is not None:
            w["weight"] = lb
        if (cp := cost(r["Cost"])) is not None:
            w["value"] = cp
        out.append(w)
    return out


def armor(lines, pdf_path):
    title = _title(lines, "Armor")
    out, kind = [], None
    for r in grid(pdf_path, title.page, title.top, ["Armor", "Armor Class (AC)", "Strength", "Stealth", "Weight", "Cost"]):
        if m := re.match(r"(Light|Medium|Heavy) Armor \(|Shield \(", r["Armor"]):
            kind = ARMOR[m.group(1) or "Shield"]
            continue
        ac = re.match(r"\+?(\d+)", r["Armor Class (AC)"]).group(1)
        a = {"name": r["Armor"], "source": SOURCE, "page": title.page, "type": kind + T, "ac": int(ac), "rarity": "none"}
        if kind != "S":
            a["armor"] = True
        if (s := re.match(r"Str (\d+)", r["Strength"])) is not None:
            a["strength"] = s.group(1)
        if r["Stealth"] == "Disadvantage":
            a["stealth"] = True
        if (lb := weight(r["Weight"])) is not None:
            a["weight"] = lb
        if (cp := cost(r["Cost"])) is not None:
            a["value"] = cp
        out.append(a)
    return out


def masteries(lines, pdf_path):
    """The Mastery Properties: each heading and its text."""
    start = next(k for k, l in enumerate(lines) if l.text.strip() == "Mastery Properties")
    out, chunk = [], None
    for l in lines[start + 1 :]:
        if l.color == HEADING and l.size >= 21:
            break
        if l.color == HEADING and l.size == 18:
            chunk = [l]
            out.append(chunk)
        elif chunk is not None:
            chunk.append(l)
    return [{"name": c[0].text.strip(), "source": SOURCE, "page": c[0].page, "entries": entries(c[1:], pdf_path)} for c in out]


# ---- tools and gear ----

NAMED = re.compile(r"(.+) \(([^()]+)\)$")


def described(lines, pdf_path, first, last):
    """Entries set as `Name (cost)` headings, pages `first` to `last`:
    {name: (cost, section, lines)}."""
    out, section, current = {}, None, None
    for l in lines:
        if not first <= l.page <= last:
            continue
        if l.color == HEADING and l.size >= 27 and l.text.strip() == "Mounts and Vehicles":
            break
        if l.color == HEADING and l.size >= 21:
            section, current = l.text.strip(), None
            continue
        if l.color == HEADING and l.size == 18 and (m := NAMED.match(l.text.strip())):
            current = []
            out[m.group(1)] = (m.group(2), section, current)
        elif current is not None:
            if l.color == HEADING:
                current = None
            else:
                current.append(l)
    return out


def tool(name, price, section, body, pdf_path):
    fields, label = {}, None
    rest = []
    for l in body:
        if l.sans and l.size == 14:
            for r in l.runs:
                if r.bold and r.text.strip().endswith(":"):
                    label = r.text.strip()[:-1]
                    fields[label] = ""
                elif label:
                    fields[label] += r.text
            if label:
                fields[label] += "\n"
        else:
            rest.append(l)
    # Lines joined as text (`dul-` + `cimer`).
    fields = {k: _joined(v) for k, v in fields.items()}
    kind = "AT" if section == "Artisan’s Tools" else {"Gaming Set": "GS", "Musical Instrument": "INS"}.get(name, "T")
    item = {"name": name, "source": SOURCE, "page": body[0].page if body else None, "type": kind + T, "rarity": "none"}
    if (cp := cost(price)) is not None:
        item["value"] = cp
    if (lb := weight(fields.get("Weight", ""))) is not None:
        item["weight"] = lb
    item["entries"] = [{"type": "entries", "name": k, "entries": [v]} for k, v in fields.items() if k != "Weight"] + entries(rest, pdf_path)
    variants = []
    if v := fields.get("Variants"):
        # `Dice (1 SP), dragonchess (1 GP), …`: an item each.
        for m in re.finditer(r"([^,()]+?) \(([^()]+)\)", v):
            variant = dict(item, name=_capitalized(m.group(1).strip()))
            price, _, lb = m.group(2).partition(", ")
            if (cp := cost(price)) is not None:
                variant["value"] = cp
            if (w := weight(lb)) is not None:
                variant["weight"] = w
            variants.append(variant)
    return item, variants


def _joined(text):
    out = ""
    for line in text.split("\n"):
        out = join(out, line)
    return re.sub(r"\s+", " ", out).strip()


def _capitalized(name):
    """`playing cards` → `Playing Cards`; `Sprig of mistletoe` → `Sprig
    of Mistletoe`."""
    small = {"of", "and", "the", "a"}
    words = name.split()
    cap = lambda w: "-".join(p[:1].upper() + p[1:] for p in w.split("-"))
    return " ".join(w if k and w in small else cap(w) for k, w in enumerate(words))


def gear(lines, pdf_path):
    """Adventuring gear: the table's weights and costs, the descriptions'
    text, and the forms of focuses and ammunition from their tables."""
    found = described(lines, pdf_path, 93, 100)
    found.pop("Potion of Healing", None)  # a magic item: with those
    titles = {l.text.strip(): l for l in lines if l.sans and l.size == 16 and 93 <= l.page <= 99}
    table = {}
    gear_title = titles["Adventuring Gear"]
    for x0, x1 in ((0, gear_title.middle), (gear_title.middle, 10_000)):
        for r in _column_grid(pdf_path, gear_title.page, gear_title.top, x0, x1):
            table[r["Item"]] = r
    out, tools = [], []
    for name, (price, section, body) in found.items():
        if section in ("Artisan’s Tools", "Other Tools"):
            item, variants = tool(name, price, section, body, pdf_path)
            tools.append(item)
            tools.extend(variants)
            continue
        # `Clothes, Traveler’s` in the tables is `Traveler’s Clothes` in the
        # text.
        item = {"name": _uninverted(name), "source": SOURCE, "page": body[0].page if body else None, "type": "G" + T, "rarity": "none"}
        row = table.get(name) or table.get(_inverted(name), {})
        if (cp := cost(price)) is not None:
            item["value"] = cp
        if (lb := weight(row.get("Weight", ""))) is not None:
            item["weight"] = lb
        text = entries(body, pdf_path)
        holds = re.search(r"holds up to (\d+) pounds", " ".join(t for t in text if isinstance(t, str)))
        if holds:
            item["containerCapacity"] = {"weight": [int(holds.group(1))]}
        if contents := pack_contents(" ".join(t for t in text if isinstance(t, str))):
            item["packContents"] = contents
        item["entries"] = text
        out.append(item)
    for caption, scf in FOCUSES.items():
        t = titles[caption]
        for r in _column_grid(pdf_path, t.page, t.top, *_column(t)):
            form = _capitalized(re.sub(r" \(.*\)$", "", r["Focus"] if "Focus" in r else r["Symbol"]))
            what = {"Arcane Focuses": "an Arcane Focus", "Druidic Focuses": "a Druidic Focus", "Holy Symbols": "a Holy Symbol"}[caption]
            item = {"name": form, "source": SOURCE, "type": "SCF" + T, "scfType": scf, "rarity": "none", "entries": [f"A spellcasting focus: {what}."]}
            if (cp := cost(r["Cost"])) is not None:
                item["value"] = cp
            if (lb := weight(r["Weight"])) is not None:
                item["weight"] = lb
            out.append(item)
    # Ammunition is a base item: magic ammunition is made from it.
    ammunition = []
    t = titles["Ammunition"]
    for r in _column_grid(pdf_path, t.page, t.top, *_column(t)):
        # Its columns are close and flush right: read the row as a whole.
        m = re.fullmatch(r"(.+?) (\d+) (\w+) ([\d½]+ lb\.) ([\d,]+ (?:GP|SP|CP))", " ".join(r.values()))
        kind, amount, storage, lb, price = m.groups()
        amount = int(amount)
        item = {"name": AMMUNITION[kind], "source": SOURCE, "type": "A" + T, "rarity": "none", "entries": [f"Sold {amount} at a time, stored in a {storage}."]}
        cp = cost(price)
        item["value"] = cp // amount if cp % amount == 0 else round(cp / amount, 2)
        item["weight"] = round(weight(lb) / amount, 3)
        ammunition.append(item)
    return out, tools, ammunition


def pack_contents(text):
    """`… contains the following items: Backpack, 10 Candles, 7 flasks of
    Oil, …, and Waterskin.`: the items, with how many."""
    m = re.search(r"contains the following items: (.*?)\.?$", text)
    if not m:
        return None
    out = []
    for part in re.split(r",\s*(?:and\s+)?", m.group(1)):
        ref = lambda name: f"{name.lower()}|{SOURCE.lower()}"
        if n := re.fullmatch(r"(\d+) (?:flasks|days|sheets) of (.+)", part):
            out.append({"item": ref(n.group(2)), "quantity": int(n.group(1))})
        elif n := re.fullmatch(r"(\d+) (.+)", part):
            out.append({"item": ref(equipment.singular(n.group(2))), "quantity": int(n.group(1))})
        else:
            out.append(ref(part))
    return out


def _inverted(name):
    """`Hooded Lantern` is `Lantern, Hooded` in the table."""
    words = name.split()
    return f"{words[-1]}, {' '.join(words[:-1])}" if len(words) > 1 else name


def _column(title):
    return (0, title.middle) if title.column == 0 else (title.middle, 10_000)


def _column_grid(pdf_path, page, title_top, x0, x1):
    """`grid` for a table set in one column of the page."""
    lines = {}
    for a, b, y, w in boxes(pdf_path, page):
        if y > title_top + 5 and x0 <= a < x1:
            key = next((k for k in lines if abs(k - y) <= 4), y)
            lines.setdefault(key, []).append((a, b, w))
    tops = sorted(lines)
    columns = _phrases(sorted(lines[tops[0]]), gap=12)
    starts, labels = [c[0] for c in columns], [c[2] for c in columns]
    col = lambda x: max([k for k, s in enumerate(starts) if x >= s - 30] or [0])
    rows, last = [], tops[0]
    for t in tops[1:]:
        ws = sorted(lines[t])
        # The table ends at a gap, or the page's foot.
        if t - last > 30 or _footer(ws):
            break
        last = t
        cells = [""] * len(starts)
        for a, _, w in ws:
            cells[col(a)] = (cells[col(a)] + " " + w).strip()
        rows.append(dict(zip(labels, cells)))
    return rows


# ---- mounts and vehicles ----


def mounts(lines, pdf_path):
    """Mounts, tack and drawn vehicles, and ships."""
    titles = {l.text.strip(): l for l in lines if l.sans and l.size == 16 and l.page in (100, 101)}
    out = []
    t = titles["Mounts and Other Animals"]
    for r in _column_grid(pdf_path, t.page, t.top, *_column(t)):
        item = {"name": _uninverted(r["Item"]), "source": SOURCE, "type": "MNT" + T, "rarity": "none", "value": cost(r["Cost"])}
        if m := re.match(r"([\d,]+) lb", r["Carrying Capacity"]):
            item["carryingCapacity"] = int(m.group(1).replace(",", ""))
        out.append(item)
    t = titles["Tack, Harness, and Drawn Vehicles"]
    under = None
    for r in _column_grid(pdf_path, t.page, t.top, *_column(t)):
        name = r["Item"]
        if name == "Large Vehicles":
            break
        if not r["Cost"]:
            under = name  # `Saddle`, its kinds below
            continue
        if name.endswith(" per day"):
            continue  # a service, not an item
        if under and name in ("Exotic", "Military", "Riding"):
            name = f"{name} {under}"
        else:
            under = None
        kind = "TAH" if "Saddle" in name else "VEH"
        item = {"name": name, "source": SOURCE, "type": kind + T, "rarity": "none", "value": cost(r["Cost"])}
        if (lb := weight(r["Weight"])) is not None:
            item["weight"] = lb
        out.append(item)
    t = titles["Airborne and Waterborne Vehicles"]
    for r in grid(pdf_path, t.page, t.top):
        name, speed = r["Ship"], r["Speed"]
        if m := re.fullmatch(r"(.+) ([\d½]+)", name):
            name, speed = m.group(1), f"{m.group(2)} {speed}"
        item = {"name": name, "source": SOURCE, "type": "SHP" + T, "rarity": "none", "value": cost(r["Cost"])}
        item["entries"] = [f"Speed {speed}; crew {r['Crew']}, passengers {r['Passengers']}, cargo {r['Cargo (Tons)']} tons; AC {r['AC']}, HP {r['HP']}, damage threshold {r['Damage Threshold']}."]
        out.append(item)
    return out


def _uninverted(name):
    """`Horse, Riding` → `Riding Horse`."""
    first, _, rest = name.partition(", ")
    return f"{rest} {first}" if rest else name
