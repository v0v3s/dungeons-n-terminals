<!-- The data: import zips (a whole data folder, or homebrew files), what
     has been imported, and starting over. -->
<script lang="ts">
  import { getVersion } from "@tauri-apps/api/app";
  import { openZip } from "./files";
  import * as api from "./api";
  import { working } from "./working.svelte";
  import { ICONS } from "./icons";
  import Modal from "./Modal.svelte";
  import Pane from "./Pane.svelte";

  let version = $state("");
  getVersion().then((v) => (version = v));

  let {
    status,
    onchange,
    onreset = () => {},
  }: { status: api.Status | null; onchange: (s: api.Status) => void; onreset?: () => void } = $props();

  /// What starting over would remove, while asking.
  type Reset = "homebrew" | "data" | "all";
  let resetting: Reset | null = $state(null);

  const RESETS: Record<Reset, { title: string; goes: string; stays: string; action: string; label: string }> = {
    homebrew: {
      title: "Remove the imported homebrew",
      goes: "every imported homebrew source",
      stays: "The official data, campaigns, characters, notes and each campaign's own homebrew stay. Import homebrew again to have it back.",
      action: "Remove homebrew",
      label: "Removing the homebrew…",
    },
    data: {
      title: "Remove all imported data",
      goes: "all imported data: the official sources and the homebrew",
      stays: "Campaigns, characters, notes and each campaign's own homebrew stay; import a zip again to use them.",
      action: "Remove the data",
      label: "Removing the data…",
    },
    all: {
      title: "Reset the app",
      goes: "everything: the imported data, every campaign with its characters, notes and homebrew, and the settings",
      stays: "The app starts as new. This cannot be undone.",
      action: "Delete everything",
      label: "Resetting…",
    },
  };

  /// Who drew the icons, for the credits.
  const iconAuthors = [...new Set(Object.values(ICONS).map((i) => i.author))].sort();

  async function reset(what: Reset) {
    error = "";
    result = null;
    try {
      const s = await working(RESETS[what].label, () => (what === "all" ? api.resetApp() : api.clearImported(what === "homebrew")));
      resetting = null;
      onchange(s);
      if (what !== "homebrew") onreset();
    } catch (e) {
      error = String(e);
    }
  }

  let busy = $state(false);
  let result: api.Imported | null = $state(null);
  let error = $state("");

  async function pick() {
    error = "";
    const path = await openZip();
    if (!path) return;
    busy = true;
    try {
      result = await working("Importing…", () => api.importZip(path));
      onchange(result.status);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<div class="data">
  <div class="inner">
  <Pane title="Data" focused>
    <p>
      dnt reads the rules and content from data you provide, in the format described in the project's
      <span class="muted">schemas/</span>. Import a <span class="b">zip</span> of a whole data folder, or of homebrew
      files in any layout.
    </p>
    <p class="dim">
      Importing again updates entries instead of doubling them; entries a zip does not have are kept.
    </p>
    <p class="dim">
      Homebrew (files that name their sources) is kept apart and stays off until a campaign enables it, under
      Sources › Homebrew. For a large homebrew collection, zip just its .json files, not its images.
    </p>
    <div>
      <button class="action" onclick={pick} disabled={busy}>{busy ? "Importing…" : "Import a zip…"}</button>
    </div>
    {#if result}
      <p class="ok">✓ {result.summary}</p>
      {#if result.skipped.length}
        <details>
          <summary class="dim">{result.skipped.length} file(s) skipped</summary>
          <ul class="dim">
            {#each result.skipped.slice(0, 50) as [file, why] (file)}<li>{file}: {why}</li>{/each}
          </ul>
        </details>
      {/if}
    {/if}
    {#if error}<p class="err">{error}</p>{/if}
  </Pane>

  {#if status?.problems.length}
    <Pane title="Problems">
      {#each status.problems as p (p)}<p class="err">{p}</p>{/each}
    </Pane>
  {/if}

  {#if status?.timings.length}
    <Pane title="Last load">
      <div class="timings">
        {#each status.timings as [stage, ms], i (i)}
          <div><span>{stage}</span> <span class="dim">{ms < 1000 ? `${ms} ms` : `${(ms / 1000).toFixed(1)} s`}</span></div>
        {/each}
      </div>
    </Pane>
  {/if}

  {#if status?.homebrew[0]}
    <Pane title="Imported homebrew">
      <p>
        <b>{status.homebrew[0]}</b> sources, <b>{status.homebrew[1]}</b> entries.
        <span class="dim">Enable them per campaign: Campaigns › Sources › Homebrew, or the compendium's @ button.</span>
      </p>
    </Pane>
  {/if}


  {#if status?.loaded || status?.campaigns.length}
    <Pane title="Start over">
      <p class="dim">If an import went wrong, remove what was imported and import again.</p>
      <div class="resets">
        {#if status.homebrew[0]}<button class="ghost" onclick={() => (resetting = "homebrew")}>Remove the imported homebrew</button>{/if}
        {#if status.loaded}<button class="ghost" onclick={() => (resetting = "data")}>Remove all imported data</button>{/if}
        <button class="ghost danger" onclick={() => (resetting = "all")}>Reset the app</button>
      </div>
    </Pane>
  {/if}
  {#if status?.imported.length}
    <Pane title="Imported sources">
      <details>
        <summary>{status.imported.length} sources, {status.entries} entries</summary>
        <div class="sources">
          {#each status.imported as [source, n] (source)}
            <div><span>{source}</span> <span class="dim">{n}</span></div>
          {/each}
        </div>
      </details>
    </Pane>
  {/if}
  <Pane title="About">
    <p>Dungeons &amp; Terminals: a D&amp;D 5e companion. It ships no game content and reads the data you import.</p>
    <p>dnt {version} · MIT license · <span class="url">github.com/v0v3s/dungeons-n-terminals</span></p>
    <p class="dim">
      Icons by {iconAuthors.join(" and ")} from <span class="url">game-icons.net</span>, under CC BY 3.0
      (<span class="url">creativecommons.org/licenses/by/3.0</span>).
    </p>
    <p class="dim">Not affiliated with Wizards of the Coast.</p>
  </Pane>
  </div>
</div>

{#if resetting}
  {@const r = RESETS[resetting]}
  {@const what = resetting}
  <Modal title={r.title} onclose={() => (resetting = null)}>
    <p>This deletes {r.goes}.</p>
    <p class="dim">{r.stays}</p>
    <div class="buttons">
      <button class="ghost" onclick={() => (resetting = null)}>Cancel</button>
      <button class="primary" class:danger={what !== "homebrew"} onclick={() => reset(what)}>{r.action}</button>
    </div>
  </Modal>
{/if}

<style>
  .url {
    color: var(--link);
  }
  summary {
    cursor: pointer;
    color: var(--gray);
  }
  details[open] summary {
    margin-bottom: 0.5em;
  }
  .resets {
    display: flex;
    flex-wrap: wrap;
    gap: 1ch;
  }
  .ghost.danger {
    color: var(--red);
  }
  .primary.danger {
    background: var(--red);
    border-color: var(--red);
  }
  .buttons {
    display: flex;
    justify-content: flex-end;
    gap: 1.5ch;
    margin-top: 1em;
  }
  @media (pointer: coarse) {
    .resets {
      flex-direction: column;
    }
    .buttons button {
      flex: 1;
    }
  }
  .timings div {
    display: flex;
    justify-content: space-between;
    gap: 2ch;
  }
  /* The whole area scrolls, with its scrollbar just inside the window's
     edge (most of main's padding); the content keeps clear of it. */
  .data {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    margin-right: calc(5px - 1ch);
  }
  .inner {
    display: flex;
    flex-direction: column;
    gap: 2em;
    max-width: calc(80ch + 2ch);
    padding: 0.8em 2ch 1em 0;
  }
  /* Panes keep their height; the area scrolls instead. */
  .inner > :global(.pane) {
    flex-shrink: 0;
  }
  p {
    margin: 0 0 1em;
  }
  .b {
    font-weight: 700;
  }
  .action {
    background: var(--accent);
    color: var(--black);
    font-weight: 700;
    border: 0;
    border-radius: 3px;
    padding: 0.4em 2ch;
    cursor: pointer;
  }
  @media (pointer: coarse) {
    .action {
      min-height: 48px;
      width: 100%;
      font-size: 1.05em;
    }
    .inner {
      padding-right: 4px;
    }
  }
  .action:disabled {
    opacity: 0.6;
  }
  .ok {
    color: var(--green);
    margin-top: 1em;
  }
  .err {
    color: var(--red);
    margin-top: 1em;
  }
  .sources {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(20ch, 1fr));
    column-gap: 2ch;
  }
</style>
