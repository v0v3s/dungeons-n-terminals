<!-- The next roll on a computer: Normal, Advantage or Disadvantage, in
     sight (the phone's chip cycles the same mode), and the keys that roll
     one number either way. -->
<script lang="ts">
  import type { RollMode, SheetCtx } from "./sheetctx";

  let { mode, ctx }: { mode: string; ctx: SheetCtx } = $props();

  const MODES: [RollMode, string][] = [
    ["normal", "Normal"],
    ["advantage", "Advantage"],
    ["disadvantage", "Disadvantage"],
  ];
</script>

<div class="rollmode">
  <div class="seg" role="group" aria-label="Next roll">
    <span class="rlabel">Roll</span>
    {#each MODES as [m, label] (m)}
      <button class:on={mode === m} class:hot={m !== "normal"} aria-pressed={mode === m} onclick={() => ctx.setRollMode(m)}>{label}</button>
    {/each}
  </div>
  <span class="dim hint"><kbd>Shift</kbd>+click advantage · <kbd>Ctrl</kbd>+click disadvantage</span>
</div>

<style>
  .rollmode {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.4rem 1rem;
  }
  .seg {
    display: inline-flex;
    align-items: center;
    gap: 0.2rem;
    padding: 0.2rem;
    background: var(--card);
    border: 1px solid var(--edge);
    border-radius: 0.6rem;
  }
  .rlabel {
    padding: 0 0.5rem 0 0.4rem;
    font-size: 0.85em;
    font-weight: 600;
    color: var(--dim);
  }
  button {
    min-height: 1.9rem;
    padding: 0 0.8rem;
    border: 0;
    border-radius: 0.45rem;
    background: none;
    color: var(--gray);
    font: inherit;
    font-size: 0.9em;
    white-space: nowrap;
    cursor: pointer;
  }
  button:hover {
    background: var(--raised);
    color: var(--fg);
  }
  button.on {
    background: var(--dark);
    color: var(--fg);
    font-weight: 600;
  }
  /* Advantage or disadvantage on: as the phone's chip shows it. */
  button.on.hot {
    background: var(--cyan);
    color: var(--black);
  }
  .hint {
    font-size: 0.85em;
  }
  kbd {
    font: inherit;
    font-family: var(--mono);
    font-size: 0.9em;
    padding: 0 0.3rem;
    border: 1px solid var(--border);
    border-radius: 0.3rem;
    color: var(--fg);
  }
</style>
