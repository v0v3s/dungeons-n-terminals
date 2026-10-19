// A small Markdown renderer for notes: frontmatter, headings, lists,
// quotes, paragraphs, **bold**, *italic*, `code` and [[links]] (rendered
// as anchors with data-link, handled by the page).

const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

function inline(text: string): string {
  let s = esc(text);
  s = s.replace(/\[\[([^\]|]+)(?:\|([^\]]+))?\]\]/g, (_, target: string, alias?: string) => {
    const t = target.trim();
    const compendium = /^[a-z]+:/i.test(t);
    const label = alias ?? (compendium ? t.split(":").slice(1).join(":").split("@")[0] : t);
    return `<a href="#" class="${compendium ? "wl wl-c" : "wl"}" data-link="${t}">${label}</a>`;
  });
  s = s.replace(/`([^`]+)`/g, "<code>$1</code>");
  s = s.replace(/\*\*([^*]+)\*\*/g, "<b>$1</b>");
  s = s.replace(/(^|[^*])\*([^*]+)\*/g, "$1<i>$2</i>");
  s = s.replace(/(^|\s)#([\w-]+)/g, '$1<span class="tag">#$2</span>');
  return s;
}

export function render(text: string): string {
  const lines = text.replace(/\r/g, "").split("\n");
  const out: string[] = [];
  let i = 0;
  // Frontmatter as a dim property list.
  if (lines[0] === "---") {
    const end = lines.indexOf("---", 1);
    if (end > 0) {
      out.push('<dl class="fm">');
      for (const l of lines.slice(1, end)) {
        const [k, ...v] = l.split(":");
        if (k.trim()) out.push(`<dt>${esc(k.trim())}</dt><dd>${inline(v.join(":").trim())}</dd>`);
      }
      out.push("</dl>");
      i = end + 1;
    }
  }
  let para: string[] = [];
  let list: string[] = [];
  const flush = () => {
    if (para.length) out.push(`<p>${para.map(inline).join(" ")}</p>`);
    if (list.length) out.push(`<ul>${list.map((l) => `<li>${inline(l)}</li>`).join("")}</ul>`);
    para = [];
    list = [];
  };
  for (; i < lines.length; i++) {
    const l = lines[i];
    const h = l.match(/^(#{1,6})\s+(.*)$/);
    const li = l.match(/^\s*(?:[-*+]|\d+\.)\s+(?:\[( |x)\]\s+)?(.*)$/);
    if (h) {
      flush();
      out.push(`<h${Math.min(h[1].length + 2, 6)} class="h">${inline(h[2])}</h${Math.min(h[1].length + 2, 6)}>`);
    } else if (li) {
      if (para.length) flush();
      list.push((li[1] === "x" ? "☑ " : li[1] === " " ? "☐ " : "") + li[2]);
    } else if (l.startsWith(">")) {
      flush();
      out.push(`<blockquote>${inline(l.replace(/^>\s?/, ""))}</blockquote>`);
    } else if (/^```/.test(l)) {
      flush();
      const body: string[] = [];
      for (i++; i < lines.length && !/^```/.test(lines[i]); i++) body.push(lines[i]);
      out.push(`<pre>${esc(body.join("\n"))}</pre>`);
    } else if (!l.trim()) {
      flush();
    } else {
      if (list.length) flush();
      para.push(l);
    }
  }
  flush();
  return out.join("\n");
}
