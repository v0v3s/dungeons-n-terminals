<!-- Characters: the roster, a new character, or the open sheet. -->
<script lang="ts">
  import { onMount } from "svelte";
  import * as api from "./api";
  import Creation from "./Creation.svelte";
  import Pane from "./Pane.svelte";
  import Prompt from "./Prompt.svelte";
  import Sheet from "./Sheet.svelte";

  let {
    status,
    open = $bindable(null),
    onnote,
  }: { status: api.Status; open?: string | null; onnote: (path: string) => void } = $props();

  let roster: api.RosterRow[] = $state([]);
  let sheet: api.Sheet | null = $state(null);
  let creating: api.CreationStep | null = $state(null);
  let naming = $state(false);
  let error = $state("");

  async function load() {
    try {
      roster = await api.roster();
      sheet = await api.sheet().catch(() => null);
    } catch (e) {
      error = String(e);
    }
  }

  async function openOne(slug: string) {
    try {
      sheet = await api.openCharacter(slug);
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
    open = sheet ? sheet.header.name : creating ? `new: ${creating.name}` : null;
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
    oncancel={() => (creating = null)}
  />
{:else if sheet}
  <Sheet bind:sheet onclose={back} {onnote} />
{:else}
  <div class="roster">
    {#if !status.campaign}
      <Pane title="Characters" focused><p>Create or activate a campaign first (Campaigns).</p></Pane>
    {:else}
      <Pane title="Characters · {status.campaign}" focused>
        {#each roster as r (r.slug)}
          <button class="row" onclick={() => openOne(r.slug)}>
            <span class="acc">◆</span>
            <span class="name">{r.name}</span>
            <span class="dim id">{r.identity}</span>
          </button>
        {/each}
        {#if !roster.length}<p class="dim">No characters yet.</p>{/if}
        <div class="new"><button class="primary" onclick={() => (naming = true)}>+ New character</button></div>
      </Pane>
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
  .row {
    width: 100%;
    display: grid;
    grid-template-columns: 2ch minmax(0, 24ch) minmax(0, 1fr);
    gap: 1ch;
    background: none;
    border: 0;
    text-align: left;
    padding: 0.3em 0.5ch;
    cursor: pointer;
  }
  .row:hover {
    background: var(--selected);
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
    white-space: nowrap;
  }
  .new {
    margin-top: 0.75em;
  }
  .err {
    color: var(--red);
  }
  p {
    margin: 0 0 0.5em;
  }
  @media (max-width: 760px) {
    .row {
      grid-template-columns: 2ch minmax(0, 1fr);
      min-height: 56px;
      align-content: center;
      border-top: 1px solid rgb(65 72 104 / 0.3);
    }
    .id {
      grid-column: 2;
      font-size: 0.85em;
      white-space: normal;
    }
    .new button {
      width: 100%;
    }
  }
</style>
