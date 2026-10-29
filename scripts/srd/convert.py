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

import classes  # noqa: E402
import options  # noqa: E402
import pdf  # noqa: E402
import spells  # noqa: E402
import tags  # noqa: E402
from tags import SOURCE  # noqa: E402

# Where each part of the SRD is (its pages).
PAGES = {"classes": (28, 82), "spells": (107, 175)}

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
    print(f"{len(names)} spells, {len(found['class'])} classes, {len(picks)} options")


if __name__ == "__main__":
    main(*sys.argv[1:3])
