"""The SRD's creatures in the data format: the stat blocks of its
"Monsters A–Z" and "Animals", and those in spells (a summoned creature's).

A stat block: its name, a line of size, type and alignment, the stat
lines (AC, HP, Speed, the ability table, Skills, Senses, Languages, CR,
…), then sections (Traits, Actions, Bonus Actions, Reactions, Legendary
Actions) of named entries.
"""

import re
from copy import copy

import tags
from pdf import Run, join
from tags import CONDITIONS, DAMAGE_TYPES, SOURCE
from text import entries

HEADING = "#88191f"
# The size, type and alignment line; a Legendary Actions section's header.
GREY = "#5a5757"
# The stat lines.
STAT = "#4a0508"
NAME_SIZE = 23
SECTIONS = {"Traits": "trait", "Actions": "action", "Bonus Actions": "bonus", "Reactions": "reaction", "Legendary Actions": "legendary"}
SIZES = {"Tiny": "T", "Small": "S", "Medium": "M", "Large": "L", "Huge": "H", "Gargantuan": "G"}
ALIGNMENTS = {"Lawful": "L", "Neutral": "N", "Chaotic": "C", "Good": "G", "Evil": "E", "Unaligned": "U"}
ABILITIES = ["str", "dex", "con", "int", "wis", "cha"]
FULL = dict(zip(tags.ABILITIES, ABILITIES))


def is_name(line) -> bool:
    return line.color == HEADING and line.size == NAME_SIZE


def span(lines, k):
    """The end of the stat block named at `lines[k]`: its lines are set
    smaller than the text around it, under section headings of their own."""
    j = k + 1
    while j < len(lines):
        l = lines[j]
        if l.size > NAME_SIZE or is_name(l):
            break
        if l.color == HEADING and l.text.strip() not in SECTIONS:
            break
        if l.size >= 15 and not l.sans:
            break  # the text around it again
        j += 1
    return j


def creatures(lines, pdf_path, names):
    """Every stat block in `lines` (the Monsters chapter and the Animals
    appendix). `names`: the known names, lower case ({"spell": {...},
    "item": {...}}), for the tags and the gear."""
    out = []
    k = 0
    while k < len(lines):
        if is_name(lines[k]):
            end = span(lines, k)
            out.append(parse(lines[k], lines[k + 1 : end], pdf_path, names))
            k = end
        else:
            k += 1
    return out


def in_spells(lines, pdf_path, names, spells):
    """The stat blocks in the spells chapter: each the creature its spell
    (the one before it, from `spells`) summons."""
    by_name = {s["name"]: s for s in spells}
    out = []
    spell = None
    for k, l in enumerate(lines):
        if l.color == HEADING and l.size == 18 and l.text.strip() in by_name:
            spell = by_name[l.text.strip()]
        if is_name(l) and spell:
            c = parse(l, lines[k + 1 : span(lines, k)], pdf_path, names)
            c["summonedBySpell"] = f"{spell['name']}|{SOURCE}"
            c["summonedBySpellLevel"] = spell["level"]
            out.append(c)
    return out


def familiars(spells, creatures):
    """Mark the creatures Find Familiar names (`an animal form you choose:
    Bat, Cat, …, or another Beast`) as familiars; the forms it names that
    are not among them."""
    forms = set()
    for s in spells:
        for t in tags.walk_text(s["entries"]):
            if m := re.search(r"an animal form you choose: (.*?),? or another", t):
                forms |= {f.strip() for f in m.group(1).split(",")}
    for c in creatures:
        if c["name"] in forms:
            c["familiar"] = True
    return forms - {c["name"] for c in creatures}


# ---- one stat block ----


def parse(heading, lines, pdf_path, names):
    name = heading.text.strip()
    c = {"name": name, "source": SOURCE, "page": heading.page}
    i = 0
    kind = ""
    while i < len(lines) and lines[i].color == GREY:
        kind = join(kind, lines[i].text)
        i += 1
    c.update(kind_line(kind))
    # The stat lines: `Label value`, a value going on to the next lines.
    stats, table = [], []
    while i < len(lines) and (lines[i].color in (STAT, "#8b8989")):
        l = lines[i]
        i += 1
        if l.sans:
            if l.size == 15:
                table.append(l)
            continue
        if not l.runs[0].bold:
            stats[-1][1] = join(stats[-1][1], l.marked)
            continue
        for r in l.runs:
            if r.bold and r.text.strip():
                stats.append([r.text.strip(), ""])
            elif r.italic and r.text.strip():
                lead, trail = r.text[: len(r.text) - len(r.text.lstrip())], r.text[len(r.text.rstrip()) :]
                stats[-1][1] += f"{lead}_{r.text.strip()}_{trail}"
            else:
                stats[-1][1] += r.text
    # Italics (`_Mind Blank_`) kept for the notes' tags.
    marked = {a: re.sub(r"\s+", " ", re.sub(r"_(\s*)_", r"\1", b)).strip() for a, b in stats}
    _stats(c, {a: b.replace("_", "") for a, b in marked.items()}, marked, table, names)
    # The sections.
    section = None
    parts = {}
    for l in lines[i:]:
        if l.color == HEADING and l.text.strip() in SECTIONS:
            section = SECTIONS[l.text.strip()]
            parts[section] = []
        elif section:
            parts[section].append(l)
    for key in SECTIONS.values():
        if key not in parts:
            continue
        body = _names(parts[key])
        if key == "legendary":
            header = ""
            while body and body[0].color == GREY:
                header = join(header, body[0].text)
                body = body[1:]
            if header:
                c["legendaryHeader"] = [tag(header, names)]
                if m := re.match(r"Legendary Action Uses: (\d+)(?: \((\d+) in Lair\))?", header):
                    c["legendaryActions"] = int(m.group(1))
                    if m.group(2):
                        c["legendaryActionsLair"] = int(m.group(2))
        found = []
        for e in tags.map_text(entries(body, pdf_path), lambda t: tag(t, names)):
            if isinstance(e, dict) and e.get("type") == "entries" and re.match(r"\d+–\d+$", e["name"]) and found:
                # A roll's outcomes (`1–4. The target does nothing.`).
                item = {"type": "item", "name": e["name"], "entries": e["entries"]}
                last = found[-1]["entries"][-1]
                if isinstance(last, dict) and last.get("type") == "list":
                    last["items"].append(item)
                else:
                    found[-1]["entries"].append({"type": "list", "style": "list-hang-notitle", "items": [item]})
            elif isinstance(e, dict) and e.get("type") == "entries":
                found.append({"name": tag_name(e["name"]), "entries": e["entries"]})
            elif found:
                found[-1]["entries"].append(e)
            else:
                raise ValueError(f"{name}: {e!r} before any named entry")
        spells = [e for e in found if re.match(r"Spellcasting\b", e["name"])]
        for e in spells:
            found.remove(e)
            c.setdefault("spellcasting", []).append(spellcasting(e, key, names))
        if found:
            c[key] = found
    return c


def _names(lines):
    """The lines, with an entry's name over two lines on the first (all
    bold, then the bold rest of the name)."""
    out = []
    for l in lines:
        prev = out[-1] if out else None
        if prev and prev.bold and prev.runs[0].italic and not prev.text.rstrip().endswith(".") and l.runs[0].bold:
            joined = copy(prev)
            joined.top = l.top
            joined.runs = prev.runs[:-1] + [Run(prev.runs[-1].text.rstrip() + " ", True, True)] + l.runs
            out[-1] = joined
        else:
            out.append(_period(l))
    return out


def _period(line):
    """A line whose entry's name has its period outside the bold
    (`**Lightning Absorption**. Whenever …`): with it inside."""
    runs = line.runs
    if len(runs) > 1 and runs[0].bold and not runs[0].text.rstrip().endswith(".") and not runs[1].bold and runs[1].text.startswith("."):
        line = copy(line)
        line.runs = [Run(runs[0].text.rstrip() + ".", True, runs[0].italic), Run(runs[1].text[1:], runs[1].bold, runs[1].italic)] + runs[2:]
    return line


def kind_line(text):
    """`Medium or Small Humanoid (Wizard), Neutral`."""
    kind, alignment = text.strip().rsplit(", ", 1)
    out = {}
    m = re.match(rf"((?:{'|'.join(SIZES)})(?: or (?:{'|'.join(SIZES)}|Smaller))?) (.*)$", kind)
    sizes = m.group(1).split(" or ")
    if sizes[-1] == "Smaller":
        order = list(SIZES)
        sizes = order[: order.index(sizes[0]) + 1]
    out["size"] = [SIZES[s] for s in sizes]
    rest = m.group(2)
    note = None
    if n := re.match(r"(.*) \(([^)]*)\)$", rest):
        rest, note = n.group(1), n.group(2)
    if swarm := re.match(r"Swarm of (\w+) (\w+?)s$", rest):
        t = {"type": swarm.group(2).lower(), "swarmSize": SIZES[swarm.group(1)]}
    elif "," in rest or " or " in rest:
        t = {"type": {"choose": [w.lower() for w in re.split(r",\s*(?:or\s+)?|\s+or\s+", rest)]}}
        note = None if note == "Your Choice" else note
    else:
        t = rest.lower()
    if note:
        t = t if isinstance(t, dict) else {"type": t}
        t["tags"] = [w.strip().lower() for w in note.split(",")]
    out["type"] = t
    if alignment == "Neutral":
        out["alignment"] = ["N"]
    else:
        out["alignment"] = [ALIGNMENTS[w] for w in alignment.split()]
    return out


def _stats(c, stats, marked, table, names):
    name = c["name"]
    known = {"AC", "Initiative", "HP", "Speed", "Skills", "Gear", "Resistances", "Vulnerabilities", "Immunities", "Senses", "Languages", "CR"}
    if set(stats) - known:
        raise ValueError(f"{name}: stat lines {set(stats) - known}")
    ac = stats["AC"]
    c["ac"] = [int(ac)] if ac.isdigit() else [{"special": ac}]
    hp = stats["HP"]
    if m := re.match(r"(\d+) \((\d+d\d+(?: [+−] \d+)?)\)$", hp):
        c["hp"] = {"average": int(m.group(1)), "formula": m.group(2).replace("−", "-")}
    else:
        c["hp"] = {"special": hp}
    c["speed"] = speed(stats["Speed"])
    # The ability table: `Str 21 +5 +5`, the score, its modifier, the save.
    # (`Con 22`: a whole word; `−2 2`: a sign left out, the modifier's.)
    found = re.findall(r"\b([SDCIWsdciw])(?:tr|ex|on|nt|is|ha)?\s*(\d+)\s*([+−-]\d+)\s*([+−-]?\d+)", " ".join(l.text for l in table))
    if "".join(f[0].upper() for f in found) != "SDCIWC":
        raise ValueError(f"{name}: abilities {found}")
    saves = {}
    for ability, (_, score, mod, save) in zip(ABILITIES, found):
        c[ability] = int(score)
        if save.isdigit() and save == mod[1:]:
            save = mod
        if save != mod:
            saves[ability] = save.replace("−", "-")
    if saves:
        c["save"] = saves
    if "Skills" in stats:
        c["skill"] = {m.group(1).lower(): m.group(2).replace("−", "-") for m in re.finditer(r"([A-Z][\w ]+?) ([+−-]\d+)", stats["Skills"])}
    if "Gear" in stats:
        c["gear"] = gear(stats["Gear"], names)
    for label, key in (("Resistances", "resist"), ("Vulnerabilities", "vulnerable"), ("Immunities", "immune")):
        if label in stats:
            damage, conditions = defences(marked[label], key, names)
            if damage:
                c[key] = damage
            if conditions:
                c["conditionImmune"] = conditions
    senses, passive = stats["Senses"].rsplit("Passive Perception ", 1)
    senses = [s.strip() for s in _split(senses.strip().rstrip(";")) if s.strip()]
    if senses:
        c["senses"] = [s[0].lower() + s[1:] for s in senses]
    c["passive"] = int(passive)
    languages = languages_(stats["Languages"])
    if languages:
        c["languages"] = languages
    cr = stats["CR"]
    pb = None
    if m := re.match(r"None \(XP 0; PB (.*)\)$", cr):
        c["pbNote"] = m.group(1)
    else:
        m = re.match(r"(\S+) \((?:XP ([\d,]+)|([\d,]+) XP)(?:, or ([\d,]+) in lair)?; PB \+(\d+)\)$", cr)
        if not m:
            raise ValueError(f"{name}: CR {cr!r}")
        c["cr"] = {"cr": m.group(1), "xpLair": int(m.group(4).replace(",", ""))} if m.group(4) else m.group(1)
        pb = int(m.group(5))
    if "Initiative" in stats:
        initiative(c, stats["Initiative"], pb)


def _split(text, sep=","):
    """Split at `sep` outside parentheses."""
    out, depth, cur = [], 0, ""
    for ch in text:
        depth += (ch == "(") - (ch == ")")
        if ch == sep and depth == 0:
            out.append(cur.strip())
            cur = ""
        else:
            cur += ch
    out.append(cur.strip())
    return [o for o in out if o]


def speed(text):
    """`10 ft., Fly 90 ft. (hover)`, `60 ft., Fly 60 ft. (requires level 4+
    spell)`."""
    out = {}
    for part in _split(text):
        if m := re.match(r"(Burrow|Climb|Fly|Swim) or (Burrow|Climb|Fly|Swim) (\d+) ft\.(?: \((.*)\))?$", part):
            out["choose"] = {"from": [m.group(1).lower(), m.group(2).lower()], "amount": int(m.group(3))}
            if m.group(4):
                out["choose"]["note"] = f"({m.group(4)})"
            continue
        m = re.match(r"(?:(Burrow|Climb|Fly|Swim) )?(\d+) ft\.(?: \((.*)\))?$", part)
        if not m:
            raise ValueError(f"speed {text!r}")
        mode = (m.group(1) or "walk").lower()
        n = int(m.group(2))
        if m.group(3) == "hover":
            out["canHover"] = True
            out[mode] = n
        elif m.group(3):
            out[mode] = {"number": n, "condition": f"({m.group(3)})"}
        else:
            out[mode] = n
    if "canHover" in out:
        out["canHover"] = out.pop("canHover")
    return out


def initiative(c, text, pb):
    """`+7 (17)`: the data format keeps it as how many times the PB is in
    it (beyond Dexterity), or as the bonus when it is not a multiple; and
    Advantage, when the score in parentheses has it."""
    m = re.match(r"([+−-]\d+) \((\d+)\)$", text)
    bonus, score = int(m.group(1).replace("−", "-")), int(m.group(2))
    extra = bonus - (c["dex"] - 10) // 2
    out = {}
    if extra and pb and extra % pb == 0:
        out["proficiency"] = extra // pb
    elif extra:
        out["initiative"] = bonus
    if score != 10 + bonus:
        out["advantageMode"] = "adv" if score > 10 + bonus else "dis"
    if "initiative" in out:
        c["initiative"] = out["initiative"] if len(out) == 1 else out
    elif out:
        c["initiative"] = out


def gear(text, names):
    """`Chain Shirt, Holy Symbol, Mace`, `Daggers (3)`: the items."""
    items = names.get("item", set())
    out = []
    for part in _split(text):
        m = re.match(r"(.*?)(?: \((\d+)\))?$", part)
        item, n = m.group(1), m.group(2)
        if item.lower() not in items and item.lower().rstrip("s") in items:
            item = item[:-1]
        if item.lower() not in items:
            raise ValueError(f"gear {item!r}")
        ref = f"{item.lower()}|{SOURCE.lower()}"
        out.append({"item": ref, "quantity": int(n)} if n else ref)
    return out


def defences(text, key, names):
    """`Poison, Psychic; Charmed, Deafened`: (damage types, conditions)."""
    parts = text.split("; ")
    if len(parts) == 1:
        first = re.match(r"\w+", parts[0]).group(0)
        parts = ["", parts[0]] if first in CONDITIONS else [parts[0], ""]
    damage, conditions = parts
    return _list(damage, key, DAMAGE_TYPES, names), _list(conditions, "conditionImmune", CONDITIONS, names)


def _list(text, key, words, names):
    out = []
    if text and not re.match(rf"({'|'.join(words)})\b", text):
        return [{"special": tag(text, names)}]
    for part in _split(text):
        if part in words:
            out.append(part.lower())
        elif m := re.match(rf"({'|'.join(words)})(?: damage)? (.+)$", part):
            e = {key: [m.group(1).lower()], "note": tag(m.group(2), names)}
            if key == "conditionImmune":
                e["cond"] = True
            out.append(e)
        else:
            raise ValueError(f"{key} {part!r}")
    return out


def languages_(text):
    """`Deep Speech; telepathy 120 ft.`, `understands Abyssal, Celestial,
    and Infernal but can’t speak`."""
    if text == "None":
        return []
    out = []
    for part in text.split("; "):
        pieces = _split(part)
        k = 0
        while k < len(pieces):
            piece = pieces[k]
            if re.search("understands", piece, re.I):
                while k + 1 < len(pieces) and not re.search(r"speak|know", piece):
                    k += 1
                    piece += ", " + pieces[k]
            out.append(piece)
            k += 1
    return out


# ---- the text ----

LABELS = re.compile(r"Attack Roll|Saving\s+Throw|Hit|Miss|Failure|Success|Trigger|Response")
ATTACKS = {"Melee": "m", "Ranged": "r", "Melee or Ranged": "m,r"}
DICE = r"\d+d\d+(?: [+−-] \d+)?"
TYPES = "|".join(DAMAGE_TYPES)


def _dice(d):
    return d.replace("−", "-")


def tag(text, names):
    """A stat block's paragraph, with the data format's tags."""
    text = re.sub(r"\s+", " ", text).strip()
    # The italic labels (`_Hit:_`) as plain text; other italics are names.
    text = re.sub(r"_([^_]+)_", lambda m: m.group(1) if LABELS.search(m.group(1)) else m.group(0), text)
    text = re.sub(
        r"(Melee or Ranged|Melee|Ranged) Attack Roll:(?: ([+−-]\d+))?",
        lambda m: "{@atkr " + ATTACKS[m.group(1)] + "}" + (" {@hit " + _dice(m.group(2)).lstrip("+") + "}" if m.group(2) else ""),
        text,
    )
    text = re.sub(
        rf"({'|'.join(tags.ABILITIES)})(?: or ({'|'.join(tags.ABILITIES)}))? Saving\s+Throw: ?",
        lambda m: "{@actSave " + FULL[m.group(1)] + "} " + ("or {@actSave " + FULL[m.group(2)] + "} " if m.group(2) else ""),
        text,
    )
    for label, t in (
        ("Failure or Success:", "{@actSaveSuccessOrFail}"),
        ("Success or Failure:", "{@actSaveSuccessOrFail}"),
        ("First Failure:", "{@actSaveFail 1}"),
        ("Second Failure:", "{@actSaveFail 2}"),
        ("Third Failure:", "{@actSaveFail 3}"),
        ("Failure:", "{@actSaveFail}"),
        ("Subsequent Failures:", "{@i Subsequent Failures:}"),
        ("Success:", "{@actSaveSuccess}"),
        ("Trigger:", "{@actTrigger}"),
        ("Response:", "{@actResponse}"),
        ("Response—", "{@actResponse d}"),
    ):
        text = text.replace(label, t)
    text = re.sub(r"Failure by (\d+) or More:", r"{@actSaveFailBy \1}", text)
    text = re.sub(r"\bHit or Miss: ?", "{@hom}", text)
    text = re.sub(r"\bHit: ?", "{@h}", text)
    text = re.sub(r"\bMiss: ?", "{@m}", text)
    # `12 (2d6 + 5) Bludgeoning damage`; other dice.
    text = re.sub(
        rf"\(({DICE})\)(?=\s(?:{TYPES})(?:\s(?:or\s)?(?:{TYPES}))*\sdamage)",
        lambda m: "({@damage " + _dice(m.group(1)) + "})",
        text,
    )
    text = re.sub(rf"(?<![{{@\w])\b({DICE})\b(?![^{{]*}})", lambda m: "{@dice " + _dice(m.group(1)) + "}", text)
    text = re.sub(r"\bDC (\d+)", r"{@dc \1}", text)
    text = re.sub(r"(?<![{@\w])([+−-]\d+) to hit", lambda m: "{@hit " + _dice(m.group(1)).lstrip("+") + "} to hit", text)
    text = re.sub(
        rf"(?<![{{@\w|])\b({'|'.join(CONDITIONS)})\b(?![^{{]*}})",
        lambda m: "{@condition " + m.group(1) + "|" + SOURCE + "}",
        text,
    )
    text = tags.tag(text, names)
    return re.sub(r"\s+", " ", text).replace("{@h} ", "{@h}").strip()


def tag_name(name):
    """`Fire Breath (Recharge 5–6)`: the recharge as a tag."""
    name = re.sub(r"\s*\(Recharge (\d)–6\)", r" {@recharge \1}", name)
    return re.sub(r"\s*\(Recharge 6\)", " {@recharge}", name)


def spellcasting(e, key, names):
    """`Spellcasting. The mage casts … (spell save DC 14): At Will: …
    2/Day Each: …`: the data format's spellcasting block."""
    text = " ".join(t for t in e["entries"] if isinstance(t, str))
    rest = [t for t in e["entries"][1:] if not isinstance(t, str)]
    # The text before tagging: from the entry, untagged again by the lists.
    parts = re.split(r"(At Will:|\d+/Day(?: Each)?:)", text)
    header = parts[0].strip()
    out = {"name": e["name"], "type": "spellcasting", "headerEntries": [header]}
    for label, spells in zip(parts[1::2], parts[2::2]):
        listed = [s.strip() for s in _split(spells.strip().rstrip("."))]
        if label == "At Will:":
            out["will"] = listed
        else:
            m = re.match(r"(\d+)/Day( Each)?:", label)
            out.setdefault("daily", {})[m.group(1) + ("e" if m.group(2) else "")] = listed
    if rest:
        out["footerEntries"] = rest
    if m := re.search(r"using (\w+) as the spellcasting ability", header):
        out["ability"] = FULL[m.group(1)]
    out["displayAs"] = key
    return out
