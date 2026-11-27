<!-- Notes: the campaign's folder tree and a note, read (with its links) or
     edited. On a narrow screen the note replaces the tree. -->
<script lang="ts">
  import { onMount, tick } from "svelte";
  import * as api from "./api";
  import { sidebar } from "./sidebar.svelte";
  import { desktop } from "./screen";
  import EntryModal from "./EntryModal.svelte";
  import Modal from "./Modal.svelte";
  import Pane from "./Pane.svelte";
  import Prompt from "./Prompt.svelte";
  import { listEnter, render, setTask } from "./md";

  let {
    status,
    narrow = false,
    openPath = $bindable(null),
    reading = $bindable(null),
    backTo = null,
    onbackto,
  }: {
    status: api.Status;
    narrow?: boolean;
    openPath?: string | null;
    reading?: string | null;
    /// The character the note was opened from: a way back to them.
    backTo?: string | null;
    onbackto?: () => void;
  } = $props();

  let rows: api.NoteRow[] = $state([]);
  let expanded: string[] = $state([]);
  let note: api.Note | null = $state(null);
  let text = $state("");
  let editing = $state(false);
  let dirty = $state(false);
  let error = $state("");
  let message = $state("");
  let query = $state("");
  let found: [string, string][] | null = $state(null);
  /// Notes opened before (to go back to).
  let back: string[] = $state([]);

  type Dialog =
    | { kind: "prompt"; title: string; hint?: string; value?: string; then: (v: string) => void }
    | { kind: "entry"; id: number }
    | { kind: "menu"; title: string; actions: [string, () => void][] }
    | { kind: "confirm"; title: string; text: string; action: string; then: () => void };
  let dialog: Dialog | null = $state(null);
  const close = () => (dialog = null);

  $effect(() => {
    reading = narrow && note ? note.title : null;
  });

  function say(m: string) {
    message = m;
    setTimeout(() => (message = ""), 3500);
  }

  async function loadTree() {
    try {
      rows = await api.noteTree(expanded);
      // A phone or a tablet shows each top-level folder as a section: open.
      if (!desktop.current) {
        const shut = rows.filter((r) => r.depth === 0 && r.is_dir && !expanded.includes(r.path)).map((r) => r.path);
        if (shut.length) {
          expanded = [...expanded, ...shut];
          rows = await api.noteTree(expanded);
        }
      }
    } catch (e) {
      error = String(e);
    }
  }

  export async function save() {
    if (!note || !dirty) return;
    try {
      await api.noteWrite(note.path, text);
      dirty = false;
      note = { ...note, text };
    } catch (e) {
      error = String(e);
    }
  }

  /// Save, then read what was written.
  async function saveAndRead() {
    await save();
    if (!dirty) editing = false;
  }

  async function open(path: string, remember = true) {
    await save();
    try {
      if (remember && note && note.path !== path) back = [...back, note.path];
      note = await api.noteRead(path);
      selected = noteFolder(path);
      text = note.text;
      dirty = false;
      editing = false;
      // Reveal it in the tree.
      const parts = path.split("/").slice(0, -1);
      const add = parts.map((_, i) => parts.slice(0, i + 1).join("/")).filter((p) => !expanded.includes(p));
      if (add.length) {
        expanded = [...expanded, ...add];
        loadTree();
      }
    } catch (e) {
      error = String(e);
    }
  }

  /// Whether Back goes to a previous note.
  export function canGoBack(): boolean {
    return back.length > 0;
  }

  /// Back: to the previous note, then (narrow) to the tree.
  export async function stepBack(): Promise<boolean> {
    await save();
    const prev = back.at(-1);
    if (prev) {
      back = back.slice(0, -1);
      await open(prev, false);
      return true;
    }
    if (note && narrow) {
      note = null;
      return true;
    }
    return false;
  }

  function toggle(path: string) {
    expanded = expanded.includes(path) ? expanded.filter((p) => p !== path) : [...expanded, path];
    loadTree();
  }

  /// Where new notes go: the folder last tapped, or the open note's.
  let selected = $state("");
  const folder = () => selected;
  /// The open note's folder.
  const noteFolder = (path: string) => path.split("/").slice(0, -1).join("/");
  /// The folder's last part, for buttons.
  const short = (f: string) => f.split("/").at(-1) ?? f;

  /// Tap a folder: open (or close) it, and make it where new notes go.
  function tapFolder(path: string) {
    const open = expanded.includes(path);
    if (open && selected === path) {
      // Closing the chosen folder chooses its parent again.
      selected = noteFolder(path);
    } else {
      selected = path;
    }
    if (!open || selected !== path) toggle(path);
  }

  function newNote(where = folder()) {
    // A session note is named by its date: no title to ask.
    if (where.split("/")[0] === "Sessions") {
      session();
      return;
    }
    dialog = {
      kind: "prompt",
      title: where ? `New note in ${where}` : "New note",
      then: async (title) => {
        try {
          const path = await api.noteCreate(where, title);
          await loadTree();
          await open(path);
          editing = true;
        } catch (e) {
          error = String(e);
        }
      },
    };
  }

  function newFolder(parent = folder()) {
    dialog = {
      kind: "prompt",
      title: parent ? `New folder in ${parent}` : "New folder",
      then: async (name) => {
        try {
          const path = await api.noteCreateFolder(parent, name);
          if (parent && !expanded.includes(parent)) expanded = [...expanded, parent];
          selected = path;
          loadTree();
        } catch (e) {
          error = String(e);
        }
      },
    };
  }

  async function session() {
    try {
      const path = await api.noteNewSession();
      await loadTree();
      await open(path);
      editing = true;
    } catch (e) {
      error = String(e);
    }
  }

  function capture() {
    dialog = {
      kind: "prompt",
      title: "Quick note",
      hint: "Jot something down: it is added to the Inbox note with the time, to sort out later",
      then: async (t) => {
        if (!t.trim()) return;
        try {
          say(await api.noteCapture(t));
          if (note?.path === "Inbox.md") open("Inbox.md", false);
          loadTree();
        } catch (e) {
          error = String(e);
        }
      },
    };
  }

  /// A tree action's label in the toolbar: short (its full one is its
  /// accessible name and title, and contains this).
  function shortAction(label: string): string {
    if (label.startsWith("New folder")) return "New folder";
    if (label === "Today's session note") return "Session note";
    if (label.startsWith("Rename")) return "Rename";
    if (label.startsWith("Delete")) return "Delete…";
    if (label.startsWith("Move") && label.endsWith(" up")) return "↑";
    if (label.startsWith("Move") && label.endsWith(" down")) return "↓";
    if (label === "Back to the top level") return "✕";
    return label;
  }

  function treeMenu() {
    // A phone or a tablet: a folder's own menu is on its title.
    if (!desktop.current) {
      dialog = { kind: "menu", title: "Notes", actions: [["New folder", () => newFolder("")], ["Today's session note", session]] };
      return;
    }
    dialog = { kind: "menu", title: selected ? short(selected) : "Notes", actions: treeActions() };
  }

  /// A folder section's title tapped (a phone or a tablet): what is done in
  /// it and to it.
  function folderMenu(f: string) {
    dialog = {
      kind: "menu",
      title: short(f),
      actions: [[`New note in ${short(f)}`, () => newNote(f)], [`New folder in ${f}`, () => newFolder(f)], ...folderActions(f).slice(0, -1)],
    };
  }

  /// A phone's or a tablet's folders as sections: the notes at the top
  /// level, each folder with what is in it (the first few, the rest on a
  /// tap), and the empty folders on one line.
  const FEW = 3;
  let more: string[] = $state([]);
  type Group = { dir: api.NoteRow; items: (api.NoteRow & { rank: number })[]; count: number };
  const sections = $derived.by(() => {
    const tops: api.NoteRow[] = [];
    const groups: Group[] = [];
    let g: Group | null = null;
    for (const r of rows) {
      if (r.depth === 0) {
        g = r.is_dir ? { dir: r, items: [], count: 0 } : null;
        if (g) groups.push(g);
        else tops.push(r);
      } else if (g) {
        if (r.depth === 1) g.count++;
        g.items.push({ ...r, rank: g.count - 1 });
      }
    }
    return { tops, groups: groups.filter((x) => x.count), empty: groups.filter((x) => !x.count).map((x) => x.dir) };
  });

  /// What the tree's ⋯ does: a new folder, today's session note, and with a
  /// folder chosen its own actions (on a computer, buttons in sight too).
  /// What makes something new: in the chosen folder, or at the top level.
  function newActions(): [string, () => void][] {
    return [[selected ? `New folder in ${selected}` : "New folder", newFolder], ["Today's session note", session]];
  }

  /// What is done to the chosen folder itself.
  function folderActions(f = selected): [string, () => void][] {
    if (!f) return [];
    return [
      [`Rename ${short(f)}`, () => renameFolder(f)],
      [`Delete ${short(f)}…`, () => deleteFolder(f)],
      ...(f.includes("/")
        ? []
        : ([
            [`Move ${f} up`, () => moveFolder(f, -1)],
            [`Move ${f} down`, () => moveFolder(f, 1)],
          ] as [string, () => void][])),
      ["Back to the top level", () => (selected = "")],
    ];
  }

  function treeActions(): [string, () => void][] {
    return [...newActions(), ...folderActions()];
  }

  function renameFolder(f: string) {
    dialog = {
      kind: "prompt",
      title: `Rename ${short(f)}`,
      value: short(f),
      then: async (name) => {
        if (!name.trim()) return;
        try {
          const to = await api.noteRename(f, name.trim());
          expanded = expanded.map((p) => (p === f || p.startsWith(f + "/") ? to + p.slice(f.length) : p));
          selected = to;
          if (note?.path.startsWith(f + "/")) note = null;
          await loadTree();
        } catch (e) {
          error = String(e);
        }
      },
    };
  }

  function deleteFolder(f: string) {
    dialog = {
      kind: "confirm",
      title: `Delete ${short(f)}`,
      text: "The folder and every note in it are gone for good.",
      action: "Delete",
      then: async () => {
        try {
          await api.noteDelete(f);
          if (note?.path.startsWith(f + "/")) note = null;
          selected = noteFolder(f);
          await loadTree();
        } catch (e) {
          error = String(e);
        }
      },
    };
  }

  async function moveFolder(f: string, delta: number) {
    try {
      await api.noteMoveFolder(f, delta);
      await loadTree();
    } catch (e) {
      error = String(e);
    }
  }

  function noteMenu() {
    if (!note) return;
    dialog = { kind: "menu", title: note.title, actions: noteActions(note) };
  }

  /// What a note's ⋯ does (on a computer, buttons in sight too).
  function noteActions(n: NonNullable<typeof note>): [string, () => void][] {
    return [
        [
          "Rename",
          () =>
            (dialog = {
              kind: "prompt",
              title: "Rename",
              value: n.title,
              then: async (name) => {
                try {
                  const path = await api.noteRename(n.path, name);
                  await loadTree();
                  await open(path, false);
                } catch (e) {
                  error = String(e);
                }
              },
            }),
        ],
        [
          "Delete…",
          () =>
            (dialog = {
              kind: "confirm",
              title: `Delete ${n.title}`,
              text: "The note is gone for good.",
              action: "Delete",
              then: async () => {
                try {
                  await api.noteDelete(n.path);
                  note = null;
                  loadTree();
                } catch (e) {
                  error = String(e);
                }
              },
            }),
        ],
    ];
  }

  async function follow(target: string) {
    await save();
    try {
      const l = await api.noteResolve(target);
      if (l.type === "entity") dialog = { kind: "entry", id: l.id };
      else if (l.type === "note") open(l.path);
      else if (l.compendium) say(`Not in the compendium: ${l.title}`);
      else
        dialog = {
          kind: "menu",
          title: `“${l.title}” does not exist yet`,
          actions: [
            [`Create it in ${folder() || "the top level"}`, () => api.noteCreate(folder(), l.title).then(async (p) => (await loadTree(), open(p)))],
            ["Create it at the top level", () => api.noteCreate("", l.title).then(async (p) => (await loadTree(), open(p)))],
          ],
        };
    } catch (e) {
      error = String(e);
    }
  }

  /// A task's checkbox ticked or not: the note changes, and is saved.
  async function toggleTask(box: HTMLInputElement) {
    if (!note) return;
    text = setTask(text, Number(box.dataset.line), box.checked);
    dirty = true;
    await save();
  }

  /// Enter at the end of a list item goes on with the list; on an empty
  /// item it ends it (as the keyboard's Enter on a phone too).
  function continueList(e: InputEvent) {
    if ((e.inputType !== "insertLineBreak" && e.inputType !== "insertParagraph") || e.isComposing) return;
    const box = e.currentTarget as HTMLTextAreaElement;
    if (box.selectionStart !== box.selectionEnd) return;
    const edit = listEnter(box.value, box.selectionStart);
    if (!edit) return;
    e.preventDefault();
    box.setSelectionRange(edit.from, edit.to);
    // As typed, so Undo takes it back; else straight into the text. (An
    // empty insert deletes the selection: the bullet.)
    const done = edit.insert ? document.execCommand("insertText", false, edit.insert) : edit.from === edit.to || document.execCommand("delete");
    if (!done) {
      box.setRangeText(edit.insert, edit.from, edit.to, "end");
      box.dispatchEvent(new Event("input", { bubbles: true }));
    }
  }

  function onread(e: MouseEvent) {
    const box = (e.target as HTMLElement).closest("input.task");
    if (box instanceof HTMLInputElement) {
      toggleTask(box);
      return;
    }
    const a = (e.target as HTMLElement).closest("a[data-link]");
    if (!a) return;
    e.preventDefault();
    follow(a.getAttribute("data-link")!);
  }

  async function find() {
    found = query.trim() ? await api.noteSearch(query) : null;
  }

  $effect(() => {
    if (openPath) {
      const p = openPath;
      openPath = null;
      tick().then(() => open(p));
    }
  });

  onMount(async () => {
    await loadTree();
    expanded = rows.filter((r) => r.is_dir).map((r) => r.path);
    loadTree();
  });
</script>

{#snippet noteBar()}
  <!-- A phone's or a tablet's note: read or edit, Save, ⋯ on its title line. -->
  <span class="ntools">
    <span class="seg">
      <button class:on={!editing} onclick={async () => (await save(), (editing = false))}>read</button>
      <button class:on={editing} onclick={() => (editing = true)}>edit</button>
    </span>
    {#if dirty}<button class="primary" onclick={saveAndRead}>Save</button>{/if}
    <button class="ghost" onclick={noteMenu} aria-label="More">⋯</button>
  </span>
{/snippet}

{#snippet tools()}
  <!-- Making something new: + Note (in the chosen folder), a quick note,
       a folder, today's session note, and ⋯. On a computer at the top of
       the note's side, above the note's own actions; on a phone over the
       folders. -->
  <div class="tools" role="toolbar" aria-label="Notes">
    <!-- Opened from a character: back to their sheet (a phone's app bar has
         its own Back). -->
    {#if backTo && desktop.current}<button class="ghost backto" title="Back to {backTo}'s sheet" onclick={() => onbackto?.()}>‹ {backTo}</button>{/if}
    <button class="ghost new" onclick={() => newNote()} title={selected ? `A note in ${selected}, from its template` : "A note at the top level"}
      >+ Note{#if selected}<span class="dim">&nbsp;· {short(selected)}</span>{/if}</button
    >
    <button class="ghost" onclick={capture}>Quick note</button>
    {#if desktop.current}
      <!-- What ⋯ does, in sight: making something new here; what is done
           to the chosen folder is on its row. Short labels; the full one
           is the name. -->
      <span class="treeacts" role="group" aria-label="Notes: actions">
        {#each newActions() as [label, run] (label)}<button class="ghost" aria-label={label} title={label} onclick={run}>{shortAction(label)}</button>{/each}
      </span>
    {/if}
    <button class="ghost" onclick={treeMenu} aria-label="More">⋯</button>
  </div>
{/snippet}

<div class="notes" class:narrow class:open={!!note} class:folded={!narrow && !sidebar.open}>
  {#if !status.campaign}
    <Pane title="Notes" focused><p>Create or activate a campaign first (Campaigns).</p></Pane>
  {:else}
    <Pane title="Notes · {status.campaign}" class="tree" section={!desktop.current}>
      <div class="searchline">
        <label class="search"><span>❯</span><input bind:value={query} oninput={find} placeholder="search notes" spellcheck="false" /></label>
        {#if !desktop.current}
          <!-- A phone or a tablet: + note (at the top level; a folder's own
               is on its title), a quick note and ⋯ on the search's line. -->
          <span class="ptools" role="toolbar" aria-label="Notes">
            <button class="ghost" onclick={() => newNote("")}>+ note</button>
            <button class="ghost" onclick={capture}>quick note</button>
            <button class="ghost" onclick={treeMenu} aria-label="More">⋯</button>
          </span>
        {/if}
      </div>
      <div class="rows">
        {#if found}
          {#each found as [path, title] (path)}
            <button class="row" onclick={() => open(path)}><span class="dim">·</span> {title} <span class="dim path">{path}</span></button>
          {/each}
          {#if !found.length}<p class="dim">Nothing matches.</p>{/if}
        {:else if !desktop.current}
          {#snippet item(r: api.NoteRow, depth: number)}
            <button
              class="row"
              class:dir={r.is_dir}
              class:current={note?.path === r.path}
              style:padding-left="{depth * 2}ch"
              onclick={() => (r.is_dir ? toggle(r.path) : open(r.path))}
            >
              {#if r.is_dir}<span class="acc">{expanded.includes(r.path) ? "▾" : "▸"}</span>{/if}
              <span class="name">{r.name}</span>
              {#if r.kind}<span class="kind" class:open={r.kind === "active" || r.kind === "open"}>{r.kind}</span>{/if}
            </button>
          {/snippet}
          {#each sections.tops as r (r.path)}{@render item(r, 0)}{/each}
          {#each sections.groups as g (g.dir.path)}
            {@const all = more.includes(g.dir.path)}
            <div class="fsec">
              <div class="fhead">
                <button class="dir" onclick={() => folderMenu(g.dir.path)}>{g.dir.name}</button>
                <span class="count">{g.count}</span>
                <button class="ghost plus" aria-label="New note in {g.dir.name}" onclick={() => newNote(g.dir.path)}>+</button>
              </div>
              {#each g.items.filter((r) => all || r.rank < FEW) as r (r.path)}{@render item(r, r.depth - 1)}{/each}
              {#if g.count > FEW}
                <button class="more" onclick={() => (more = all ? more.filter((p) => p !== g.dir.path) : [...more, g.dir.path])}
                  >{all ? "fewer ▴" : `${g.count - FEW} more ▾`}</button
                >
              {/if}
            </div>
          {/each}
          {#if sections.empty.length}
            <!-- Empty folders: their names on one line, each its menu. -->
            <div class="fhead empty">
              {#each sections.empty as d, i (d.path)}{#if i}<span class="dot">·</span>{/if}<button class="dir" onclick={() => folderMenu(d.path)}>{d.name}</button>{/each}
            </div>
          {/if}
        {:else}
          {#each rows as r (r.path)}
            {#snippet line()}
              <button
                class="row"
                class:dir={r.is_dir}
                class:current={note?.path === r.path || (r.is_dir && selected === r.path)}
                style:padding-left="{r.depth * 2 + 0.5}ch"
                onclick={() => (r.is_dir ? tapFolder(r.path) : open(r.path))}
              >
                <span class="acc">{r.is_dir ? (expanded.includes(r.path) ? "▾" : "▸") : "·"}</span>
                {r.name}
              </button>
            {/snippet}
            {#if desktop.current && r.is_dir && selected === r.path}
              <!-- The chosen folder: what is done to it, on its row. -->
              <div class="chosen">
                {@render line()}
                <span class="rowacts" role="group" aria-label="{short(r.path)}: actions">
                  {#each folderActions() as [label, run] (label)}<button class="ghost" class:danger={label.startsWith("Delete")} aria-label={label} title={label} onclick={run}>{shortAction(label)}</button>{/each}
                </span>
              </div>
            {:else}
              {@render line()}
            {/if}
          {/each}
        {/if}
      </div>
    </Pane>

    <Pane
      title={note ? `${desktop.current ? note.path : note.title}${dirty ? " ●" : ""}` : "Note"}
      focused={!!note}
      class="note"
      section={!desktop.current}
      aside={!desktop.current && note ? noteBar : undefined}
    >
      {#if desktop.current}{@render tools()}{/if}
      {#if note && !desktop.current}
        <!-- Where it is: under the title (a computer has it as the title). -->
        <div class="where">{note.path}</div>
      {/if}
      {#if note && desktop.current}
        <div class="ntools">
          {#if !narrow && back.length}<button class="ghost" onclick={stepBack}>‹ back</button>{/if}
          <button class="ghost" class:on={!editing} onclick={async () => (await save(), (editing = false))}>Read</button>
          <button class="ghost" class:on={editing} onclick={() => (editing = true)}>Edit</button>
          {#if dirty}<button class="primary" onclick={saveAndRead}>Save</button>{/if}
          {#if desktop.current}
            <span class="noteacts" role="group" aria-label="{note.title}: actions">
              {#each noteActions(note) as [label, run] (label)}<button class="ghost" class:danger={label.startsWith("Delete")} onclick={run}>{label}</button>{/each}
            </span>
          {/if}
          <button class="ghost" onclick={noteMenu} aria-label="More">⋯</button>
        </div>
      {/if}
      {#if note}
        {#if editing}
          <textarea bind:value={text} oninput={() => (dirty = true)} onbeforeinput={continueList} onblur={save} spellcheck="false"></textarea>
        {:else}
          <!-- The text and its query blocks; on a computer the notes that
               link here in a column beside them. -->
          <div class="readwrap">
          <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
          <div class="read" onclick={onread}>{@html render(text)}</div>
          {#each note.queries as q, i (i)}
            <div class="query">
              <div class="qtitle"><span class="acc">{q.title}</span> <span class="dim">{desktop.current ? "· " : ""}{q.results.length}</span></div>
              {#if !q.results.length && desktop.current}<div class="dim">Nothing yet.</div>{/if}
              {#each q.results as [path, title] (path)}<button onclick={() => open(path)}>{title}</button>{/each}
            </div>
          {/each}
          </div>
          {#if note.backlinks.length}
            <div class="backlinks">
              <span class="dim">Linked from</span>
              {#each note.backlinks as [path, title] (path)}<button onclick={() => open(path)}>{title}</button>{/each}
            </div>
          {/if}
        {/if}
      {:else}
        <p class="dim">Choose a note, or start one.</p>
      {/if}
    </Pane>
  {/if}
  {#if error}<p class="err">{error}</p>{/if}
  {#if message}<div class="status">{message}</div>{/if}
</div>

{#if dialog?.kind === "prompt"}
  {@const d = dialog}
  <Prompt
    title={d.title}
    hint={d.hint}
    value={d.value}
    onclose={close}
    onsubmit={(v) => {
      const then = d.then;
      close();
      then(v);
    }}
  />
{:else if dialog?.kind === "entry"}
  <EntryModal id={dialog.id} onclose={close} />
{:else if dialog?.kind === "menu"}
  {@const d = dialog}
  <Modal title={d.title} onclose={close}>
    <div class="menu">
      {#each d.actions as [label, run] (label)}
        <button
          onclick={() => {
            close();
            run();
          }}>{label}</button
        >
      {/each}
    </div>
  </Modal>
{:else if dialog?.kind === "confirm"}
  {@const d = dialog}
  <Modal title={d.title} onclose={close}>
    <p>{d.text}</p>
    <div class="confirm">
      <button class="ghost" onclick={close}>Cancel</button>
      <button
        class="primary danger"
        onclick={() => {
          const then = d.then;
          close();
          then();
        }}>{d.action}</button
      >
    </div>
  </Modal>
{/if}

<style>
  .confirm {
    display: flex;
    justify-content: flex-end;
    gap: 1.5ch;
  }
  .confirm .danger {
    background: var(--red);
    border-color: var(--red);
  }
  @media (pointer: coarse) {
    .confirm button {
      flex: 1;
      min-height: 44px;
    }
  }
  .query {
    margin-top: 1em;
    padding-top: 0.5em;
    border-top: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    align-items: flex-start;
  }
  .query button {
    background: none;
    border: 0;
    padding: 0.15em 0;
    color: var(--link);
    cursor: pointer;
    text-align: left;
  }
  .qtitle {
    margin-bottom: 0.2em;
  }
  .notes {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(28ch, 38ch) minmax(0, 1fr);
    gap: 0 2ch;
    padding-top: 0.6em;
    position: relative;
  }
  .notes > :global(*) {
    min-height: 0;
  }
  .tools,
  .ntools {
    display: flex;
    gap: 1ch;
    flex-wrap: wrap;
    margin-bottom: 0.6em;
  }
  .ntools .on {
    border-color: var(--accent);
    color: var(--accent);
  }
  .search {
    display: flex;
    gap: 1ch;
    border-bottom: 1px solid var(--border);
    padding-bottom: 0.3em;
    margin-bottom: 0.4em;
  }
  .search span {
    color: var(--dim);
  }
  .search input {
    flex: 1;
    background: none;
    border: 0;
    outline: none;
    min-width: 0;
  }
  .rows {
    overflow-y: auto;
    flex: 1;
    min-height: 0;
  }
  .row {
    width: 100%;
    display: flex;
    gap: 1ch;
    background: none;
    border: 0;
    text-align: left;
    padding: 0.1em 0.5ch;
    cursor: pointer;
    white-space: nowrap;
    overflow: hidden;
  }
  .row:hover,
  .row.current {
    background: var(--selected);
  }
  .row.dir {
    font-weight: 700;
  }
  .path {
    margin-left: auto;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .acc {
    color: var(--accent);
  }
  textarea {
    flex: 1;
    min-height: 0;
    width: 100%;
    resize: none;
    background: none;
    border: 0;
    outline: none;
    font: inherit;
    color: inherit;
    line-height: var(--line);
    caret-color: var(--accent);
  }
  .read {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    max-width: 96ch;
  }
  .read :global(.fm) {
    display: grid;
    grid-template-columns: max-content 1fr;
    column-gap: 2ch;
    color: var(--dim);
    border-bottom: 1px solid var(--border);
    padding-bottom: 0.5em;
    margin: 0 0 0.75em;
  }
  .read :global(.fm dd) {
    margin: 0;
  }
  .read :global(.h) {
    color: var(--accent);
    font-size: 1em;
    margin: 1em 0 0.3em;
  }
  .read :global(p) {
    margin: 0 0 0.8em;
  }
  .read :global(ul) {
    margin: 0 0 0.8em;
    padding-left: 2ch;
  }
  /* Tasks: a checkbox in the bullet's place, big enough to tap. */
  .read :global(ul.tasks) {
    list-style: none;
    padding-left: 0;
  }
  .read :global(li.task label) {
    display: inline-flex;
    align-items: baseline;
    gap: 0.6ch;
    cursor: pointer;
  }
  .read :global(input.task) {
    width: 1.05em;
    height: 1.05em;
    margin: 0;
    accent-color: var(--accent);
    cursor: pointer;
    flex-shrink: 0;
    align-self: center;
  }
  .read :global(input.task:checked + span) {
    color: var(--dim);
    text-decoration: line-through;
  }
  @media (pointer: coarse) {
    .read :global(li.task label) {
      min-height: 40px;
      align-items: center;
    }
    .read :global(input.task) {
      width: 1.4em;
      height: 1.4em;
    }
  }
  .read :global(blockquote) {
    border-left: 2px solid var(--dark);
    margin: 0 0 0.8em;
    padding-left: 1.5ch;
    font-style: italic;
  }
  .read :global(pre) {
    color: var(--green);
    white-space: pre-wrap;
  }
  .read :global(code) {
    color: var(--green);
  }
  .read :global(.wl) {
    color: var(--magenta);
    text-decoration: none;
  }
  .read :global(.wl-c) {
    color: var(--cyan);
  }
  .read :global(.tag) {
    color: var(--green);
  }
  .backlinks {
    border-top: 1px solid var(--border);
    margin-top: 0.75em;
    padding-top: 0.5em;
    display: flex;
    gap: 1.5ch;
    flex-wrap: wrap;
  }
  .backlinks button {
    background: none;
    border: 0;
    color: var(--magenta);
    cursor: pointer;
    padding: 0;
  }
  .menu {
    display: flex;
    flex-direction: column;
  }
  .menu button {
    background: none;
    border: 0;
    text-align: left;
    padding: 0.3em 1ch;
    cursor: pointer;
  }
  .menu button:hover {
    background: var(--selected);
  }
  .err {
    color: var(--red);
  }
  .status {
    position: absolute;
    bottom: 0.5em;
    left: 50%;
    transform: translateX(-50%);
    background: var(--bar);
    border: 1px solid var(--accent);
    border-radius: var(--radius);
    padding: 0.3em 2ch;
  }
  .narrow,
  .folded {
    grid-template-columns: minmax(0, 1fr);
  }
  /* The folders hidden with the header's ☰. */
  .folded > :global(.tree) {
    display: none;
  }
  .narrow :global(.note),
  .narrow.open :global(.tree) {
    display: none;
  }
  .narrow.open :global(.note) {
    display: flex;
  }
  .tools .new {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .readwrap {
    display: contents;
  }
  /* A computer: the folders a share of the window, the note the rest;
     the folder's and the note's actions in sight; the notes that link here
     in a column beside the text, which keeps its reading line. */
  @media (pointer: fine) and (min-width: 761px) {
    .notes {
      grid-template-columns: minmax(16rem, 22%) minmax(0, 1fr);
      gap: 0 1rem;
    }
    /* The folders hidden: the note has the whole width. */
    .notes.folded {
      grid-template-columns: minmax(0, 1fr);
    }
    .notes :global(.note > .body) {
      display: grid;
      grid-template-columns: minmax(0, 1fr) auto;
      grid-template-rows: auto auto minmax(0, 1fr);
      column-gap: 1.5rem;
    }
    .tools,
    .ntools,
    textarea {
      grid-column: 1 / -1;
    }
    .backto {
      color: var(--accent);
      margin-right: 0.4rem;
    }
    /* Making something new, apart from the note's own actions under it. */
    .tools {
      padding-bottom: 0.6rem;
      border-bottom: 1px solid var(--edge);
    }
    .readwrap {
      display: block;
      grid-column: 1;
      min-height: 0;
      overflow-y: auto;
    }
    .read {
      max-width: none;
      overflow: visible;
    }
    .read :global(p),
    .read :global(li) {
      max-width: 75ch;
    }
    .backlinks {
      grid-column: 2;
      grid-row: 3;
      align-self: start;
      width: 15rem;
      flex-direction: column;
      align-items: flex-start;
      gap: 0.4rem;
      margin: 0;
      padding: 0 0 0 1rem;
      border-top: 0;
      border-left: 1px solid var(--edge);
    }
    /* The tree's toolbar: + Note, Quick note, the chosen folder's actions
       and ⋯, all one size, wrapping as one row. */
    .treeacts {
      display: contents;
    }
    .tools {
      gap: 0.3rem;
      align-items: center;
    }
    .tools .ghost {
      min-height: 1.95rem;
      padding: 0 0.6rem;
      font-size: 0.85em;
    }
    /* The chosen folder's row: its name, then what is done to it (wrapping
       under the name when the sidebar is narrow). */
    .chosen {
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      gap: 0.25rem;
      background: var(--selected);
      border-radius: 6px;
      padding-right: 0.25rem;
    }
    .chosen .row {
      width: auto;
      flex: 1 1 auto;
      background: none;
    }
    .rowacts {
      display: flex;
      flex-wrap: wrap;
      gap: 0.25rem;
      margin-left: auto;
      padding: 0.15rem 0;
    }
    .rowacts .ghost {
      min-height: 1.6rem;
      min-width: 1.6rem;
      padding: 0 0.45rem;
      font-size: 0.78em;
    }
    /* The note's toolbar: Read and Edit a toggle, the rest the same size. */
    .noteacts {
      display: contents;
    }
    .ntools {
      gap: 0.3rem;
      align-items: center;
    }
    .ntools .ghost,
    .ntools .primary {
      min-height: 1.95rem;
      padding: 0 0.7rem;
      font-size: 0.85em;
    }
    .ntools .ghost.on {
      background: var(--dark);
      border-color: var(--dark);
      color: var(--fg);
    }
    .danger {
      color: var(--red);
    }
  }
  @media (pointer: coarse) {
    .row {
      min-height: 46px;
      align-items: center;
      border-top: 1px solid rgb(65 72 104 / 0.3);
    }
    .tools {
      display: grid;
      /* "+ Note · <folder>" gives way (its folder ends in "…"); the others keep their label. */
      grid-template-columns: minmax(0, 1fr) auto auto;
    }
    .tools button {
      white-space: nowrap;
    }
    .menu button {
      min-height: 48px;
    }
  }
  .searchline {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }
  .searchline .search {
    flex: 1;
    min-width: 0;
  }
  /* A phone or a tablet: the folders as sections, the search a prompt line
     with + note, quick note and ⋯ on it; 36 px rows with a thin line; a
     note a section with read/edit, Save and ⋯ on its title line. */
  @media not ((pointer: fine) and (min-width: 761px)) {
    .searchline {
      min-height: 34px;
      margin-bottom: 0.3rem;
    }
    .search {
      border: 0;
      margin: 0;
      padding: 0;
      align-items: center;
    }
    .search span {
      color: var(--accent);
      font-weight: 700;
    }
    .search input {
      border-bottom: 1px solid var(--border);
      padding: 4px 0;
    }
    .search input:focus {
      border-bottom-color: var(--accent);
    }
    .ptools {
      display: flex;
      gap: 0.3rem;
      flex-shrink: 0;
    }
    .ptools button,
    .plus,
    .ntools button {
      min-height: 26px;
      height: 26px;
      padding: 0 0.6rem;
      font-size: 0.85em;
      white-space: nowrap;
    }
    .notes .row {
      min-height: 38px;
      align-items: center;
      gap: 0.8ch;
      border-top: 0;
      border-bottom: 1px solid var(--raised);
      padding-right: 0;
      border-radius: 0;
    }
    .row .name {
      flex: 1;
      min-width: 0;
      overflow: hidden;
      text-overflow: ellipsis;
    }
    .row.dir {
      font-weight: 400;
      color: var(--accent);
    }
    .kind {
      color: var(--dim);
      font-size: 0.85em;
    }
    .kind.open {
      color: var(--green);
    }
    .fsec {
      margin-top: 0.8rem;
    }
    .fhead {
      display: flex;
      align-items: center;
      gap: 0.6ch;
      min-height: 32px;
      border-bottom: 1px solid rgb(224 175 104 / 0.55);
    }
    .fhead .dir {
      background: none;
      border: 0;
      padding: 0;
      font: inherit;
      font-weight: 700;
      font-size: 1.1em;
      color: var(--accent);
      cursor: pointer;
    }
    .fhead .count {
      color: var(--dim);
      font-size: 0.9em;
      flex: 1;
    }
    .fhead.empty {
      margin-top: 0.8rem;
      flex-wrap: wrap;
    }
    .fhead.empty .dir {
      font-weight: 400;
      font-size: 1em;
    }
    /* The text as long as it is: the lists follow it. */
    .read {
      flex: none;
      overflow: visible;
    }
    .query {
      margin-top: 0.8rem;
    }
    .fhead .dot {
      color: var(--dim);
    }
    .more {
      background: none;
      border: 0;
      padding: 0;
      min-height: 30px;
      color: var(--dim);
      font-size: 0.85em;
      cursor: pointer;
    }
    .where {
      color: var(--dim);
      font-size: 0.85em;
      margin-bottom: 0.4rem;
    }
    .ntools {
      display: flex;
      align-items: center;
      gap: 0.3rem;
      margin: 0;
      flex-wrap: nowrap;
    }
    .seg {
      display: flex;
      border: 1px solid var(--border);
      border-radius: 6px;
      overflow: hidden;
    }
    .seg button {
      height: 24px;
      min-height: 24px;
      background: none;
      border: 0;
      border-radius: 0;
      color: var(--fg);
      cursor: pointer;
    }
    .seg button.on {
      background: var(--accent);
      color: var(--bg);
    }
    .query {
      border-top: 0;
      padding-top: 0;
      align-items: stretch;
    }
    .qtitle {
      display: flex;
      gap: 0.6ch;
      align-items: baseline;
      font-weight: 700;
      border-bottom: 1px solid rgb(224 175 104 / 0.3);
      padding-bottom: 0.2rem;
    }
    .qtitle .dim {
      font-weight: 400;
      font-size: 0.9em;
    }
    .query button {
      min-height: 36px;
      border-bottom: 1px solid var(--raised);
      color: var(--fg);
    }
  }
</style>
