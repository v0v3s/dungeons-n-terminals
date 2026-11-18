// A text cut short with an ellipsis has its full text as a tooltip: set on
// the way in (the pointer over it), so it follows text that changes.
const MARK = "dntAutoTitle";

/// The element whose text is cut short, from the one under the pointer up.
function cut(start: Element | null): HTMLElement | null {
  for (let el = start, depth = 0; el instanceof HTMLElement && depth < 4; el = el.parentElement, depth++) {
    if (getComputedStyle(el).textOverflow === "ellipsis") return el;
  }
  return null;
}

function over(e: MouseEvent) {
  const el = cut(e.target as Element | null);
  if (!el) return;
  const short = el.scrollWidth > el.clientWidth + 1;
  if (short && (!el.title || el.dataset[MARK])) {
    el.title = el.textContent?.trim().replace(/\s+/g, " ") ?? "";
    el.dataset[MARK] = "1";
  } else if (!short && el.dataset[MARK]) {
    el.removeAttribute("title");
    delete el.dataset[MARK];
  }
}

/// Listen on the whole page; returns the way to stop.
export function fullTextTooltips(): () => void {
  document.addEventListener("mouseover", over);
  return () => document.removeEventListener("mouseover", over);
}
