<!-- The first screen while the library loads: what is being read, with a
     terminal spinner and the time so far. -->
<script lang="ts">
  import { onMount } from "svelte";

  let { stage }: { stage: string } = $props();

  const FRAMES = "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏";
  let frame = $state(0);
  let seconds = $state(0);

  onMount(() => {
    const start = Date.now();
    const spin = setInterval(() => {
      frame = (frame + 1) % FRAMES.length;
      seconds = Math.floor((Date.now() - start) / 1000);
    }, 80);
    return () => clearInterval(spin);
  });
</script>

<div class="loading" role="status">
  <img class="brand" src="/dnt-mark.svg" alt="dnt" />
  <div class="line">
    <span class="spin">{FRAMES[frame]}</span>
    <span>{stage || "Starting"}…</span>
  </div>
  <div class="dim">{seconds >= 2 ? `${seconds} s` : " "}</div>
</div>

<style>
  .loading {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 1em;
  }
  .brand {
    height: 7em;
    width: auto;
    flex-shrink: 0;
    align-self: center;
  }
  .line {
    display: flex;
    gap: 1.5ch;
  }
  .spin {
    color: var(--accent);
    font-weight: 700;
  }
</style>
