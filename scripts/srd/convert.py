"""Convert the System Reference Document 5.2.1 (CC-BY-4.0) into a data
folder in dnt's data format.

    python3 scripts/srd/convert.py SRD_CC_v5.2.1.pdf srd/

The PDF is at https://www.dndbeyond.com/srd. Needs poppler's pdftohtml
and pdftotext.
"""

import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))

import backgrounds  # noqa: E402
import classes  # noqa: E402
import feats  # noqa: E402
import items  # noqa: E402
import options  # noqa: E402
import pdf  # noqa: E402
import species  # noqa: E402
import spells  # noqa: E402
import tags  # noqa: E402
from tags import SOURCE  # noqa: E402

# Where each part of the SRD is (its pages).
PAGES = {"classes": (28, 82), "origins": (83, 86), "feats": (87, 88), "equipment": (89, 101), "spells": (107, 175)}

BOOK = {
    "name": "System Reference Document 5.2.1",
    "id": SOURCE,
    "source": SOURCE,
    "group": "core",
    "published": "2025-04-22",
    "author": "Wizards of the Coast",
}


def write(path, data):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        json.dump(data, f, indent=1, ensure_ascii=False)
        f.write("\n")


def unknown_items(classes, backgrounds, known):
    """The items starting equipment names that the data does not have."""
    names = {i["name"].lower() for i in known}
    refs = []
    for c in classes:
        refs += [e for g in c["startingEquipment"]["defaultData"] for alt in g.values() for e in alt]
    for b in backgrounds:
        refs += [e for g in b["startingEquipment"] for alt in g.values() for e in alt]
    wanted = {(e if isinstance(e, str) else e.get("item", "")).split("|")[0] for e in refs}
    return {w for w in wanted if w and w not in names}


def main(pdf_path, out):
    write(f"{out}/books.json", {"book": [BOOK]})
    found = spells.spells(pdf.lines(pdf_path, *PAGES["spells"]), pdf_path)
    write(f"{out}/spells/spells-srd52.json", {"spell": [s for s, _ in found]})
    write(f"{out}/spells/index.json", {SOURCE: "spells-srd52.json"})
    lists = {s["name"]: {"class": [{"name": c, "source": SOURCE} for c in classes]} for s, classes in found}
    write(f"{out}/spells/sources.json", {SOURCE: lists})
    names = {s["name"].lower() for s, _ in found}
    found, lists = classes.classes(pdf.lines(pdf_path, *PAGES["classes"]), pdf_path)
    picks = options.options(lists, pdf_path)
    classes.finish(found, {"spell": names, "optionalfeature": {o["name"].lower() for o in picks}})
    for c in found["class"]:
        options.progressions(c)
    picks = [dict(o, entries=tags.map_text(o["entries"], lambda t: tags.tag(t, {"spell": names}))) for o in picks]
    write(f"{out}/optionalfeatures.json", {"optionalfeature": picks})
    index = {}
    for c in found["class"]:
        slug = c["name"].lower()
        doc = {"class": [c]}
        for kind in ("subclass", "classFeature", "subclassFeature"):
            doc[kind] = [e for e in found[kind] if e["className"] == c["name"]]
        write(f"{out}/class/class-{slug}.json", doc)
        index[slug] = f"class-{slug}.json"
    write(f"{out}/class/index.json", index)
    origins = pdf.lines(pdf_path, *PAGES["origins"])
    known = {"spell": names}
    tagged = lambda es: tags.map_text(es, lambda t: tags.tag(t, known))
    found_feats = [dict(f, entries=tagged(f["entries"])) for f in feats.feats(pdf.lines(pdf_path, *PAGES["feats"]), pdf_path)]
    write(f"{out}/feats.json", {"feat": found_feats})
    write(f"{out}/backgrounds.json", {"background": backgrounds.backgrounds(origins)})
    races = [dict(r, entries=tagged(r["entries"])) for r in species.species(origins, pdf_path)]
    write(f"{out}/races.json", {"race": races})
    print(f"{len(found_feats)} feats, {len(races)} species")
    gear_lines = pdf.lines(pdf_path, *PAGES["equipment"])
    base = items.weapons(gear_lines, pdf_path) + items.armor(gear_lines, pdf_path)
    write(f"{out}/items-base.json", {"baseitem": base, "itemMastery": items.masteries(gear_lines, pdf_path)})
    gear, tools = items.gear(gear_lines, pdf_path)
    things = [dict(i, entries=tagged(i["entries"])) if "entries" in i else i for i in gear + tools + items.mounts(gear_lines, pdf_path)]
    write(f"{out}/items.json", {"item": things})
    print(f"{len(base)} weapons and armor, {len(things)} items")
    missing = unknown_items(found["class"], [json.load(open(f"{out}/backgrounds.json"))["background"]][0], base + things)
    if missing:
        print("items named but not in the data:", ", ".join(sorted(missing)))
    print(f"{len(names)} spells, {len(found['class'])} classes, {len(picks)} options")


if __name__ == "__main__":
    main(*sys.argv[1:3])
