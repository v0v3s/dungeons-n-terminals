<!-- The data: import zips (a whole data folder, or homebrew files), and
     what has been imported. -->
<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import * as api from "./api";
  import Pane from "./Pane.svelte";

  let { status, onchange }: { status: api.Status | null; onchange: (s: api.Status) => void } = $props();

  let busy = $state(false);
  let result: api.Imported | null = $state(null);
  let error = $state("");

  async function pick() {
    error = "";
    const path = await open({ multiple: false, directory: false, filters: [{ name: "Zip", extensions: ["zip"] }] });
    if (!path) return;
    busy = true;
    try {
      result = await api.importZip(path);
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

  {#if status?.sources.length}
    <Pane title="Imported sources">
      <div class="sources">
        {#each status.sources as [source, n] (source)}
          <div><span>{source}</span> <span class="dim">{n}</span></div>
        {/each}
      </div>
    </Pane>
  {/if}
  </div>
</div>

<style>
  /* The whole area scrolls, with its scrollbar at the window's edge
     (past main's padding); the content keeps clear of it. */
  .data {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    margin-right: -1ch;
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
