<!-- Editing a homebrew entry: its fields as rows (changed ones marked);
     tapping one edits it the way it is written (a switch, a choice, a
     line, Markdown or JSON). Below: the whole entry as JSON, a new
     field, sharing it with every campaign, deleting it. -->
<script lang="ts">
  import * as api from "./api";
  import Modal from "./Modal.svelte";
  import Prompt from "./Prompt.svelte";
  import { working } from "./working.svelte";

  let {
    editing = $bindable(),
    onchanged,
    onclose,
  }: {
    editing: api.Editing;
    /// The library changed (the entry was saved).
    onchanged: () => void;
    /// Closed: the entry's id now, or null when it was deleted.
    onclose: (id: number | null) => void;
  } = $props();

  let field: api.FieldRow | null = $state(null);
  /// The text being edited for a field (or the whole entry).
  let text = $state("");
  let wholeJson = $state(false);
  let adding: "name" | { name: string } | null = $state(null);
  let confirmDelete = $state(false);
  let error = $state("");

  const where = $derived(
    editing.doc === "global"
      ? "global homebrew: every campaign has it"
      : editing.replaces
        ? "campaign version: replaces the book entry in this campaign"
        : "campaign homebrew",
  );

  async function run(label: string, change: () => Promise<api.Editing>) {
    try {
      error = "";
      editing = await working(label, change);
      onchanged();
      return true;
    } catch (e) {
      error = String(e);
      return false;
    }
  }

  /// For a field of several choices: what is ticked, and a custom one.
  let ticked: string[] = $state([]);
  let custom = $state("");

  function openField(f: api.FieldRow) {
    error = "";
    text = f.text;
    ticked = f.multi && f.display !== "—" ? f.display.split(", ").filter(Boolean) : [];
    custom = "";
    field = f;
  }

  function tick(c: string) {
    ticked = ticked.includes(c) ? ticked.filter((t) => t !== c) : [...ticked, c];
  }

  async function set(to: api.SetTo) {
    if (field && (await run("Saving…", () => api.brewSet(field!.name, to)))) field = null;
  }

  async function openJson() {
    try {
      text = await api.brewJson();
      wholeJson = true;
    } catch (e) {
      error = String(e);
    }
  }

  async function remove() {
    try {
      await working("Deleting…", api.brewDelete);
      onchanged();
      onclose(null);
    } catch (e) {
      error = String(e);
    }
  }

  const big = (f: api.FieldRow) => f.widget === "entries" || f.widget === "json";
  const revertLabel = $derived(editing.replaces ? "Back to the book" : "Remove the field");
</script>

<Modal title="Edit · {editing.name}" onclose={() => onclose(editing.id)} wide>
  <p class="dim where">{editing.kind} · {where}</p>
  {#if editing.message}<p class="ok">{editing.message}</p>{/if}
  <div class="fields">
    {#each editing.fields as f (f.name)}
      <button class="field" class:changed={f.changed} onclick={() => openField(f)}>
        <span class="mark">{f.changed ? "●" : " "}</span>
        <span class="name">{f.label || f.name}</span>
        <span class="value">{f.display || "—"}</span>
      </button>
    {/each}
  </div>
  {#if error && !field && !wholeJson}<p class="err">{error}</p>{/if}
  <div class="tools">
    <button class="ghost" onclick={openJson}>Edit as JSON</button>
    <button class="ghost" onclick={() => (adding = "name")}>+ Field</button>
    {#if editing.doc === "campaign" && !editing.replaces}
      <button class="ghost" onclick={() => run("Sharing…", api.brewPromote)}>Make global</button>
    {/if}
    <button class="ghost danger" onclick={() => (confirmDelete = true)}>{editing.replaces ? "Remove" : "Delete"}</button>
    <button class="primary" onclick={() => onclose(editing.id)}>Done</button>
  </div>
</Modal>

{#if field}
  {@const f = field}
  <Modal title={f.label || f.name} onclose={() => (field = null)} wide={big(f)}>
    {#if f.help}<p class="dim">{f.help}</p>{/if}
    {#if f.multi && f.choices}
      {@const others = ticked.filter((t) => !f.choices!.includes(t))}
      <div class="choices">
        {#each [...f.choices, ...others] as c (c)}
          <button class="choice" class:on={ticked.includes(c)} onclick={() => tick(c)}>{ticked.includes(c) ? "◉" : "○"} {c}</button>
        {/each}
      </div>
      <form
        onsubmit={(e) => {
          e.preventDefault();
          if (custom.trim() && !ticked.includes(custom.trim())) ticked = [...ticked, custom.trim()];
          custom = "";
        }}
      >
        <label class="line"><span>+</span><input bind:value={custom} placeholder="a custom one" spellcheck="false" autocomplete="off" /></label>
      </form>
      {#if f.hint}<p class="dim">{f.hint}</p>{/if}
    {:else if f.widget === "bool"}
      <div class="choices">
        <button class="choice" class:on={f.display === "yes"} onclick={() => f.display !== "yes" && set({ type: "toggle" })}>yes</button>
        <button class="choice" class:on={f.display !== "yes"} onclick={() => f.display === "yes" && set({ type: "toggle" })}>no</button>
      </div>
    {:else if f.choices}
      <div class="choices">
        {#each f.choices as c, i (i)}
          <button class="choice" class:on={c === f.display} onclick={() => set({ type: "choice", value: i })}>{c}</button>
        {/each}
      </div>
    {:else if big(f)}
      <p class="dim">{f.widget === "entries" ? "Markdown: paragraphs, - lists, ## headings, **bold**, {@tags}." : "JSON."}</p>
      <textarea
        bind:value={text}
        spellcheck="false"
        autocapitalize={f.widget === "json" ? "off" : "sentences"}
        rows="14"
      ></textarea>
    {:else}
      <form
        onsubmit={(e) => {
          e.preventDefault();
          set({ type: "text", value: text });
        }}
      >
        <label class="line"><span>❯</span><input bind:value={text} inputmode={f.widget === "number" ? "decimal" : "text"} spellcheck="false" autocomplete="off" /></label>
      </form>
      {#if f.hint}<p class="dim">{f.hint}</p>{/if}
    {/if}
    {#if error}<p class="err">{error}</p>{/if}
    <div class="buttons">
      {#if f.changed}<button class="ghost" onclick={() => set({ type: "clear" })}>{revertLabel}</button>{/if}
      <span class="gap"></span>
      <button class="ghost" onclick={() => (field = null)}>Cancel</button>
      {#if f.multi}
        <button class="primary" onclick={() => set({ type: "text", value: [...ticked, custom.trim()].filter(Boolean).join(", ") })}>Save</button>
      {:else if f.widget !== "bool" && !f.choices}
        <button class="primary" onclick={() => set({ type: "text", value: text })}>Save</button>
      {/if}
    </div>
  </Modal>
{/if}

{#if wholeJson}
  <Modal title="{editing.name} · JSON" onclose={() => (wholeJson = false)} wide>
    <p class="dim">The entry as stored{editing.replaces ? " (only what differs from the book, with _copy)" : ""}.</p>
    <textarea bind:value={text} spellcheck="false" autocapitalize="off" autocomplete="off" rows="18"></textarea>
    {#if error}<p class="err">{error}</p>{/if}
    <div class="buttons">
      <span class="gap"></span>
      <button class="ghost" onclick={() => (wholeJson = false)}>Cancel</button>
      <button
        class="primary"
        onclick={async () => {
          if (await run("Saving…", () => api.brewReplace(text))) wholeJson = false;
        }}>Save</button
      >
    </div>
  </Modal>
{/if}

{#if adding === "name"}
  <Prompt title="New field" hint="Its JSON name, e.g. weight, bonusWeapon, resist" action="Next" onclose={() => (adding = null)} onsubmit={(n) => (adding = n.trim() ? { name: n.trim() } : null)} />
{:else if adding}
  {@const name = adding.name}
  <Prompt
    title="{name} ="
    hint={'Its value as JSON: 3, "text", true, ["a", "b"]'}
    action="Add"
    onclose={() => (adding = null)}
    onsubmit={async (v) => {
      if (await run("Saving…", () => api.brewSet(name, { type: "json", value: v }))) adding = null;
    }}
  />
{/if}

{#if confirmDelete}
  <Modal title={editing.replaces ? "Remove the campaign version" : `Delete ${editing.name}`} onclose={() => (confirmDelete = false)}>
    <p>
      {editing.replaces
        ? "The campaign goes back to the book entry."
        : editing.doc === "global"
          ? "It goes from the global homebrew, so from every campaign."
          : "It goes from this campaign's homebrew."}
    </p>
    <div class="buttons">
      <span class="gap"></span>
      <button class="ghost" onclick={() => (confirmDelete = false)}>Cancel</button>
      <button class="primary danger" onclick={remove}>{editing.replaces ? "Remove" : "Delete"}</button>
    </div>
  </Modal>
{/if}

<style>
  p {
    margin: 0 0 0.6em;
  }
  .ok {
    color: var(--green);
  }
  .err {
    color: var(--red);
  }
  .fields {
    display: flex;
    flex-direction: column;
    max-height: 55vh;
    overflow-y: auto;
  }
  .field {
    flex-shrink: 0;
    display: grid;
    grid-template-columns: 2ch minmax(10ch, 16ch) minmax(0, 1fr);
    gap: 1ch;
    align-items: baseline;
    background: none;
    border: 0;
    border-bottom: 1px solid rgb(65 72 104 / 0.35);
    padding: 0.35em 0.5ch;
    text-align: left;
    cursor: pointer;
  }
  .mark {
    color: var(--accent);
  }
  .name {
    color: var(--gray);
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .changed .name {
    color: var(--accent);
  }
  .value {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tools,
  .buttons {
    display: flex;
    flex-wrap: wrap;
    gap: 1ch;
    margin-top: 1em;
  }
  .tools .primary {
    margin-left: auto;
  }
  .gap {
    flex: 1;
  }
  .ghost.danger {
    color: var(--red);
  }
  .primary.danger {
    background: var(--red);
    border-color: var(--red);
  }
  .choices {
    display: flex;
    flex-direction: column;
    gap: 0.3em;
    max-height: 50vh;
    overflow-y: auto;
  }
  .choice {
    flex-shrink: 0;
    background: none;
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 0.4em 1.5ch;
    text-align: left;
    cursor: pointer;
  }
  .choice.on {
    border-color: var(--accent);
    color: var(--accent);
  }
  .line {
    display: flex;
    gap: 1ch;
    align-items: center;
    border-bottom: 1px solid var(--border);
    padding: 0.3em 0;
    margin-bottom: 0.6em;
  }
  .line span {
    color: var(--accent);
  }
  .line input {
    flex: 1;
    min-width: 0;
    background: none;
    border: 0;
    outline: none;
  }
  textarea {
    width: 100%;
    box-sizing: border-box;
    min-height: 12em;
    background: var(--black);
    color: var(--fg);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 0.6em 1ch;
    font: inherit;
    resize: vertical;
  }
  @media (pointer: coarse) {
    /* One line a field: the name, and its value to the right. */
    .field {
      min-height: 48px;
      align-items: center;
      grid-template-columns: 2ch auto minmax(0, 1fr);
    }
    .field .value {
      text-align: right;
      color: var(--gray);
    }
    .tools button,
    .buttons button {
      flex: 1;
      min-height: 44px;
    }
    .tools .primary {
      flex-basis: 100%;
    }
    .gap {
      display: none;
    }
    .choice {
      min-height: 48px;
    }
  }
</style>
