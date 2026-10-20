<!-- Getting started, for a new app (or one just reset): import the data,
     create a campaign, then start. Each step says what it is for and does
     it in place; the done ones are ticked. -->
<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import * as api from "./api";
  import { working } from "./working.svelte";
  import Pane from "./Pane.svelte";

  let {
    status,
    onchange,
    ondone,
  }: {
    status: api.Status;
    onchange: (s: api.Status) => void;
    /// Leave for a section.
    ondone: (section: "characters" | "compendium") => void;
  } = $props();

  let error = $state("");
  let imported = $state("");
  let name = $state("");
  let presets: [string, string[]][] = $state([]);
  /// The books of the new campaign: a preset's name, or "" for all official.
  let preset = $state("");

  const step = $derived(!status.loaded ? 1 : !status.campaigns.length ? 2 : 3);

  $effect(() => {
    if (status.loaded && !presets.length) api.sources().then((s) => (presets = s.presets));
  });

  async function importZip() {
    error = "";
    const path = await open({ multiple: false, directory: false, filters: [{ name: "Zip", extensions: ["zip"] }] });
    if (!path) return;
    try {
      const r = await working("Importing…", () => api.importZip(path));
      imported = r.summary;
      onchange(r.status);
    } catch (e) {
      error = String(e);
    }
  }

  async function create() {
    error = "";
    const sources = presets.find(([n]) => n === preset)?.[1] ?? [];
    try {
      onchange(await working("Creating the campaign…", () => api.createCampaign(name, [...sources])));
    } catch (e) {
      error = String(e);
    }
  }

  const mark = (n: number) => (n < step ? "✓" : n === step ? "▸" : "·");
</script>

<div class="welcome">
  <div class="inner">
    <div class="hello">
      <img src="/dnt-mark.svg" alt="dnt" />
      <div>
        <h1>Getting started</h1>
        <p class="dim">A D&amp;D 5e companion: a compendium, character sheets that do the maths, dice and campaign notes.</p>
      </div>
    </div>

    <Pane title="{mark(1)} 1 · Import the data" focused={step === 1} class={step > 1 ? "done" : ""}>
      {#if step === 1}
        <p>
          dnt ships no game content: it reads the rules, spells, classes and items from data you import, as a
          <b>zip</b>.
        </p>
        <p class="dim">
          On a computer, the project's <code>scripts/pack-data.sh</code> makes a small zip of just what dnt reads,
          from a data repository and a homebrew collection. Put the zip on this device, then import it.
        </p>
        <div><button class="primary" onclick={importZip}>Import a zip…</button></div>
      {:else}
        <p class="dim">{imported ? `${imported}.` : `${status.entries} entries.`} More data or homebrew can be imported later, under Data.</p>
      {/if}
    </Pane>

    <Pane title="{mark(2)} 2 · Create a campaign" focused={step === 2} class={step !== 2 ? "done" : ""}>
      {#if step === 2}
        <p>A campaign holds your characters, notes and homebrew, and chooses the books it uses.</p>
        <form
          onsubmit={(e) => {
            e.preventDefault();
            create();
          }}
        >
          <label class="line"><span>❯</span><input bind:value={name} placeholder="the campaign's name" spellcheck="false" /></label>
          <label class="books">
            <span class="dim">Books</span>
            <select bind:value={preset}>
              <option value="">All official</option>
              {#each presets as [n] (n)}<option value={n}>{n}</option>{/each}
            </select>
          </label>
          <p class="dim">The books and homebrew can be changed at any time, from the compendium's @ button.</p>
          <div><button class="primary" type="submit" disabled={!name.trim()}>Create the campaign</button></div>
        </form>
      {:else if step > 2}
        <p class="dim">{status.campaign ?? status.campaigns[0]?.name} is ready. More campaigns can be added under Campaigns.</p>
      {:else}
        <p class="dim">After the data.</p>
      {/if}
    </Pane>

    <Pane title="{mark(3)} 3 · Play" focused={step === 3}>
      {#if step === 3}
        <p>Everything is set. Create a character, or look something up.</p>
        <div class="go">
          <button class="primary" onclick={() => ondone("characters")}>Create a character</button>
          <button class="ghost" onclick={() => ondone("compendium")}>Browse the compendium</button>
        </div>
      {:else}
        <p class="dim">Characters, the compendium and notes, once there is a campaign.</p>
      {/if}
    </Pane>

    {#if error}<p class="err">{error}</p>{/if}
  </div>
</div>

<style>
  .welcome {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }
  .inner {
    display: flex;
    flex-direction: column;
    gap: 1.6em;
    max-width: 80ch;
    padding: 0.8em 1ch 1.5em 0;
  }
  .inner > :global(.pane) {
    flex-shrink: 0;
  }
  .inner > :global(.pane.done) {
    opacity: 0.75;
  }
  .hello {
    display: flex;
    gap: 2ch;
    align-items: center;
  }
  .hello img {
    height: 4.5em;
    flex-shrink: 0;
  }
  h1 {
    margin: 0 0 0.2em;
    font-size: 1.2em;
    color: var(--accent);
  }
  p {
    margin: 0 0 0.8em;
  }
  .hello p {
    margin: 0;
  }
  code {
    color: var(--code);
  }
  .line {
    display: flex;
    gap: 1ch;
    align-items: center;
    border-bottom: 1px solid var(--border);
    padding: 0.3em 0;
    margin-bottom: 0.8em;
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
  .books {
    display: flex;
    gap: 1.5ch;
    align-items: center;
    margin-bottom: 0.6em;
  }
  select {
    flex: 1;
    min-width: 0;
  }
  .go {
    display: flex;
    flex-wrap: wrap;
    gap: 1.5ch;
  }
  .err {
    color: var(--red);
  }
  @media (pointer: coarse) {
    .inner button {
      width: 100%;
      min-height: 48px;
    }
    .go {
      flex-direction: column;
    }
    select {
      min-height: 44px;
    }
  }
</style>
