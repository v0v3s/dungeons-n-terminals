<!-- Characters: the roster, a new character, or the open sheet. -->
<script lang="ts">
  import { onMount } from "svelte";
  import * as api from "./api";
  import Creation from "./Creation.svelte";
  import Pane from "./Pane.svelte";
  import Prompt from "./Prompt.svelte";
  import Sheet from "./Sheet.svelte";
  import type { SheetTab } from "./sheetctx";

  let {
    status,
    open = $bindable(null),
    tab = $bindable("Overview"),
    actions = $bindable([]),
    onnote,
  }: {
    status: api.Status;
    open?: string | null;
    tab?: SheetTab;
    /// The open sheet's actions, for the app bar.
    actions?: { label: string; name: string; onclick: () => void }[];
    onnote: (path: string) => void;
  } = $props();

  let roster: api.RosterRow[] = $state([]);
  let unreadable: string[] = $state([]);
  let sheet: api.Sheet | null = $state(null);
  let creating: api.CreationStep | null = $state(null);
  let naming = $state(false);
  let error = $state("");

  async function load() {
    try {
      roster = await api.roster();
      unreadable = await api.unreadableCharacters();
      sheet = await api.sheet().catch(() => null);
      // A new character in progress carries on where it was left.
      creating = await api.creationCurrent().catch(() => null);
    } catch (e) {
      error = String(e);
    }
  }

  async function openOne(slug: string) {
    try {
      sheet = await api.openCharacter(slug);
      tab = "Overview";
    } catch (e) {
      error = String(e);
    }
  }

  async function start(name: string) {
    try {
      creating = await api.creationStart(name);
      naming = false;
    } catch (e) {
      error = String(e);
      naming = false;
    }
  }

  /// Back to the roster.
  export async function back(): Promise<boolean> {
    if (creating) {
      const prev = await api.creationBack();
      creating = prev;
      return true;
    }
    if (sheet) {
      await api.closeCharacter();
      sheet = null;
      load();
      return true;
    }
    return false;
  }

  $effect(() => {
    // The name with the character's level (all classes together).
    open = sheet ? (sheet.header.level ? `${sheet.header.name} · Lv ${sheet.header.level}` : sheet.header.name) : creating ? `new: ${creating.name}` : null;
  });

  onMount(load);
</script>

{#if creating}
  <Creation
    bind:step={creating}
    ondone={(s) => {
      creating = null;
      sheet = s;
      load();
    }}
    oncancel={async () => {
      await api.creationCancel().catch(() => {});
      creating = null;
    }}
  />
{:else if sheet}
  <Sheet bind:sheet bind:tab bind:actions onclose={back} {onnote} />
{:else}
  <div class="roster">
    {#if !status.campaign}
      <Pane title="Characters" focused><p>Create or activate a campaign first (Campaigns).</p></Pane>
    {:else}
      <!-- The action on its own, then the characters: each a card to open. -->
      <div class="bar">
        <h2>Characters <span class="dim">· {status.campaign}</span></h2>
        <button class="primary" onclick={() => (naming = true)}>+ New character</button>
      </div>
      {#if !roster.length}<p class="dim empty">No characters yet: start one with <b>+ New character</b>.</p>{/if}
      <div class="cards">
        {#each roster as r (r.slug)}
          <button class="card" onclick={() => openOne(r.slug)}>
            <span class="acc">◆</span>
            <span class="what">
              <span class="name">{r.name}</span>
              <span class="dim id">{r.identity}</span>
            </span>
            <span class="dim go">›</span>
          </button>
        {/each}
      </div>
      {#each unreadable as u (u)}<p class="err">Could not read {u}</p>{/each}
    {/if}
    {#if error}<p class="err">{error}</p>{/if}
  </div>
{/if}

{#if naming}
  <Prompt title="New character" hint="What is their name?" action="Start" onclose={() => (naming = false)} onsubmit={start} />
{/if}

<style>
  .roster {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 0.8em 1ch 1em 0;
    max-width: 100ch;
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
    gap: 0.8em;
  }
  .card {
    display: grid;
    grid-template-columns: 2ch minmax(0, 1fr) auto;
    gap: 1ch;
    align-items: center;
    background: none;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 0.7em 1.5ch;
    text-align: left;
    cursor: pointer;
  }
  .card:hover {
    border-color: var(--accent);
  }
  .what {
    display: flex;
    flex-direction: column;
    gap: 0.15em;
    min-width: 0;
  }
  .acc {
    color: var(--accent);
  }
  .name {
    font-weight: 700;
  }
  .id {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .go {
    font-size: 1.3em;
  }
  .empty {
    margin: 0 0 1em;
  }
  .err {
    color: var(--red);
  }
  p {
    margin: 0 0 0.5em;
  }
  @media (max-width: 760px) {
    .bar button {
      width: 100%;
      min-height: 48px;
    }
    .card {
      min-height: 64px;
    }
    .id {
      font-size: 0.9em;
    }
  }
</style>
