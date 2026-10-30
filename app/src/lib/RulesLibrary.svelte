<!-- The rules library (github.com/v0v3s/dnt-rules): its folders, files and
     rules as a tree to tick, then imported as the user's own rules. Rules
     that differ from the user's ask first: overwrite or keep, each or all. -->
<script lang="ts">
  import { onMount } from "svelte";
  import * as api from "./api";
  import { working } from "./working.svelte";
  import Modal from "./Modal.svelte";

  let { onclose }: { onclose: () => void } = $props();

  let lib: api.RulesLibrary | null = $state(null);
  let error = $state("");
  let done = $state("");
  /// The ticked rules, by id.
  let ticked: string[] = $state([]);
  /// Folded folders and files (by name or path).
  let folded: string[] = $state([]);
  /// Asking about the ticked rules that differ: id → overwrite.
  let asking: Record<string, boolean> | null = $state(null);

  // Once, when opened (not an effect: `working` reads and sets state).
  onMount(() => {
    working("Reading the rules library…", () => api.rulesLibrary())
      .then((l) => {
        lib = l;
        // Folded at first: the folders, then what is in one when unfolded.
        folded = l.folders.flatMap((f) => [f.name, ...f.files.map((x) => x.path)]);
      })
      .catch((e) => (error = String(e)));
  });

  const rules = (l: api.RulesLibrary) => l.folders.flatMap((f) => f.files.flatMap((x) => x.rules));
  const idsOf = (list: api.LibraryRule[]) => list.map((r) => r.id);
  const folderRules = (f: api.LibraryFolder) => f.files.flatMap((x) => x.rules);

  /// "true" all ticked, "mixed" some, "false" none.
  function ticks(ids: string[]): "true" | "mixed" | "false" {
    const n = ids.filter((id) => ticked.includes(id)).length;
    return n === 0 ? "false" : n === ids.length ? "true" : "mixed";
  }
  const glyph = { true: "◉", mixed: "◐", false: "○" };

  function tick(ids: string[]) {
    ticked = ticks(ids) === "true" ? ticked.filter((id) => !ids.includes(id)) : [...new Set([...ticked, ...ids])];
  }

  function fold(key: string) {
    folded = folded.includes(key) ? folded.filter((k) => k !== key) : [...folded, key];
  }

  const STATUS: Record<api.RuleStatus, string> = { new: "new", same: "yours already", differs: "differs from yours" };

  function importTicked() {
    if (!lib) return;
    const differ = rules(lib).filter((r) => ticked.includes(r.id) && r.status === "differs");
    if (differ.length) asking = Object.fromEntries(differ.map((r) => [r.id, false]));
    else run([]);
  }

  async function run(overwrite: string[]) {
    error = "";
    try {
      const r = await working("Importing the rules…", () => api.rulesLibraryImport(ticked, overwrite));
      lib = r.library;
      done = r.summary;
      ticked = [];
      asking = null;
    } catch (e) {
      error = String(e);
    }
  }

  const labelOf = (id: string) => (lib ? rules(lib).find((r) => r.id === id)?.label ?? id : id);
</script>

<Modal title="Rules library" {onclose} wide>
  {#if asking}
    {@const a = asking}
    <p>These differ from your rules for the same features: overwrite yours, or keep them?</p>
    <div class="all">
      <button class="ghost" onclick={() => (asking = Object.fromEntries(Object.keys(a).map((id) => [id, true])))}>Overwrite all</button>
      <button class="ghost" onclick={() => (asking = Object.fromEntries(Object.keys(a).map((id) => [id, false])))}>Keep all</button>
    </div>
    <ul class="ask">
      {#each Object.keys(a) as id (id)}
        <li>
          <span class="rname">{labelOf(id)}</span>
          <span class="choice" role="group" aria-label={labelOf(id)}>
            <button class:on={a[id]} aria-pressed={a[id]} onclick={() => (asking = { ...a, [id]: true })}>Overwrite</button>
            <button class:on={!a[id]} aria-pressed={!a[id]} onclick={() => (asking = { ...a, [id]: false })}>Keep</button>
          </span>
        </li>
      {/each}
    </ul>
    <div class="go">
      <button class="primary" onclick={() => run(Object.keys(a).filter((id) => a[id]))}>Import</button>
      <button class="ghost" onclick={() => (asking = null)}>Back</button>
    </div>
  {:else}
    <p class="dim">
      Rules for features, from <span class="url">github.com/v0v3s/dnt-rules</span>. Tick a folder, a file or single
      rules: they become your rules, and the sheet uses them.
    </p>
    {#if lib}
      <div class="tree">
        {#each lib.folders as folder (folder.name)}
          {@const ids = idsOf(folderRules(folder))}
          <div class="row folder">
            <button class="fold" aria-label="{folded.includes(folder.name) ? 'Unfold' : 'Fold'} {folder.label}" onclick={() => fold(folder.name)}><span>{folded.includes(folder.name) ? "▸" : "▾"}</span></button>
            <button class="tick" role="checkbox" aria-checked={ticks(ids)} onclick={() => tick(ids)}>
              <span class="glyph">{glyph[ticks(ids)]}</span><span class="name">{folder.label}</span>
              <span class="dim">{ids.length} rule(s)</span>
            </button>
          </div>
          {#if !folded.includes(folder.name)}
            {#each folder.files as file (file.path)}
              {@const fids = idsOf(file.rules)}
              <div class="row file">
                <button class="fold" aria-label="{folded.includes(file.path) ? 'Unfold' : 'Fold'} {file.label}" onclick={() => fold(file.path)}><span>{folded.includes(file.path) ? "▸" : "▾"}</span></button>
                <button class="tick" role="checkbox" aria-checked={ticks(fids)} onclick={() => tick(fids)}>
                  <span class="glyph">{glyph[ticks(fids)]}</span><span class="name">{file.label}</span>
                  <span class="dim">{file.rules.length} rule(s)</span>
                </button>
              </div>
              {#if !folded.includes(file.path)}
                {#each file.rules as rule (rule.id)}
                  <div class="row rule">
                    <button class="tick" role="checkbox" aria-checked={ticks([rule.id])} onclick={() => tick([rule.id])}>
                      <span class="glyph">{glyph[ticks([rule.id])]}</span><span class="name">{rule.label}</span>
                      <span class="status" class:differs={rule.status === "differs"}>
                        {STATUS[rule.status]}{rule.builtin ? " · replaces dnt's own" : ""}
                      </span>
                    </button>
                  </div>
                {/each}
              {/if}
            {/each}
          {/if}
        {/each}
      </div>
      {#each lib.problems as p (p)}<p class="err">{p}</p>{/each}
      <div class="go">
        <button class="primary" disabled={!ticked.length} onclick={importTicked}>Import {ticked.length} rule(s)</button>
      </div>
    {/if}
  {/if}
  {#if done}<p class="ok">✓ {done}</p>{/if}
  {#if error}<p class="err">{error}</p>{/if}
</Modal>

<style>
  .tree {
    display: flex;
    flex-direction: column;
    margin-bottom: 0.8em;
  }
  .row {
    display: flex;
    align-items: center;
  }
  .row.file {
    padding-left: 2.5ch;
  }
  .row.rule {
    padding-left: 7.5ch;
  }
  .fold {
    align-self: stretch;
    display: flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: 0;
    color: var(--dim);
    width: 2.5ch;
    padding: 0;
    font-size: 1.15em;
    line-height: 1;
    cursor: pointer;
  }
  /* The triangles sit low in their em box. */
  .fold span {
    position: relative;
    top: -0.3em;
  }
  .tick {
    flex: 1;
    min-width: 0;
    display: flex;
    gap: 1ch;
    background: none;
    border: 0;
    text-align: left;
    padding: 0.15em 0.5ch;
    cursor: pointer;
  }
  .glyph {
    color: var(--dim);
  }
  .tick[aria-checked="true"] .glyph,
  .tick[aria-checked="mixed"] .glyph,
  .tick[aria-checked="true"] .name {
    color: var(--accent);
  }
  .folder .name {
    color: var(--accent);
    font-weight: 700;
  }
  .name {
    flex: 1;
    min-width: 0;
  }
  .status {
    color: var(--dim);
    font-size: 0.9em;
    text-align: right;
  }
  .status.differs {
    color: var(--yellow);
  }
  .url {
    color: var(--link);
  }
  .all,
  .go {
    display: flex;
    flex-wrap: wrap;
    gap: 1.5ch;
    margin: 0.6em 0;
  }
  .ask {
    list-style: none;
    padding: 0;
    margin: 0 0 0.6em;
  }
  .ask li {
    display: flex;
    gap: 1.5ch;
    align-items: center;
    padding: 0.2em 0;
  }
  .rname {
    flex: 1;
    min-width: 0;
  }
  .choice {
    display: flex;
    gap: 0.5ch;
  }
  .choice button {
    background: none;
    border: 1px solid var(--border);
    color: var(--dim);
    padding: 0.1em 1ch;
    cursor: pointer;
  }
  .choice button.on {
    border-color: var(--accent);
    color: var(--accent);
  }
  .ok {
    color: var(--green);
  }
  .err {
    color: var(--red);
  }
  @media (pointer: coarse) {
    .tick,
    .fold {
      min-height: 44px;
    }
    .row.rule {
      padding-left: 6.5ch;
    }
    .tick {
      flex-wrap: wrap;
    }
    .status {
      flex-basis: 100%;
      text-align: left;
      padding-left: 3ch;
    }
    .choice button,
    .go button,
    .all button {
      min-height: 44px;
    }
  }
</style>
