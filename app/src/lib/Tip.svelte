<!-- A one-time tip on a phone (tips.svelte.ts): `? swipe sideways…`, gone
     with its ✕. -->
<script lang="ts">
  import { touch } from "./screen";
  import { tips } from "./tips.svelte";

  let { id, text }: { id: string; text: string } = $props();
</script>

{#if touch.current && tips.due(id)}
  <div class="tip" role="note" data-tip={id}>
    <span class="q">?</span>
    <span class="text">{text}</span>
    <button class="close" aria-label="Dismiss the tip" onclick={() => tips.done(id)}>✕</button>
  </div>
{/if}

<style>
  .tip {
    display: flex;
    align-items: center;
    gap: 1ch;
    color: var(--gray);
    border: 1px dashed var(--border);
    border-radius: var(--radius);
    padding: 0 0 0 1ch;
    margin: 0.4em 0;
    flex-shrink: 0;
  }
  .q {
    color: var(--cyan);
    font-weight: 700;
  }
  .text {
    flex: 1;
    min-width: 0;
  }
  .close {
    width: 40px;
    height: 40px;
    background: none;
    border: 0;
    color: var(--dim);
    cursor: pointer;
  }
</style>
