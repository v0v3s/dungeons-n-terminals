"""Lines of body text as the data format's `entries`: paragraphs, run-in
named entries (`Part Water. You part water…`), bullet lists and tables.
"""

import re
import subprocess
from functools import lru_cache
from html import unescape

from pdf import join

# A line this much below the last one starts a new paragraph.
GAP = 26


def is_table_line(line) -> bool:
    """Tables are set in the sans font; body text in the serif one."""
    return line.sans


def entries(lines, pdf_path=None, prose=False) -> list:
    """Body lines (in order) as entries. `prose`: sans lines are text (a
    sidebar's), not tables."""
    out = []
    para = None  # the paragraph being built: [name or None, text]
    prev = None
    i = 0
    while i < len(lines):
        l = lines[i]
        if l.sans and l.size == 17 and pdf_path:
            # A sidebar: its title in small capitals, then its text.
            j = i + 1
            while j < len(lines) and lines[j].sans and lines[j].size == 12:
                j += 1
            k = j
            while k < len(lines) and lines[k].sans and lines[k].size == 14:
                k += 1
            _flush(out, para)
            para, prev = None, None
            out.append({"type": "inset", "name": title_words(l, pdf_path), "entries": entries(lines[j:k], pdf_path, prose=True)})
            i = k
            continue
        if is_table_line(l) and not prose:
            j = i
            while j < len(lines) and is_table_line(lines[j]):
                j += 1
            _flush(out, para)
            para = None
            out.extend(tables(lines[i:j], pdf_path))
            prev = None
            i = j
            continue
        text = l.marked
        starts = (
            para is None
            or text.startswith("  ")
            or text.lstrip().startswith("•")
            or (prev is not None and prev.page == l.page and prev.column == l.column and l.top - prev.top > GAP)
            or _runin(l) is not None and _ends_sentence(para[1])
        )
        if starts:
            _flush(out, para)
            name, rest = _runin(l) or (None, text)
            para = [name, rest.strip()]
        else:
            para[1] = join(para[1], text)
        prev = l
        i += 1
    _flush(out, para)
    return _nest(_lists(out))


def _nest(out):
    """Paragraphs, lists and tables after a named entry are its own, up
    to the next named entry (or a sidebar)."""
    result = []
    for e in out:
        named = isinstance(e, dict) and e.get("type") == "entries"
        inset = isinstance(e, dict) and e.get("type") == "inset"
        if not named and not inset and result and isinstance(result[-1], dict) and result[-1].get("type") == "entries":
            result[-1]["entries"].append(e)
        else:
            result.append(e)
    return result


def _ends_sentence(text: str) -> bool:
    return bool(re.search(r"[.:!?”)]$", text.rstrip()))


def _runin(line):
    """A paragraph's bold or bold-italic name: (`Part Water`, the rest)."""
    runs = line.runs
    name = ""
    k = 0
    while k < len(runs) and (runs[k].bold or not runs[k].text.strip()):
        name += runs[k].text
        k += 1
    name = name.strip()
    if not name or not name.endswith(".") or k == 0:
        return None
    rest = "".join(f"_{r.text}_" if r.italic and r.text.strip() else r.text for r in runs[k:])
    return name[:-1].strip(), rest


def _flush(out, para):
    if para is None:
        return
    name, text = para
    text = re.sub(r"_(\s*)_", r"\1", text).strip()
    if name:
        out.append({"type": "entries", "name": name, "entries": [text] if text else []})
    elif text:
        out.append(text)


def _lists(out):
    """Paragraphs starting with a bullet become a list."""
    result = []
    for e in out:
        if isinstance(e, str) and e.startswith("•"):
            item = e.lstrip("• ").strip()
            if result and isinstance(result[-1], dict) and result[-1].get("type") == "list":
                result[-1]["items"].append(item)
            else:
                result.append({"type": "list", "items": [item]})
        else:
            result.append(e)
    return result


# ---- tables ----


def tables(lines, pdf_path=None) -> list:
    """Sans lines as tables: a bold title (or none), a bold header row,
    rows. Tables side by side (one title per table on the first row)
    are split by where their titles start."""
    if not lines:
        return []
    groups = []
    # Titles: a line larger than the rows (16 against 14).
    current = None
    for l in lines:
        if l.size >= 16 and l.bold:
            current = {"titles": l.cells, "lines": []}
            groups.append(current)
        else:
            if current is None:
                current = {"titles": [], "lines": []}
                groups.append(current)
            current["lines"].append(l)
    out = []
    for g in groups:
        titles = g["titles"]
        if len(titles) <= 1:
            out.append(_table(titles[0][1].strip() if titles else None, g["lines"], pdf_path))
            continue
        bounds = [t[0] for t in titles] + [10_000]
        for k, (left, title) in enumerate(titles):
            part = []
            for l in g["lines"]:
                cells = [c for c in l.cells if bounds[k] - 15 <= c[0] < bounds[k + 1] - 15]
                if cells:
                    part.append(_with_cells(l, cells, (bounds[k] - 15, bounds[k + 1] - 15)))
            out.append(_table(title.strip(), part, pdf_path))
    return out


def _with_cells(line, cells, bounds):
    from copy import copy

    l = copy(line)
    l.cells = cells
    l.bounds = bounds
    return l


def _table(title, lines, pdf_path):
    if not lines:
        return {"type": "table", "caption": title, "colLabels": [], "rows": []} if title else {}
    # The header: the bold lines before the first row (one label can go
    # on over two lines).
    header, rows = [], []
    for l in lines:
        if not rows and l.bold:
            header.append(l)
        else:
            rows.append(l)
    words = {id(l): _line_words(l, pdf_path) for l in lines}
    # The columns: where the phrases of the header's fullest line start,
    # or of the fullest row when there is no header.
    if header:
        starts = [p[0] for p in max((_phrases(words[id(h)]) for h in header), key=len)]
    else:
        starts = [p[0] for p in max((_phrases(words[id(r)]) for r in rows), key=len)]
    col = lambda x: max([k for k, s in enumerate(starts) if x >= s - 12] or [0])
    labels = [""] * len(starts)
    for h in header:
        for x0, _, w in _phrases(words[id(h)]):
            k = min(range(len(starts)), key=lambda k: abs(starts[k] - x0))
            labels[k] = (labels[k] + " " + w).strip()
    body = []
    for r in rows:
        ws = words[id(r)]
        row = [""] * len(starts)
        for x0, _, w in ws:
            row[col(x0)] = (row[col(x0)] + " " + w).strip()
        if body and ws and col(ws[0][0]) > 0 and not row[0]:
            # A row's text going on from the row before.
            body[-1] = [join(a, b) if b else a for a, b in zip(body[-1], row)]
        else:
            body.append(row)
    table = {"type": "table"}
    if title:
        table["caption"] = title
    if header:
        table["colLabels"] = labels
    table["rows"] = [[re.sub(r"\s+([,;.])", r"\1", c).strip() for c in row] for row in body]
    return table


def _line_words(line, pdf_path):
    """A line's words, left to right: (left, right, word), from its
    column of the page."""
    left, right = (0, line.middle) if line.column == 0 else (line.middle, 10_000)
    if line.right > line.middle + 20:
        right = 10_000  # a row across the page
    # One of tables set side by side: its part of the column.
    if getattr(line, "bounds", None):
        left, right = max(left, line.bounds[0]), min(right, line.bounds[1])
    out = [(x0, x1, w) for x0, x1, y, w in boxes(pdf_path, line.page) if abs(y - line.top) <= 6 and left <= x0 < right]
    return sorted(out)


def _phrases(words, gap=10):
    """Words close together as one phrase: (left, right, text)."""
    out = []
    for x0, x1, w in words:
        if out and x0 - out[-1][1] < gap:
            out[-1] = (out[-1][0], x1, out[-1][2] + " " + w)
        else:
            out.append((x0, x1, w))
    return out


def title_words(line, pdf_path):
    """A title set in small capitals (`B` + `reaking`): its words."""
    ws = [(x0, x1, w) for x0, x1, y, w in boxes(pdf_path, line.page) if -4 <= y - line.top <= 12 and ((x0 < line.middle) == (line.column == 0))]
    out = ""
    last = None
    for x0, x1, w in sorted(ws):
        out += w if last is not None and x0 - last < 3 else " " + w
        last = x1
    small = {"a", "an", "and", "as", "at", "by", "for", "in", "of", "on", "or", "the", "to", "with"}
    words = out.strip().lower().split()
    return " ".join(w if k and w in small else w[:1].upper() + w[1:] for k, w in enumerate(words))


@lru_cache(maxsize=None)
def boxes(pdf_path, page):
    """Every word on a page: (left, right, top, word), in pdftohtml's
    pixels."""
    html = subprocess.run(
        ["pdftotext", "-bbox", "-f", str(page), "-l", str(page), pdf_path, "-"], check=True, capture_output=True
    ).stdout.decode("utf-8")
    box = r'<word xMin="([\d.]+)" yMin="([\d.]+)" xMax="([\d.]+)" yMax="[\d.]+">([^<]*)</word>'
    # pdftotext's points against pdftohtml's pixels: 1.5 apart.
    return tuple((float(a) * 1.5, float(c) * 1.5, float(b) * 1.5, unescape(w)) for a, b, c, w in re.findall(box, html))
