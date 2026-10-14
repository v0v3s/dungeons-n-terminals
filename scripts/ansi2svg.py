"""Render a `tmux capture-pane -e -p` dump (ANSI SGR colours) to SVG, for
checking how the TUI looks. Convert to PNG with `rsvg-convert`.

usage: python3 -I scripts/ansi2svg.py capture.txt out.svg

The palette approximates Tokyo Night; real colours follow the terminal theme.
"""
import re
import sys
from html import escape

PALETTE = [
    "#15161e", "#f7768e", "#9ece6a", "#e0af68", "#7aa2f7", "#bb9af7", "#7dcfff", "#a9b1d6",
    "#414868", "#f7768e", "#9ece6a", "#e0af68", "#7aa2f7", "#bb9af7", "#7dcfff", "#c0caf5",
]
BG, FG = "#1a1b26", "#c0caf5"
CW, CH = 9, 19


def xterm256(n):
    if n < 16:
        return PALETTE[n]
    if n < 232:
        n -= 16
        r, g, b = n // 36, (n // 6) % 6, n % 6
        f = lambda v: 0 if v == 0 else 55 + v * 40
        return "#%02x%02x%02x" % (f(r), f(g), f(b))
    v = 8 + (n - 232) * 10
    return "#%02x%02x%02x" % (v, v, v)


def fresh():
    return dict(fg=None, bg=None, bold=False, dim=False, it=False, ul=False, rev=False, st=False)


def apply(st, codes):
    i = 0
    while i < len(codes):
        c = codes[i]
        if c == 0:
            st.update(fresh())
        elif c in (1, 2, 3, 4, 7, 9):
            st[{1: "bold", 2: "dim", 3: "it", 4: "ul", 7: "rev", 9: "st"}[c]] = True
        elif c == 22:
            st["bold"] = st["dim"] = False
        elif c in (23, 24, 27, 29):
            st[{23: "it", 24: "ul", 27: "rev", 29: "st"}[c]] = False
        elif 30 <= c <= 37:
            st["fg"] = PALETTE[c - 30]
        elif 90 <= c <= 97:
            st["fg"] = PALETTE[c - 90 + 8]
        elif 40 <= c <= 47:
            st["bg"] = PALETTE[c - 40]
        elif 100 <= c <= 107:
            st["bg"] = PALETTE[c - 100 + 8]
        elif c == 39:
            st["fg"] = None
        elif c == 49:
            st["bg"] = None
        elif c in (38, 48) and i + 2 < len(codes) and codes[i + 1] == 5:
            st["fg" if c == 38 else "bg"] = xterm256(codes[i + 2])
            i += 2
        elif c in (38, 48) and i + 4 < len(codes) and codes[i + 1] == 2:
            st["fg" if c == 38 else "bg"] = "#%02x%02x%02x" % tuple(codes[i + 2:i + 5])
            i += 4
        i += 1


def main(src, dst):
    rows = []
    for raw in open(src, encoding="utf-8").read().split("\n"):
        st, cells = fresh(), []
        for part in re.split(r"(\x1b\[[0-9;]*m)", raw):
            m = re.fullmatch(r"\x1b\[([0-9;]*)m", part)
            if m:
                apply(st, [int(c) if c else 0 for c in m.group(1).split(";")])
            else:
                cells.extend((ch, dict(st)) for ch in part)
        rows.append(cells)
    width = max((len(r) for r in rows), default=80)
    out = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{width * CW}" height="{len(rows) * CH}" '
           f'font-family="JetBrains Mono" font-size="15">',
           f'<rect width="100%" height="100%" fill="{BG}"/>']
    for y, cells in enumerate(rows):
        for x, (ch, s) in enumerate(cells):
            fg, bg = s["fg"] or FG, s["bg"]
            if s["rev"]:
                fg, bg = (bg or BG), (s["fg"] or FG)
            if bg:
                out.append(f'<rect x="{x * CW}" y="{y * CH}" width="{CW + 0.5}" height="{CH}" fill="{bg}"/>')
            if ch.strip():
                attrs = f'fill="{fg}"'
                if s["bold"]:
                    attrs += ' font-weight="bold"'
                if s["it"]:
                    attrs += ' font-style="italic"'
                if s["dim"]:
                    attrs += ' opacity="0.6"'
                deco = [d for d, on in (("underline", s["ul"]), ("line-through", s["st"])) if on]
                if deco:
                    attrs += f' text-decoration="{" ".join(deco)}"'
                out.append(f'<text x="{x * CW}" y="{y * CH + 14}" {attrs}>{escape(ch)}</text>')
    out.append("</svg>")
    open(dst, "w", encoding="utf-8").write("\n".join(out))


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
