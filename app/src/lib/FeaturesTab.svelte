<!-- Features by where they come from; tap one to read it, or write the
     rules that put it on the sheet. -->
<script lang="ts">
  import type { Feature } from "./api";
  import Pane from "./Pane.svelte";
  import type { SheetCtx } from "./sheetctx";

  let { groups, ctx }: { groups: { title: string; features: Feature[] }[]; ctx: SheetCtx } = $props();

  function swap(f: Feature) {
    ctx.pick({ type: "mastery" }, ([value]) => ctx.act({ command: "swap_mastery", weapon: f.mastery, new: value }));
  }
</script>

<div class="features">
  {#each groups as g (g.title)}
    <Pane title={g.title}>
      {#each g.features as f (f.index)}
        <div class="row">
          <button class="feature" onclick={() => ctx.show(f.entity)}>
            <span class="lvl dim">{f.level > 0 ? f.level : ""}</span>
            <span class="name">{f.name}</span>
            {#if f.automated}<span class="auto">auto</span>{/if}
          </button>
          {#if f.mastery}<button class="ghost swap" onclick={() => swap(f)}>swap</button>{/if}
          <button class="rules" title="The rules that put it on the sheet" onclick={() => ctx.rules(f.entity)}>rules</button>
        </div>
      {/each}
    </Pane>
  {/each}
  {#if !groups.length}<p class="dim">No features yet: take a level.</p>{/if}
</div>

<style>
  /* Quiet: a feature's rules are for the curious. */
  .rules {
    background: none;
    border: 0;
    color: var(--dim);
    padding: 0 1ch;
    min-height: 2.4em;
    cursor: pointer;
  }
  .rules:hover {
    color: var(--accent);
  }
  .features {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(38ch, 1fr));
    gap: 1.4em 2ch;
    align-items: start;
    padding-top: 0.6em;
  }
  .row {
    display: flex;
    align-items: center;
  }
  .feature {
    flex: 1;
    display: grid;
    grid-template-columns: 3ch 1fr auto;
    gap: 1ch;
    background: none;
    border: 0;
    padding: 0.1em 0;
    text-align: left;
    cursor: pointer;
  }
  .feature:hover .name {
    color: var(--accent);
  }
  .auto {
    color: var(--accent);
    font-size: 0.85em;
  }
  .swap {
    padding: 0 1ch;
  }
  @media (max-width: 760px) {
    .features {
      grid-template-columns: minmax(0, 1fr);
    }
    .feature {
      min-height: 44px;
      align-items: center;
    }
  }
</style>
