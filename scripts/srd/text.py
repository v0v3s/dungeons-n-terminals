"""Lines of body text as the data format's `entries`: paragraphs, run-in
named entries (`Part Water. You part water…`), bullet lists and tables.
"""

import re
import subprocess
from functools import lru_cache

from pdf import join

# A line this much below the last one starts a new paragraph.
GAP = 26


def is_table_line(line) -> bool:
    """Tables are set in the sans font; body text in the serif one."""
    return line.sans


def entries(lines, pdf_path=None) -> list:
    """Body lines (in order) as entries."""
    out = []
    para = None  # the paragraph being built: [name or None, text]
    prev = None
    i = 0
    while i < len(lines):
        l = lines[i]
        if is_table_line(l):
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
    return _lists(out)


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
                    part.append(_with_cells(l, cells))
            out.append(_table(title.strip(), part, pdf_path))
    return out


def _with_cells(line, cells):
    from copy import copy

    l = copy(line)
    l.cells = cells
    return l


def _table(title, lines, pdf_path):
    if not lines:
        return {"type": "table", "caption": title, "colLabels": [], "rows": []} if title else {}
    header, rows = None, []
    for l in lines:
        if header is None and l.bold:
            header = l
        else:
            rows.append(l)
    width = max((len(r.cells) for r in rows), default=1)
    # Where each column starts: from the fullest row.
    starts = next((sorted(c[0] for c in r.cells) for r in rows if len(r.cells) == width), [])
    labels = _header(header, width, starts, pdf_path) if header else []
    body = []
    for r in rows:
        cells = sorted(r.cells)
        if len(cells) < width and body and cells and cells[0][0] > starts[0] + 10:
            # A row's text going on from the row before.
            for left, text in cells:
                col = max(k for k, s in enumerate(starts) if left >= s - 10)
                body[-1][col] = join(body[-1][col], text)
            continue
        row = [""] * width
        for left, text in cells:
            col = max([k for k, s in enumerate(starts) if left >= s - 12] or [0])
            row[col] = join(row[col], text)
        body.append(row)
    table = {"type": "table"}
    if title:
        table["caption"] = title
    if labels:
        table["colLabels"] = labels
    table["rows"] = [[c.strip() for c in row] for row in body]
    return table


def _header(line, width, starts, pdf_path):
    cells = sorted(line.cells)
    if len(cells) == width or not pdf_path:
        return [c[1].strip() for c in cells]
    # Merged into fewer pieces: split by where each word is.
    words = _words(pdf_path, line.page)
    labels = [""] * width
    for x, y, w in words:
        if abs(y - line.top) <= 6 and line.cells[0][0] - 5 <= x:
            last = max(c[0] + len(c[1]) * 9 for c in line.cells)
            if x > last:
                continue
            col = max([k for k, s in enumerate(starts) if x >= s - 30] or [0])
            labels[col] = (labels[col] + " " + w).strip()
    return labels


@lru_cache(maxsize=None)
def _words(pdf_path, page):
    """Every word on a page with where it is (pdftotext -bbox)."""
    html = subprocess.run(
        ["pdftotext", "-bbox", "-f", str(page), "-l", str(page), pdf_path, "-"], check=True, capture_output=True
    ).stdout.decode("utf-8")
    out = []
    # pdftotext's points against pdftohtml's pixels: 1.5 apart.
    for m in re.finditer(r'<word xMin="([\d.]+)" yMin="([\d.]+)"[^>]*>([^<]*)</word>', html):
        out.append((float(m.group(1)) * 1.5, float(m.group(2)) * 1.5, m.group(3)))
    return tuple(out)
