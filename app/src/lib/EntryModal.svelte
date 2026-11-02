<!-- A compendium entry in a modal, with its links. -->
<script lang="ts">
  import { onMount } from "svelte";
  import * as api from "./api";
  import { GLYPH } from "./api";
  import Blocks from "./Blocks.svelte";
  import Modal from "./Modal.svelte";

  /// `part`: just that feature of the entry (a species trait), with a way
  /// to the whole entry.
  let { id, part = null, onclose }: { id: number; part?: string | null; onclose: () => void } = $props();

  let stack: api.Entry[] = $state([]);
  let error = $state("");
  const entry = $derived(stack.at(-1));

  async function open(n: number, only: string | null = null) {
    try {
      stack = [...stack, await (only ? api.featureEntry(n, only) : api.entry(n))];
    } catch (e) {
      error = String(e);
    }
  }

  async function onlink(link: api.Link) {
    const n = await api.follow(link);
    if (n === null) error = `Not in the library: ${link.args.split("|")[0]}`;
    else open(n);
  }

  onMount(() => open(id, part));
</script>

<Modal title={entry?.name ?? "…"} {onclose} wide footer="esc">
  {#if entry}
    {#if stack.length > 1}<button class="back" onclick={() => (stack = stack.slice(0, -1))}>‹ {stack.at(-2)?.name}</button>{/if}
    <div class="sub">
      <span class="g-{entry.group}">{GLYPH[entry.group]} {entry.kind}</span><span class="dim">{`  ·  ${entry.source}${entry.page ? ` · p. ${entry.page}` : ""}`}</span>
      {#if entry.from}
        {@const f = entry.from}
        <div>
          <span class="dim">from</span>
          <button class="from g-{f.group}" onclick={() => open(f.id)}>{GLYPH[f.group]} {f.name}</button>
          <span class="dim">({f.kind.toLowerCase()})</span>
        </div>
      {/if}
    </div>
    <div class="doc"><Blocks blocks={entry.blocks} {onlink} /></div>
  {/if}
  {#if error}<p class="err">{error}</p>{/if}
</Modal>

<style>
  .back {
    background: none;
    border: 0;
    padding: 0;
    color: var(--link);
    cursor: pointer;
    text-align: left;
    margin-bottom: 0.25em;
  }
  .from {
    background: none;
    border: 0;
    padding: 0;
    cursor: pointer;
    text-decoration: underline;
    text-underline-offset: 0.2em;
  }
  .sub {
    border-bottom: 1px solid var(--border);
    padding-bottom: 0.5em;
    margin-bottom: 0.75em;
  }
  .doc {
    overflow-y: auto;
  }
  .err {
    color: var(--red);
  }
</style>
