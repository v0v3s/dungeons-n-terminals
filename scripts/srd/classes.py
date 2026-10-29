"""The SRD's classes: each class, its features, its subclass and the
subclass's features, in the data format.

A class's chapter is: the class heading, its Core Traits table, its
Features table (one row per level, set across the page), "Becoming a …",
"… Class Features" (a `Level N: Name` heading per feature), maybe a list
of options (invocations, metamagic: see options.py), a spell list (the
spells chapter has those), and "… Subclass: Name" with its features.
"""

import re

import equipment
import tags
from pdf import join
from tags import SOURCE
from text import boxes, entries

HEADING = "#88191f"
FEATURE = re.compile(r"Level (\d+): (.+)$")
ABILITY = {a: a[:3].lower() for a in tags.ABILITIES}

# What the tables do not say.
CASTERS = {
    "Bard": ("full", "cha"), "Cleric": ("full", "wis"), "Druid": ("full", "wis"),
    "Paladin": ("1/2", "cha"), "Ranger": ("1/2", "wis"), "Sorcerer": ("full", "cha"),
    "Warlock": ("pact", "cha"), "Wizard": ("full", "int"),
}


def classes(lines, pdf_path):
    """{"class": […], "subclass": […], "classFeature": […],
    "subclassFeature": […]} and the lines of each class's option lists
    ({"Eldritch Invocation Options": lines})."""
    out = {"class": [], "subclass": [], "classFeature": [], "subclassFeature": []}
    options = {}
    for chunk in _split(lines, lambda l: l.color == HEADING and l.size == 27):
        one(chunk, pdf_path, out, options)
    return out, options


def _split(lines, starts):
    chunks = []
    for l in lines:
        if starts(l):
            chunks.append([l])
        elif chunks:
            chunks[-1].append(l)
    return chunks


def one(chunk, pdf_path, out, options):
    name = chunk[0].text.strip()
    traits_title = next(l for l in chunk if l.sans and l.text.strip() == f"Core {name} Traits")
    table_title = next(l for l in chunk if l.sans and l.size == 16 and l.text.strip() == f"{name} Features")
    rows, labels, bottom = features_table(pdf_path, table_title)
    # The tables are not text: leave them out of the features.
    traits = [l for l in chunk if l.page == traits_title.page and l.column == traits_title.column and l.sans and l.top > traits_title.top and l.size == 14 and l.top < traits_title.top + 400]
    traits = _contiguous(traits, traits_title)
    on_table = lambda l: l.page == table_title.page and table_title.top <= l.top <= bottom
    body = [l for l in chunk[1:] if l not in traits and l is not traits_title and not on_table(l)]

    cls = {"name": name, "source": SOURCE, "page": chunk[0].page, "edition": "one"}
    cls.update(core_traits(traits))
    caster = CASTERS.get(name)
    if caster:
        cls["casterProgression"], cls["spellcastingAbility"] = caster
    cls.update(progressions(name, rows, labels))
    cls["subclassTitle"] = f"{name} Subclass"

    sections = _split(body, lambda l: l.color == HEADING and l.size == 21 and not _goes_on(body, l))
    features = {}
    subclass = None
    for s in sections:
        title = s[0].text.strip()
        if title.startswith(f"{name} Subclass:"):
            heading = [l for l in s if l.color == HEADING and l.size == 21]
            sub_name = " ".join(l.text.strip() for l in heading).split(":", 1)[1].strip()
            sub_name = re.sub(r"\s+", " ", sub_name)
            subclass = (sub_name, _features([l for l in s if l not in heading], pdf_path))
        elif title == f"{name} Class Features":
            features = _features(s[1:], pdf_path)
        elif title.endswith(" Options"):
            options[title] = s[1:]
    _class_features(cls, rows, features, out)
    if subclass:
        _subclass(cls, *subclass, out)
    out["class"].append(cls)


def _goes_on(body, line):
    """The second line of a heading set on two lines."""
    k = body.index(line)
    prev = body[k - 1] if k else None
    return prev is not None and prev.color == HEADING and prev.size == 21 and prev.page == line.page and 0 < line.top - prev.top < 30


def _contiguous(lines, title):
    """The rows right under a table's title, up to the first gap."""
    out, last = [], title.top
    for l in sorted(lines, key=lambda l: l.top):
        if l.top - last > 40:
            break
        out.append(l)
        last = l.top
    return out


# ---- the Core Traits table ----


def core_traits(lines):
    fields, label = {}, None
    for l in lines:
        first = l.runs[0]
        if first.bold:
            label = first.text.strip()
            fields[label] = "".join(r.text for r in l.runs[1:]).strip()
        elif label:
            fields[label] = join(fields[label], l.text)
    # `(see “Equipment”)`: where to read more.
    fields = {k: re.sub(r"\s*\(see [^)]*\)", "", v) for k, v in fields.items()}
    out = {}
    hd = re.match(r"D(\d+)", fields["Hit Point Die"])
    out["hd"] = {"number": 1, "faces": int(hd.group(1))}
    out["proficiency"] = [ABILITY[a] for a in re.findall("|".join(tags.ABILITIES), fields["Saving Throw"])]
    primary = fields["Primary Ability"]
    names = [ABILITY[a] for a in re.findall("|".join(tags.ABILITIES), primary)]
    # `Strength or Dexterity`: either; `Dexterity and Wisdom`: both.
    out["primaryAbility"] = [{a: True} for a in names] if " or " in primary else [{a: True for a in names}]
    start = {}
    armor = fields.get("Armor Training", "None")
    if armor != "None":
        start["armor"] = [a for a in ("light", "medium", "heavy") if a.title() in armor] + (["shield"] if "Shields" in armor else [])
    weapons = fields.get("Weapon Proficiencies", "")
    start["weapons"] = _weapons(weapons)
    if fields.get("Tool Proficiencies"):
        start["tools"] = [fields["Tool Proficiencies"]]
    start["skills"] = _skills(fields["Skill Proficiencies"])
    out["startingProficiencies"] = start
    out["startingEquipment"] = {"default": [fields["Starting Equipment"]], "defaultData": equipment.parse(fields["Starting Equipment"])}
    return out


def _weapons(text):
    """`Simple and Martial weapons`, `Simple weapons and Martial weapons
    that have the Finesse or Light property`."""
    out = []
    if text.startswith("Simple"):
        out.append("simple")
    rest = re.sub(r"^Simple weapons( and )?|^Simple and ", "", text)
    if rest.startswith("Martial weapons") and rest != "Martial weapons":
        out.append(rest[0].lower() + rest[1:])
    elif "Martial" in rest:
        out.append("martial")
    return out


def _skills(text):
    """`Choose 2: Animal Handling, …, or Survival`; `Arcana, Deception,
    Choose 2: …`; `Choose any 3 skills`."""
    out = []
    m = re.search(r"Choose (any )?(\d+)(?: skills)?:?\s*(.*)$", text)
    fixed = text[: m.start()] if m else text
    for s in re.split(r",\s*", fixed.strip(" ,")):
        if s:
            out.append({s.lower(): True})
    if m:
        count = int(m.group(2))
        if m.group(1):
            out.append({"any": count})
        else:
            names = [s.strip().lower() for s in re.split(r",\s*(?:or\s+)?|\s+or\s+", m.group(3).rstrip(".")) if s.strip()]
            out.append({"choose": {"from": names, "count": count}})
    # The data writes fixed skills and a choice as one object.
    merged = {}
    for o in out:
        merged.update(o)
    return [merged]


# ---- the Features table ----


def features_table(pdf_path, title):
    """The rows (level → {column: text}) of a class's Features table, its
    other columns' labels (in order) and where it ends."""
    lines = {}
    for x0, x1, y, w in boxes(pdf_path, title.page):
        if y > title.top + 5:
            key = next((k for k in lines if abs(k - y) <= 4), y)
            lines.setdefault(key, []).append((x0, x1, w))
    tops = sorted(lines)
    for t in tops:
        lines[t].sort()
    first = next(t for t in tops if lines[t][0][2] == "1" and len(lines[t]) > 2)
    head = [t for t in tops if t < first and first - t < 60]
    body, bottom = [], first
    for t in tops:
        if t < first:
            continue
        ws = lines[t]
        level = ws[0][2]
        if len(body) == 20 or t - bottom > 30:
            break
        if re.fullmatch(r"\d+", level) and int(level) == len(body) + 1 and ws[0][0] < 140:
            body.append(ws)
        elif body:
            body[-1] = body[-1] + ws  # a row going on to a second line
        bottom = t
    # Columns: Level, Proficiency Bonus, Class Features, then the rest:
    # one per phrase on the header's last line, right of Class Features.
    last = lines[head[-1]]
    features_x = next(x1 for x0, x1, w in last if w == "Features")
    columns = []  # [x0, x1, label]
    for x0, x1, w in last:
        if x0 <= features_x:
            continue
        if columns and x0 - columns[-1][1] < 8:
            columns[-1][1], columns[-1][2] = x1, columns[-1][2] + " " + w
        else:
            columns.append([x0, x1, w])
    centre = lambda c: (c[0] + c[1]) / 2
    # Words above: the first part of a label (`Rage` over `Damage`), or a
    # title over several columns (`Spell Slots per Spell Level`), left out.
    for t in head[:-1]:
        phrases = []
        for x0, x1, w in lines[t]:
            if x0 <= features_x:
                continue
            if phrases and x0 - phrases[-1][1] < 8:
                phrases[-1][1], phrases[-1][2] = x1, phrases[-1][2] + " " + w
            else:
                phrases.append([x0, x1, w])
        for x0, x1, w in phrases:
            under = [c for c in columns if c[0] < x1 and c[1] > x0]
            if len(under) <= 2:
                c = min(columns, key=lambda c: abs(centre(c) - (x0 + x1) / 2))
                c[2] = w + " " + c[2]
    starts = columns[0][0] - 15 if columns else 10_000
    rows = []
    for r in body:
        cells = {"level": int(r[0][2]), "bonus": r[1][2], "features": "", "extra": [""] * len(columns)}
        for x0, x1, w in r[2:]:
            if x0 >= starts:
                k = min(range(len(columns)), key=lambda k: abs(centre(columns[k]) - (x0 + x1) / 2))
                cells["extra"][k] = (cells["extra"][k] + " " + w).strip()
            else:
                cells["features"] = join(cells["features"], w)
        rows.append(cells)
    return rows, [c[2] for c in columns], bottom + 10


def progressions(name, rows, labels):
    """The class's table groups, and the progressions the data keeps
    beside them (cantrips, prepared spells)."""
    out = {}
    cols = {label: [r["extra"][k] for r in rows] for k, label in enumerate(labels)}
    slots = [l for l in labels if re.fullmatch(r"\d", l)]
    plain = [l for l in labels if l not in slots]

    def value(v):
        v = v.strip()
        if re.fullmatch(r"D\d+", v):
            return "1d" + v[1:]
        return int(v) if re.fullmatch(r"\d+", v) else v

    if "Cantrips" in cols:
        out["cantripProgression"] = [value(v) for v in cols["Cantrips"]]
    if "Prepared Spells" in cols:
        out["preparedSpellsProgression"] = [value(v) for v in cols["Prepared Spells"]]
    groups = []
    if name == "Warlock":
        plain = [l for l in plain if l not in ("Spell Slots", "Slot Level")]
    if plain:
        groups.append({"colLabels": plain, "rows": [[value(cols[l][i]) for l in plain] for i in range(20)]})
    if slots:
        groups.append({
            "title": "Spell Slots per Spell Level",
            "colLabels": [_ordinal(int(s)) for s in slots],
            "rowsSpellProgression": [[value(cols[s][i]) if cols[s][i] not in ("—", "") else 0 for s in slots] for i in range(20)],
        })
    if name == "Warlock":
        groups.append({
            "title": "Pact Magic",
            "colLabels": ["Spell Slots", "Slot Level"],
            "rows": [[value(cols["Spell Slots"][i]), _ordinal(int(cols["Slot Level"][i]))] for i in range(20)],
        })
    out["classTableGroups"] = groups
    return out


def _ordinal(n):
    return f"{n}{ {1: 'st', 2: 'nd', 3: 'rd'}.get(n, 'th') }"


# ---- the features ----


def _features(lines, pdf_path):
    """{(level, name): entries} from `Level N: Name` headings; the text
    before the first one under (0, "")."""
    out = {}
    current, body = (0, ""), []
    for l in lines:
        m = FEATURE.match(l.text.strip()) if l.color == HEADING and l.size == 18 else None
        if m:
            out[current] = entries(body, pdf_path)
            current, body = (int(m.group(1)), m.group(2).strip()), []
        else:
            body.append(l)
    out[current] = entries(body, pdf_path)
    return out


def _table_names(row):
    text = row["features"]
    if text in ("—", ""):
        return []
    return [n.strip() for n in re.split(r",\s*(?![^()]*\))", text)]


def _class_features(cls, rows, features, out):
    name = cls["name"]
    refs = []
    used = {(0, "")}
    for row in rows:
        level = row["level"]
        for feature in _table_names(row):
            ref = f"{feature}|{name}|{SOURCE}|{level}"
            if feature == f"{name} Subclass" or feature == "Subclass feature":
                feature = feature if feature != "Subclass feature" else "Subclass Feature"
                ref = f"{feature}|{name}|{SOURCE}|{level}"
                refs.append({"classFeature": ref, "gainSubclassFeature": True})
            else:
                refs.append(ref)
            text = features.get((level, feature))
            plain = re.sub(r" \(.*\)$", "", feature)
            if text is None:
                text = features.get((level, plain))
            used.update(k for k in features if k[1] in (feature, plain))
            if text is None:
                # Said once for several levels (Ability Score Improvement),
                # or not at all (Subclass feature).
                earlier = sorted(k for k in features if k[1] in (feature, plain))
                if earlier:
                    text = features[earlier[0]]
                elif feature == "Subclass Feature":
                    text = [f"You gain a feature from your {name} subclass."]
                else:
                    raise ValueError(f"{name}: no text for {feature} at level {level}")
            out["classFeature"].append({
                "name": feature, "source": SOURCE, "page": cls["page"], "className": name,
                "classSource": SOURCE, "level": level, "entries": text,
            })
    cls["classFeatures"] = refs
    left = [k for k in features if k not in used]
    if left:
        raise ValueError(f"{name}: features not in the table: {left}")


def _subclass(cls, name, features, out):
    # `Path of the Berserker` → `Berserker`, `Life Domain` → `Life`.
    short = re.sub(r"^(Path|College|Circle|Oath|Warrior) of (the )?| (Domain|Patron|Sorcery)$", "", name)
    sub = {
        "name": name, "shortName": short, "source": SOURCE, "className": cls["name"], "classSource": SOURCE,
        "page": cls["page"], "edition": "one", "subclassFeatures": [],
    }
    intro = features.pop((0, ""), [])
    levels = sorted({lvl for lvl, _ in features})
    for i, level in enumerate(levels):
        here = [(lvl, n) for lvl, n in features if lvl == level]
        # The first feature at a level carries the rest (the data's way).
        head_name = name if i == 0 else here[0][1]
        refs = f"{head_name}|{cls['name']}|{SOURCE}|{short}|{SOURCE}|{level}"
        sub["subclassFeatures"].append(refs)
        body = intro if i == 0 else []
        own = []
        for lvl, n in here:
            if i == 0 or n != head_name:
                own.append({"type": "refSubclassFeature", "subclassFeature": f"{n}|{cls['name']}|{SOURCE}|{short}|{SOURCE}|{level}"})
                out["subclassFeature"].append(_sub_feature(cls, short, n, level, features[(lvl, n)]))
        if i == 0:
            out["subclassFeature"].append(_sub_feature(cls, short, name, level, body + own))
        else:
            out["subclassFeature"].append(_sub_feature(cls, short, head_name, level, features[here[0]] + own))
    spells = _spell_table(features)
    if spells:
        sub["additionalSpells"] = [{"prepared": spells}]
    out["subclass"].append(sub)


def _spell_table(features):
    """A subclass's always-prepared spells, from its one spell table
    (`Cleric Level | Prepared Spells`): {"3": ["aid|srd52", …]}."""
    found = []

    def walk(es):
        for e in es:
            if isinstance(e, dict):
                labels = e.get("colLabels", [])
                if e.get("type") == "table" and len(labels) == 2 and labels[0].endswith(" Level") and labels[1].endswith("Spells"):
                    found.append(e)
                walk(e.get("entries", []))

    for es in features.values():
        walk(es)
    if len(found) != 1:
        return None
    return {r[0]: [f"{n.strip().lower()}|{SOURCE.lower()}" for n in r[1].split(",")] for r in found[0]["rows"]}


def finish(out, names):
    """Tag the text: dice, conditions, and spells named in italics."""
    for kind in ("classFeature", "subclassFeature"):
        for f in out[kind]:
            f["entries"] = tags.map_text(f["entries"], lambda t: tags.tag(t, names))


def _sub_feature(cls, short, name, level, text):
    return {
        "name": name, "source": SOURCE, "page": cls["page"], "className": cls["name"], "classSource": SOURCE,
        "subclassShortName": short, "subclassSource": SOURCE, "level": level, "entries": text,
    }
