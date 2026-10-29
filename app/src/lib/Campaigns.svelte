<!-- Campaigns: which one is active, and the books each one uses. -->
<script lang="ts">
  import * as api from "./api";
  import { saveAs } from "./files";
  import { working } from "./working.svelte";
  import Menu from "./Menu.svelte";
  import Modal from "./Modal.svelte";
  import Pane from "./Pane.svelte";
  import Prompt from "./Prompt.svelte";
  import SourcesEditor from "./SourcesEditor.svelte";

  let { status, onchange }: { status: api.Status; onchange: (s: api.Status) => void } = $props();

  let error = $state("");
  let creating = $state(false);
  let renaming: api.CampaignInfo | null = $state(null);
  let deleting: api.CampaignInfo | null = $state(null);
  /// The campaign whose sources are being edited.
  let editing: api.CampaignInfo | null = $state(null);
  let available: api.Sources | null = $state(null);
  let newName = $state("");
  /// The books of a new campaign: a preset, or -1 for all official.
  let preset = $state(-1);

  /// Run a change (showing what it does while it takes), and show the result.
  async function run(label: string, change: () => Promise<api.Status>) {
    try {
      error = "";
      onchange(await working(label, change));
      return true;
    } catch (e) {
      error = String(e);
      return false;
    }
  }

  /// What an export did.
  let notice = $state("");

  /// The campaign with all it has, to import elsewhere (Data › Import).
  async function exportCampaign(c: api.CampaignInfo) {
    const path = await saveAs(`${c.name}.dnt.zip`, "dnt export", "zip");
    if (!path) return;
    try {
      error = "";
      notice = await working("Exporting…", () => api.exportCampaign(c.slug, path));
    } catch (e) {
      error = String(e);
    }
  }

  async function loadSources() {
    available ??= await working("Reading the sources…", api.sources);
  }

  async function startCreate() {
    await loadSources();
    newName = "";
    preset = -1;
    creating = true;
  }

  async function create() {
    const sources = available?.presets[preset]?.[1] ?? [];
    if (await run("Creating the campaign…", () => api.createCampaign(newName, [...sources]))) creating = false;
  }

  const describe = (c: api.CampaignInfo) => {
    const books = !c.sources.length ? "all official" : c.sources.length > 6 ? `${c.sources.length} sources` : c.sources.join(" ");
    return c.homebrew.length ? `${books} + ${c.homebrew.length} homebrew` : books;
  };
</script>

<div class="campaigns">
  <div class="inner">
    {#if !status.loaded}
      <Pane title="Campaigns" focused><p>Import data first (Data), then create a campaign.</p></Pane>
    {:else if !status.campaigns.length}
      <Pane title="Welcome" focused>
        <p>A campaign holds your characters, notes and homebrew, and chooses which books it uses.</p>
        <div><button class="primary" onclick={startCreate}>Create a campaign…</button></div>
      </Pane>
    {:else}
      <!-- The action on its own, then each campaign as a card with its own buttons. -->
      <div class="bar">
        <h2>Campaigns</h2>
        <button class="primary" onclick={startCreate}>+ New campaign</button>
      </div>
      <div class="cards">
        {#each status.campaigns as c (c.slug)}
          <section class="card" class:active={c.active}>
            <div class="head">
              <span class="mark">{c.active ? "◈" : "◇"}</span>
              <span class="name">{c.name}</span>
              {#if c.active}<span class="badge">active</span>{/if}
              <Menu
                title={c.name}
                actions={[
                  ["Books & homebrew", () => (editing = c)],
                  ["Rename", () => (renaming = c)],
                  ["Export", () => exportCampaign(c)],
                  ["Delete", () => (deleting = c), true],
                ]}
              />
            </div>
            <div class="dim books">{describe(c)}{c.source ? ` · its own entries are source ${c.source}` : ""}</div>
            {#if !c.active}
              <div class="actions">
                <button class="primary" onclick={() => run(`Opening ${c.name}…`, () => api.activateCampaign(c.slug))}>Make active</button>
              </div>
            {/if}
          </section>
        {/each}
      </div>
    {/if}
    {#if error}<p class="err">{error}</p>{/if}
    {#if notice}<p class="ok">✓ {notice}</p>{/if}
  </div>
</div>

{#if creating}
  <Modal title="New campaign" onclose={() => (creating = false)}>
    <form
      onsubmit={(e) => {
        e.preventDefault();
        create();
      }}
    >
      <label class="line"><span>❯</span><input bind:value={newName} placeholder="Curse of Strahd" spellcheck="false" /></label>
      <p class="muted">Books</p>
      <div class="presets">
        <button type="button" class="preset" class:on={preset === -1} onclick={() => (preset = -1)}>
          <span>{preset === -1 ? "◉" : "○"} All official</span><span class="dim">newest printings, no homebrew</span>
        </button>
        {#each available?.presets ?? [] as [name, ids], i (name)}
          <button type="button" class="preset" class:on={preset === i} onclick={() => (preset = i)}>
            <span>{preset === i ? "◉" : "○"} {name}</span><span class="dim">{ids.join(" ")}</span>
          </button>
        {/each}
      </div>
      <div class="buttons">
        <button type="button" class="ghost" onclick={() => (creating = false)}>Cancel</button>
        <button type="submit" class="primary" disabled={!newName.trim()}>Create</button>
      </div>
    </form>
  </Modal>
{/if}

{#if renaming}
  <Prompt
    title="Rename {renaming.name}"
    value={renaming.name}
    action="Rename"
    onclose={() => (renaming = null)}
    onsubmit={async (name) => {
      const c = renaming;
      if (c && (await run("Renaming…", () => api.renameCampaign(c.slug, name)))) renaming = null;
    }}
  />
{/if}

{#if deleting}
  {@const c = deleting}
  <Modal title="Delete {c.name}" onclose={() => (deleting = null)}>
    <p>Delete <b>{c.name}</b> with its characters, notes and homebrew? This cannot be undone.</p>
    {#if c.active && status.campaigns.length > 1}
      <p class="dim">Another campaign becomes the active one.</p>
    {/if}
    <div class="buttons">
      <button class="ghost" onclick={() => (deleting = null)}>Cancel</button>
      <button
        class="primary danger"
        onclick={async () => {
          if (await run(`Deleting ${c.name}…`, () => api.deleteCampaign(c.slug))) deleting = null;
        }}>Delete</button
      >
    </div>
  </Modal>
{/if}

{#if editing}
  <SourcesEditor campaign={editing} onsave={onchange} onclose={() => (editing = null)} />
{/if}

<style>
  .campaigns {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }
  .inner {
    max-width: 90ch;
    padding: 0.8em 1ch 1em 0;
  }
  p {
    margin: 0 0 1em;
  }
  .bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 2ch;
    flex-wrap: wrap;
    margin-bottom: 1em;
  }
  h2 {
    margin: 0;
    font-size: 1.05em;
    color: var(--accent);
  }
  .cards {
    display: flex;
    flex-direction: column;
    gap: 1em;
  }
  .card {
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 0.8em 1.5ch;
    display: flex;
    flex-direction: column;
    gap: 0.5em;
  }
  .card.active {
    border-color: var(--accent);
  }
  .head {
    display: flex;
    align-items: center;
    gap: 1.5ch;
  }
  .mark {
    color: var(--dim);
  }
  .active .mark,
  .active .name {
    color: var(--accent);
  }
  .name {
    flex: 1;
    min-width: 0;
    font-weight: 700;
  }
  .badge {
    font-size: 0.85em;
    color: var(--black);
    background: var(--accent);
    border-radius: 3px;
    padding: 0 0.8ch;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 1ch;
    margin-top: 0.2em;
  }
  .err {
    color: var(--red);
    margin-top: 1em;
  }
  .ok {
    color: var(--green);
    margin-top: 1em;
  }
  .primary.danger {
    background: var(--red);
    border-color: var(--red);
  }
  .line {
    display: flex;
    gap: 1ch;
    align-items: center;
    border-bottom: 1px solid var(--border);
    padding: 0.3em 0;
    margin-bottom: 0.75em;
  }
  .line span {
    color: var(--accent);
  }
  .line input {
    flex: 1;
    background: none;
    border: 0;
    outline: none;
    min-width: 0;
  }
  .presets {
    display: flex;
    flex-direction: column;
    gap: 0.3em;
  }
  .preset {
    display: flex;
    justify-content: space-between;
    gap: 2ch;
    background: none;
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 0.4em 1.5ch;
    text-align: left;
    cursor: pointer;
  }
  .preset.on {
    border-color: var(--accent);
    color: var(--accent);
  }
  .buttons {
    display: flex;
    justify-content: flex-end;
    gap: 1.5ch;
    margin-top: 1em;
  }
  @media (pointer: coarse) {
    .bar button {
      width: 100%;
      min-height: 48px;
    }
    .actions button {
      flex: 1;
    }
    .preset {
      min-height: 52px;
      flex-direction: column;
      justify-content: center;
      gap: 0.15em;
    }
    .buttons button {
      flex: 1;
    }
  }
</style>
