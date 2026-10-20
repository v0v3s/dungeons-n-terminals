<!-- Notes: the campaign's folder tree and a note, read (with its links) or
     edited. On a narrow screen the note replaces the tree. -->
<script lang="ts">
  import { onMount, tick } from "svelte";
  import * as api from "./api";
  import EntryModal from "./EntryModal.svelte";
  import Modal from "./Modal.svelte";
  import Pane from "./Pane.svelte";
  import Prompt from "./Prompt.svelte";
  import { render } from "./md";

  let {
    status,
    narrow = false,
    openPath = $bindable(null),
    reading = $bindable(null),
  }: { status: api.Status; narrow?: boolean; openPath?: string | null; reading?: string | null } = $props();

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
    | { kind: "menu"; title: string; actions: [string, () => void][] };
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
    } catch (e) {
      error = String(e);
    }
  }

  async function save() {
    if (!note || !dirty) return;
    try {
      await api.noteWrite(note.path, text);
      dirty = false;
      note = { ...note, text };
    } catch (e) {
      error = String(e);
    }
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

  function newFolder() {
    const parent = folder();
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

  function treeMenu() {
    dialog = {
      kind: "menu",
      title: "Notes",
      actions: [
        [selected ? `New folder in ${selected}` : "New folder", newFolder],
        ["Today's session note", session],
        ...(selected ? ([["Back to the top level", () => (selected = "")]] as [string, () => void][]) : []),
      ],
    };
  }

  function noteMenu() {
    if (!note) return;
    const n = note;
    dialog = {
      kind: "menu",
      title: n.title,
      actions: [
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
          "Delete",
          async () => {
            try {
              await api.noteDelete(n.path);
              note = null;
              loadTree();
            } catch (e) {
              error = String(e);
            }
          },
        ],
      ],
    };
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

  function onread(e: MouseEvent) {
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

<div class="notes" class:narrow class:open={!!note}>
  {#if !status.campaign}
    <Pane title="Notes" focused><p>Create or activate a campaign first (Campaigns).</p></Pane>
  {:else}
    <Pane title="Notes · {status.campaign}" class="tree">
      <div class="tools">
        <button class="ghost new" onclick={() => newNote()} title={selected ? `A note in ${selected}, from its template` : "A note at the top level"}
          >+ Note{#if selected}<span class="dim"> · {short(selected)}</span>{/if}</button
        >
        <button class="ghost" onclick={capture}>Quick note</button>
        <button class="ghost" onclick={treeMenu} aria-label="More">⋯</button>
      </div>
      <label class="search"><span>❯</span><input bind:value={query} oninput={find} placeholder="search notes" spellcheck="false" /></label>
      <div class="rows">
        {#if found}
          {#each found as [path, title] (path)}
            <button class="row" onclick={() => open(path)}><span class="dim">·</span> {title} <span class="dim path">{path}</span></button>
          {/each}
          {#if !found.length}<p class="dim">Nothing matches.</p>{/if}
        {:else}
          {#each rows as r (r.path)}
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
          {/each}
        {/if}
      </div>
    </Pane>

    <Pane title={note ? `${note.path}${dirty ? " ●" : ""}` : "Note"} focused={!!note} class="note">
      {#if note}
        <div class="ntools">
          {#if !narrow && back.length}<button class="ghost" onclick={stepBack}>‹ back</button>{/if}
          <button class="ghost" class:on={!editing} onclick={async () => (await save(), (editing = false))}>Read</button>
          <button class="ghost" class:on={editing} onclick={() => (editing = true)}>Edit</button>
          {#if dirty}<button class="primary" onclick={save}>Save</button>{/if}
          <button class="ghost" onclick={noteMenu} aria-label="More">⋯</button>
        </div>
        {#if editing}
          <textarea bind:value={text} oninput={() => (dirty = true)} onblur={save} spellcheck="false"></textarea>
        {:else}
          <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
          <div class="read" onclick={onread}>{@html render(text)}</div>
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
{/if}

<style>
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
  .narrow {
    grid-template-columns: minmax(0, 1fr);
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
  @media (pointer: coarse) {
    .row {
      min-height: 46px;
      align-items: center;
      border-top: 1px solid rgb(65 72 104 / 0.3);
    }
      .tools {
      display: grid;
      grid-template-columns: minmax(0, 1.4fr) minmax(0, 1fr) auto;
    }
    .menu button {
      min-height: 48px;
    }
  }
</style>
