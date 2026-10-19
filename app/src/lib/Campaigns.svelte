<!-- Campaigns: which one is active, and the books each one uses. -->
<script lang="ts">
  import * as api from "./api";
  import Modal from "./Modal.svelte";
  import Pane from "./Pane.svelte";
  import Prompt from "./Prompt.svelte";

  let { status, onchange }: { status: api.Status; onchange: (s: api.Status) => void } = $props();

  let error = $state("");
  let creating = $state(false);
  let renaming: api.CampaignInfo | null = $state(null);
  /// The sources editor: the campaign, and what is ticked.
  let editing: api.CampaignInfo | null = $state(null);
  let picked: string[] = $state([]);
  let reprinted = $state(false);
  let available: api.Sources | null = $state(null);
  let filter = $state("");
  let newName = $state("");
  let preset = $state(0);

  async function run(p: Promise<api.Status>) {
    try {
      error = "";
      onchange(await p);
      return true;
    } catch (e) {
      error = String(e);
      return false;
    }
  }

  async function loadSources() {
    available ??= await api.sources();
  }

  async function startCreate() {
    await loadSources();
    newName = "";
    preset = 0;
    creating = true;
  }

  async function create() {
    const sources = available?.presets[preset]?.[1] ?? [];
    if (await run(api.createCampaign(newName, [...sources]))) creating = false;
  }

  async function edit(c: api.CampaignInfo) {
    await loadSources();
    picked = [...c.sources];
    reprinted = c.show_reprinted;
    filter = "";
    editing = c;
  }

  async function saveSources() {
    if (editing && (await run(api.setCampaignSources(editing.slug, picked, reprinted)))) editing = null;
  }

  function toggle(id: string) {
    picked = picked.includes(id) ? picked.filter((s) => s !== id) : [...picked, id];
  }

  const groups = $derived.by(() => {
    const words = filter.toLowerCase().split(/\s+/).filter(Boolean);
    const out = new Map<string, api.Source[]>();
    for (const s of available?.sources ?? []) {
      if (!words.every((w) => (s.name + " " + s.id).toLowerCase().includes(w))) continue;
      const g = section(s.group);
      out.set(g, [...(out.get(g) ?? []), s]);
    }
    return [...out.entries()].sort(([a], [b]) => SECTIONS.indexOf(a) - SECTIONS.indexOf(b));
  });

  /// The data's book groups, merged into a few readable sections.
  const SECTIONS = ["Core rulebooks", "Supplements", "Settings", "Homebrew", "Adventures", "Other"];
  function section(group: string): string {
    if (group === "core") return SECTIONS[0];
    if (group.startsWith("supplement")) return SECTIONS[1];
    if (group.startsWith("setting")) return SECTIONS[2];
    if (group === "homebrew") return SECTIONS[3];
    if (group === "adventure") return SECTIONS[4];
    return SECTIONS[5];
  }

  const describe = (c: api.CampaignInfo) =>
    !c.sources.length ? "all sources" : c.sources.length > 6 ? `${c.sources.length} sources` : c.sources.join(" ");
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
      <Pane title="Campaigns" focused>
        <ul class="list">
          {#each status.campaigns as c (c.slug)}
            <li class:active={c.active}>
              <div class="main">
                <span class="mark">{c.active ? "◈" : "◇"}</span>
                <div class="what">
                  <div class="name">{c.name}</div>
                  <div class="dim">{describe(c)}{c.source ? ` · homebrew ${c.source}` : ""}</div>
                </div>
              </div>
              <div class="actions">
                {#if !c.active}<button class="primary" onclick={() => run(api.activateCampaign(c.slug))}>Activate</button>{/if}
                <button class="ghost" onclick={() => edit(c)}>Sources</button>
                <button class="ghost" onclick={() => (renaming = c)}>Rename</button>
              </div>
            </li>
          {/each}
        </ul>
        <div class="new"><button class="ghost" onclick={startCreate}>+ New campaign</button></div>
      </Pane>
    {/if}
    {#if error}<p class="err">{error}</p>{/if}
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
        {#each available?.presets ?? [] as [name, ids], i (name)}
          <button type="button" class="preset" class:on={preset === i} onclick={() => (preset = i)}>
            <span>{preset === i ? "◉" : "○"} {name}</span><span class="dim">{ids.join(" ")}</span>
          </button>
        {/each}
        <button type="button" class="preset" class:on={preset === -1} onclick={() => (preset = -1)}>
          <span>{preset === -1 ? "◉" : "○"} All sources</span><span class="dim">change them later</span>
        </button>
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
      if (renaming && (await run(api.renameCampaign(renaming.slug, name)))) renaming = null;
    }}
  />
{/if}

{#if editing}
  <Modal title="Sources · {editing.name}" onclose={() => (editing = null)} wide>
    <div class="presets row">
      {#each available?.presets ?? [] as [name, ids] (name)}
        <button class="ghost" onclick={() => (picked = [...ids])}>{name}</button>
      {/each}
      <button class="ghost" onclick={() => (picked = [])}>All</button>
    </div>
    <label class="line"><span>❯</span><input bind:value={filter} placeholder="filter" spellcheck="false" /></label>
    <p class="dim">{picked.length ? `${picked.length} chosen` : "Nothing ticked: every source is used."}</p>
    <div class="sources">
      {#each groups as [group, list] (group)}
        <h3>{group}</h3>
        {#each list as s (s.id)}
          <button class="source" class:on={picked.includes(s.id)} onclick={() => toggle(s.id)}>
            <span class="tick">{picked.includes(s.id) ? "◉" : "○"}</span>
            <span class="sname">{s.name}</span>
            <span class="dim">{s.id}</span>
          </button>
        {/each}
      {/each}
    </div>
    <label class="reprinted"><input type="checkbox" bind:checked={reprinted} /> Show entries that were reprinted in a chosen book</label>
    <div class="buttons">
      <button class="ghost" onclick={() => (editing = null)}>Cancel</button>
      <button class="primary" onclick={saveSources}>Save</button>
    </div>
  </Modal>
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
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .list li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 2ch;
    padding: 0.5em 0;
    flex-wrap: wrap;
  }
  .list li + li {
    border-top: 1px solid rgb(65 72 104 / 0.35);
  }
  .main {
    display: flex;
    gap: 1.5ch;
    align-items: baseline;
    min-width: 0;
  }
  .mark {
    color: var(--dim);
  }
  .active .mark,
  .active .name {
    color: var(--accent);
  }
  .name {
    font-weight: 700;
  }
  .actions {
    display: flex;
    gap: 1ch;
  }
  .new {
    margin-top: 0.75em;
  }
  .err {
    color: var(--red);
    margin-top: 1em;
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
  .presets.row {
    flex-direction: row;
    flex-wrap: wrap;
    gap: 1ch;
    margin-bottom: 0.75em;
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
  .sources {
    overflow-y: auto;
    max-height: 50vh;
    display: flex;
    flex-direction: column;
  }
  h3 {
    flex-shrink: 0;
    font-size: 1em;
    color: var(--accent);
    margin: 0.75em 0 0.25em;
  }
  .source {
    flex-shrink: 0;
    display: flex;
    gap: 1ch;
    background: none;
    border: 0;
    text-align: left;
    padding: 0.1em 0.5ch;
    cursor: pointer;
  }
  .source .tick {
    color: var(--dim);
  }
  .source.on .tick,
  .source.on .sname {
    color: var(--accent);
  }
  .sname {
    flex: 1;
  }
  .reprinted {
    display: flex;
    gap: 1ch;
    align-items: center;
    margin-top: 0.75em;
  }
  .buttons {
    display: flex;
    justify-content: flex-end;
    gap: 1.5ch;
    margin-top: 1em;
  }
  @media (pointer: coarse) {
    .list li {
      padding: 12px 0;
    }
    .actions {
      width: 100%;
    }
    .actions button {
      flex: 1;
    }
    .source {
      min-height: 44px;
      align-items: center;
    }
    .preset {
      min-height: 52px;
      align-items: center;
    }
    .buttons button {
      flex: 1;
    }
  }
</style>
