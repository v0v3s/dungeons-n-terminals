<!-- Getting started, for a new app (or one just reset): the rules (built
     in, more imported), create a campaign, then start. Each step says what it is for and does
     it in place; the done ones are ticked. -->
<script lang="ts">
  import { openZip } from "./files";
  import { openLink, WIKI_DATA } from "./links";
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

  const step = $derived(!status.loaded ? 1 : !status.campaigns.length ? 2 : 3);

  async function importZip() {
    error = "";
    const path = await openZip();
    if (!path) return;
    try {
      const r = await working("Importing…", () => api.importZip(path));
      imported = r.summary;
      onchange(r.status);
    } catch (e) {
      error = String(e);
    }
  }

  /// The first campaign uses every official book (on a first start, the
  /// SRD); its books are chosen later, from the compendium's @ button.
  async function create() {
    error = "";
    try {
      onchange(await working("Creating the campaign…", () => api.createCampaign(name, [])));
    } catch (e) {
      error = String(e);
    }
  }

  const mark = (n: number) => (n < step ? "✓" : n === step ? "▸" : "·");

  /// The wiki's page on making your own data: opened in the browser, or its
  /// address copied, for reading it on another device.
  const WIKI = WIKI_DATA;
  async function openWiki(e: MouseEvent) {
    e.preventDefault();
    try {
      await openLink(WIKI);
    } catch (e) {
      error = String(e);
    }
  }
  let copied = $state(false);
  async function copyWiki() {
    try {
      await navigator.clipboard.writeText(WIKI);
      copied = true;
      setTimeout(() => (copied = false), 3000);
    } catch (e) {
      error = String(e);
    }
  }
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

    <Pane title="{mark(1)} 1 · The rules" focused={step === 1} class={step > 1 ? "done" : ""}>
      <p>
        The rules of the System Reference Document 5.2.1 are built in: {status.entries} entries.
        {imported ? `${imported}.` : ""}
      </p>
      <p class="dim">
        Your own books and homebrew can be imported as a <b>zip</b>, now or later under Data. On a computer, the
        project's <code>scripts/pack-data.sh</code> makes a small zip of just what dnt reads.
      </p>
      <div><button class={step === 1 ? "primary" : "ghost"} onclick={importZip}>Import a zip…</button></div>
      <div class="wiki">
        <a class="url" href={WIKI} onclick={openWiki}>How to make your own data (wiki)</a>
        <button class="ghost" onclick={copyWiki}>{copied ? "Copied ✓" : "Copy link"}</button>
      </div>
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
  .go {
    display: flex;
    flex-wrap: wrap;
    gap: 1.5ch;
  }
  .err {
    color: var(--red);
  }
  .wiki {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.4em 1ch;
    margin-top: 0.9em;
  }
  .url {
    color: var(--link);
    word-break: break-all;
  }
  /* A computer: the welcome on the left, the steps on the right, the whole
     window used. */
  @media (pointer: fine) and (min-width: 761px) {
    .inner {
      display: grid;
      grid-template-columns: minmax(0, 0.9fr) minmax(0, 1.1fr);
      align-items: start;
      gap: 1.2rem 3rem;
      max-width: none;
      padding: 2.5rem 2.5rem 2rem;
    }
    .hello {
      grid-column: 1;
      grid-row: 1 / span 4;
      flex-direction: column;
      align-items: flex-start;
      gap: 1.2rem;
      padding-top: 1rem;
    }
    .hello img {
      height: 5.5rem;
    }
    h1 {
      font-size: 2.2em;
      color: var(--fg);
    }
    .hello p {
      font-size: 1.15em;
      max-width: 34ch;
    }
    .inner > :global(.pane) {
      grid-column: 2;
    }
    .inner > .err {
      grid-column: 2;
    }
  }
  @media (pointer: coarse) {
    .inner button {
      width: 100%;
      min-height: 48px;
    }
    .go {
      flex-direction: column;
    }
  }
</style>
