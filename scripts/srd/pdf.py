"""The SRD's pages as lines of styled text, in reading order.

`pdftohtml -xml` (poppler) gives every line's position, font and bold or
italic runs; here they become `Line`s: one per row of a column, with its
runs, sorted column by column. Running headers and page numbers are left
out.
"""

import re
import subprocess
import xml.etree.ElementTree as ET
from dataclasses import dataclass, field


@dataclass
class Run:
    text: str
    bold: bool = False
    italic: bool = False


@dataclass
class Line:
    page: int
    column: int
    top: int
    left: int
    size: int
    family: str
    color: str
    runs: list = field(default_factory=list)
    # The row's pieces, as laid out: (left, text), for tables.
    cells: list = field(default_factory=list)
    # Where the page's two columns meet.
    middle: float = 0
    right: int = 0

    @property
    def text(self) -> str:
        return "".join(r.text for r in self.runs)

    @property
    def marked(self) -> str:
        """The text with italic runs as `_italic_` (names of spells, items:
        the tagger looks them up)."""
        out = ""
        for r in self.runs:
            if r.italic and not r.bold and r.text.strip():
                lead = r.text[: len(r.text) - len(r.text.lstrip())]
                trail = r.text[len(r.text.rstrip()) :]
                out += f"{lead}_{r.text.strip()}_{trail}"
            else:
                out += r.text
        return re.sub(r"_(\s*)_", r"\1", out)

    @property
    def sans(self) -> bool:
        return "GillSans" in self.family

    @property
    def bold(self) -> bool:
        """Bold all through (a heading or a label)."""
        return all(r.bold or not r.text.strip() for r in self.runs)

    @property
    def italic(self) -> bool:
        return all(r.italic or not r.text.strip() for r in self.runs)

    def __repr__(self) -> str:
        style = f"{self.size}{'s' if self.sans else ''}{' ' + self.color if self.color != '#231f20' else ''}"
        return f"<{self.page}:{self.column}:{self.top} {style} {self.text!r}>"


# What is not the text: page numbers and the running header, in grey.
GREY = "#7b7879"


def lines(pdf: str, first: int, last: int) -> list:
    xml = subprocess.run(
        ["pdftohtml", "-xml", "-i", "-q", "-f", str(first), "-l", str(last), "-stdout", pdf],
        check=True,
        capture_output=True,
    ).stdout.decode("utf-8")
    # The XML can carry stray control characters.
    xml = re.sub(r"[\x00-\x08\x0b\x0c\x0e-\x1f]", "", xml)
    root = ET.fromstring(xml)
    fonts = {}
    out = []
    for page in root.iter("page"):
        number = int(page.get("number"))
        middle = int(page.get("width")) / 2
        for spec in page.iter("fontspec"):
            fonts[spec.get("id")] = (int(spec.get("size")), spec.get("family"), spec.get("color"))
        rows = []
        for t in page.iter("text"):
            size, family, color = fonts[t.get("font")]
            if color == GREY:
                continue
            top, left = int(t.get("top")), int(t.get("left"))
            runs = list(_runs(t, "SemiBold" in family or "Bold" in family))
            if not "".join(r.text for r in runs).strip():
                continue
            column = 0 if left < middle - 20 else 1
            text = "".join(r.text for r in runs)
            rows.append(Line(number, column, top, left, size, family, color, runs, [(left, text)], middle - 20, left + int(t.get("width"))))
        # A table across the page: its right half is not the right
        # column's text.
        cell = lambda l: l.sans and l.size <= 14
        wide = [o.top for o in rows if o.column == 0 and cell(o) and o.right > middle]
        across = {l.top for l in rows if l.column == 1 and cell(l) and any(abs(t - l.top) <= 3 for t in wide)}
        # Its header too: a row beside the left half, next to such rows.
        heads = [o.top for o in rows if o.column == 0 and cell(o) and o.bold]
        for l in rows:
            if l.column == 1 and cell(l) and l.bold and any(abs(t - l.top) <= 3 for t in heads) and any(0 < t - l.top <= 60 for t in across):
                across.add(l.top)
        for l in rows:
            if l.column == 1 and cell(l) and any(abs(t - l.top) <= 3 for t in across):
                l.column = 0
        # Pieces of one row (a label and its value) become one line.
        rows.sort(key=lambda l: (l.column, l.top, l.left))
        groups = []
        for l in rows:
            prev = groups[-1][0] if groups else None
            if prev and prev.column == l.column and abs(prev.top - l.top) <= 3:
                groups[-1].append(l)
            else:
                groups.append([l])
        for g in groups:
            # Left to right: a piece can sit a pixel or two higher.
            g.sort(key=lambda l: l.left)
            first = min(g, key=lambda l: l.top)
            line = Line(number, first.column, first.top, g[0].left, g[0].size, g[0].family, g[0].color, [], [], middle - 20)
            end = None
            for l in g:
                # Pieces apart on the row: words apart.
                apart = end is not None and l.left - end > 2
                if apart and not line.runs[-1].text[-1:].isspace() and not l.runs[0].text[:1].isspace():
                    line.runs.append(Run(" ", line.runs[-1].bold, line.runs[-1].italic))
                line.runs.extend(l.runs)
                line.cells.extend(l.cells)
                end = l.right
            line.right = max(l.right for l in g)
            out.append(line)
    return out


def _runs(t, bold_font: bool):
    """A <text>'s runs, with <b> and <i> (nested either way)."""

    def walk(e, bold, italic):
        bold = bold or e.tag == "b"
        italic = italic or e.tag == "i"
        if e.text:
            yield Run(e.text.replace("\xad", "").replace("\u200b", ""), bold, italic)
        for child in e:
            yield from walk(child, bold, italic)
            if child.tail:
                yield Run(child.tail.replace("\xad", "").replace("\u200b", ""), bold, italic)

    yield from walk(t, bold_font, False)


def join(prev: str, line: str) -> str:
    """Join a line onto the text before it, undoing hyphenation."""
    prev = prev.rstrip()
    line = line.strip()
    if not prev:
        return line
    # `Wis-` + `dom`; in italics, `_Poly-_` + `_morph_`.
    m = re.search(r"[a-z]-(_?)$", prev)
    word = re.search(r"([\w-]+)-_?$", prev)
    if m and word and _real_hyphen(word.group(1)):
        return prev + line
    if m and re.match(r"_?[a-z]", line):
        if m.group(1) and line.startswith("_"):
            return prev[:-2] + line[1:]
        return prev[: m.start() + 1] + line
    if prev.endswith("—"):
        return prev + line
    return prev + " " + line


# Words whose hyphen at a line's end is part of the word: `20-foot-radius`,
# `self-evident`.
_PREFIXES = {"self", "half", "well", "one", "two", "three", "four", "five", "ten", "high", "low", "long", "short", "full", "semi", "spell", "cross"}


def _real_hyphen(word: str) -> bool:
    return "-" in word or any(c.isdigit() for c in word) or word.lower() in _PREFIXES
