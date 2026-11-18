// A small Markdown renderer for notes: frontmatter, headings, lists (with
// tasks, "- [ ]" and "- [x]", as checkboxes), quotes, paragraphs,
// **bold**, *italic*, `code` and [[links]] (rendered as anchors with
// data-link, handled by the page). And the editor's list keys.

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
  // Comments are notes to the writer; ```dnt blocks show their results
  // below the note instead. Each hidden part keeps its line breaks, so a
  // line here is the same line in the text (a task's checkbox names it).
  const blank = (m: string) => "\n".repeat(m.split("\n").length - 1);
  const shown = text
    .replace(/\r/g, "")
    .replace(/<!--[\s\S]*?-->/g, blank)
    .replace(/^[ \t]*```dnt[^\n]*\n[\s\S]*?^[ \t]*```[ \t]*$/gm, blank);
  const lines = shown.split("\n");
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
  /// List items: their text, and for a task whether it is done and its line.
  let list: { text: string; task?: { done: boolean; line: number } }[] = [];
  const item = (l: (typeof list)[number]) =>
    l.task
      ? `<li class="task"><label><input type="checkbox" class="task" data-line="${l.task.line}"${l.task.done ? " checked" : ""} /> <span>${inline(l.text)}</span></label></li>`
      : `<li>${inline(l.text)}</li>`;
  const flush = () => {
    if (para.length) out.push(`<p>${para.map(inline).join(" ")}</p>`);
    if (list.length) out.push(`<ul${list.some((l) => l.task) ? ' class="tasks"' : ""}>${list.map(item).join("")}</ul>`);
    para = [];
    list = [];
  };
  for (; i < lines.length; i++) {
    const l = lines[i];
    const h = l.match(/^(#{1,6})\s+(.*)$/);
    const li = l.match(/^\s*(?:[-*+]|\d+\.)\s+(?:\[( |x|X)\]\s+)?(.*)$/);
    if (h) {
      flush();
      out.push(`<h${Math.min(h[1].length + 2, 6)} class="h">${inline(h[2])}</h${Math.min(h[1].length + 2, 6)}>`);
    } else if (li) {
      if (para.length) flush();
      list.push(li[1] ? { text: li[2], task: { done: li[1] !== " ", line: i } } : { text: li[2] });
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

/// The text with the task on this line ticked or not.
export function setTask(text: string, line: number, done: boolean): string {
  const lines = text.split("\n");
  const l = lines[line];
  if (l === undefined) return text;
  lines[line] = l.replace(/^(\s*(?:[-*+]|\d+\.)\s+)\[( |x|X)\]/, `$1[${done ? "x" : " "}]`);
  return lines.join("\n");
}

/// What Enter does at the end of a list item in the editor: the next item
/// ("- ", "2. ", "- [ ] "), or on an empty item the list's end (its bullet
/// taken away). `null`: an ordinary line break.
export function listEnter(text: string, caret: number): { from: number; to: number; insert: string } | null {
  const start = text.lastIndexOf("\n", caret - 1) + 1;
  const end = text.indexOf("\n", caret);
  // Only at the end of the line.
  if (text.slice(caret, end < 0 ? text.length : end).trim()) return null;
  const line = text.slice(start, caret);
  const m = line.match(/^(\s*)([-*+]|(\d+)\.)(\s+)(\[( |x|X)\]\s+)?(.*)$/);
  if (!m) return null;
  const [, indent, bullet, number, space, task, , rest] = m;
  if (!rest.trim()) return { from: start, to: caret, insert: indent };
  const next = number ? `${Number(number) + 1}.` : bullet;
  return { from: caret, to: caret, insert: `\n${indent}${next}${space}${task ? "[ ] " : ""}` };
}
