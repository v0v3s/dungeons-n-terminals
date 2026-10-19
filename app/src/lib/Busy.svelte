<!-- While something slow runs: taps are caught at once, and after a moment
     a small pane says what is happening, with a terminal spinner. -->
<script lang="ts">
  import { fade } from "svelte/transition";
  import { busy } from "./busy.svelte";

  const FRAMES = "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏";
  let frame = $state(0);
  let shown = $state(false);

  $effect(() => {
    if (!busy.label) {
      shown = false;
      return;
    }
    // Quick work never flashes the pane.
    const show = setTimeout(() => (shown = true), 150);
    const spin = setInterval(() => (frame = (frame + 1) % FRAMES.length), 80);
    return () => {
      clearTimeout(show);
      clearInterval(spin);
    };
  });
</script>

{#if busy.label}
  <div class="catch" role="presentation">
    {#if shown}
      <div class="pane" transition:fade={{ duration: 100 }} role="status">
        <span class="spin">{FRAMES[frame]}</span>
        <span>{busy.label}</span>
      </div>
    {/if}
  </div>
{/if}

<style>
  .catch {
    position: fixed;
    inset: 0;
    z-index: 100;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: progress;
  }
  .pane {
    display: flex;
    gap: 1.5ch;
    align-items: center;
    background: var(--bg);
    border: 1px solid var(--accent);
    border-radius: 6px;
    padding: 0.9em 2.5ch;
    box-shadow: 0 0 0 100vmax rgb(0 0 0 / 0.35);
  }
  .spin {
    color: var(--accent);
    font-weight: 700;
  }
</style>
