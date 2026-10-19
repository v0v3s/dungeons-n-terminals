<!-- A compendium entry in a modal, with its links. -->
<script lang="ts">
  import { onMount } from "svelte";
  import * as api from "./api";
  import { GLYPH } from "./api";
  import Blocks from "./Blocks.svelte";
  import Modal from "./Modal.svelte";

  let { id, onclose }: { id: number; onclose: () => void } = $props();

  let stack: api.Entry[] = $state([]);
  let error = $state("");
  const entry = $derived(stack.at(-1));

  async function open(n: number) {
    try {
      stack = [...stack, await api.entry(n)];
    } catch (e) {
      error = String(e);
    }
  }

  async function onlink(link: api.Link) {
    const n = await api.follow(link);
    if (n === null) error = `Not in the library: ${link.args.split("|")[0]}`;
    else open(n);
  }

  onMount(() => open(id));
</script>

<Modal title={entry?.name ?? "…"} {onclose} wide footer="esc">
  {#if entry}
    {#if stack.length > 1}<button class="back" onclick={() => (stack = stack.slice(0, -1))}>‹ {stack.at(-2)?.name}</button>{/if}
    <div class="sub">
      <span class="g-{entry.group}">{GLYPH[entry.group]} {entry.kind}</span><span class="dim">{`  ·  ${entry.source}${entry.page ? ` · p. ${entry.page}` : ""}`}</span>
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
