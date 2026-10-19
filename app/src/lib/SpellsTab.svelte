<!-- Spells per casting class and per feature, favorites first. Tap a
     spell for what you can do with it. -->
<script lang="ts">
  import type { SpellRow } from "./api";
  import type { SheetCtx } from "./sheetctx";

  let { rows, ctx }: { rows: SpellRow[]; ctx: SheetCtx } = $props();

  const MARK = { cantrip: "◦", prepared: "●", book: "○", granted: "◆" };

  function add(class_id: string) {
    ctx.pick({ type: "class_spells", class_id }, (ids, o) => ctx.act({ command: "add_spells", to: o.target, ids }));
  }

  function choose(r: Extract<SpellRow, { type: "choice" }>) {
    ctx.pick({ type: "grant_spells", key: r.key, filter: r.filter, missing: r.missing }, (ids, o) =>
      ctx.act({ command: "add_spells", to: o.target, ids }),
    );
  }

  function cast(id: string, free: boolean) {
    if (free) return ctx.act({ command: "cast", id, how: "free" });
    ctx.pick({ type: "cast", id }, ([how]) => ctx.act({ command: "cast", id, how }));
  }

  function menu(r: Extract<SpellRow, { type: "spell" }>) {
    const actions: [string, () => void][] = [];
    if (r.entity !== null) actions.push(["Read it", () => ctx.show(r.entity!)]);
    if (r.castable) actions.push([r.free ? "Cast (a free cast)" : "Cast…", () => cast(r.id, r.free)]);
    if (r.free) actions.push(["Cast with a slot…", () => cast(r.id, false)]);
    actions.push(["Roll it", () => ctx.roll({ type: "spell", id: r.id })]);
    if (r.class_id && r.mark !== "cantrip") {
      actions.push([r.mark === "prepared" ? "Unprepare" : "Prepare", () => ctx.act({ command: "toggle_prepared", class_id: r.class_id, id: r.id })]);
    }
    actions.push([r.favorite ? "No longer a favorite" : "Make it a favorite", () => ctx.act({ command: "toggle_favorite", id: r.id })]);
    if (r.target) actions.push(["Remove", () => ctx.act({ command: "remove_spell", from: r.target, id: r.id })]);
    ctx.menu(r.name, actions);
  }
</script>

<div class="spells">
  {#if !rows.length}
    <p class="muted">No spellcasting yet.</p>
    <p class="dim">Spellcasting classes, subclasses and feats like Magic Initiate add spells here.</p>
  {/if}
  {#each rows as r, i (i)}
    {#if r.type === "header"}
      <div class="header">
        <div class="htitle">
          <span class="title">{r.title}</span>
          {#if r.class_id}<button class="ghost add" onclick={() => add(r.class_id!)}>+ add</button>{/if}
        </div>
        {#if r.detail}<div class="dim">{r.detail}</div>{/if}
        {#if r.counts.length}
          <div class="counts">
            {#each r.counts as [label, have, max] (label)}<span><span class="dim">{label}</span> <b class:over={have > max}>{have}/{max}</b></span>{/each}
          </div>
        {/if}
      </div>
    {:else if r.type === "spell"}
      <button class="spell" onclick={() => menu(r)}>
        <span class="mark" class:dim={r.mark === "book"}>{MARK[r.mark]}</span>
        <span class="name">{r.name}{#if r.favorite && !r.in_favorites}<span class="star"> ★</span>{/if}</span>
        <span class="detail dim">{r.detail}</span>
      </button>
    {:else}
      <button class="choice" onclick={() => choose(r)}>+ {r.text}</button>
    {/if}
  {/each}
  <p class="legend dim">◦ cantrip · ● prepared or known · ○ in the spellbook · ◆ from a feature</p>
</div>

<style>
  .spells {
    max-width: 110ch;
    padding-top: 0.6em;
  }
  :where(button:not(.ghost):not(.primary)) {
    background: none;
    border: 0;
    text-align: left;
    cursor: pointer;
  }
  .header {
    margin-top: 1.2em;
    padding-bottom: 0.3em;
    border-bottom: 1px solid var(--border);
    margin-bottom: 0.3em;
  }
  .header:first-child {
    margin-top: 0;
  }
  .htitle {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .title {
    color: var(--accent);
    font-weight: 700;
  }
  .add {
    padding: 0 1.5ch;
  }
  .counts {
    display: flex;
    gap: 2ch;
    flex-wrap: wrap;
  }
  .over {
    color: var(--red);
  }
  .spell {
    width: 100%;
    display: grid;
    grid-template-columns: 2ch minmax(0, 1fr) auto;
    gap: 1ch;
    padding: 0.1em 0.5ch;
  }
  .spell:hover {
    background: var(--selected);
  }
  .mark {
    color: var(--accent);
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .star {
    color: var(--accent);
  }
  .detail {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .choice {
    color: var(--accent);
    padding: 0.2em 0.5ch;
  }
  .legend {
    margin-top: 1.5em;
  }
  @media (max-width: 760px) {
    .spell {
      grid-template-columns: 2ch minmax(0, 1fr);
      min-height: 48px;
      align-content: center;
      border-top: 1px solid rgb(65 72 104 / 0.3);
    }
    .detail {
      grid-column: 2;
      font-size: 0.85em;
      white-space: normal;
    }
    .choice,
    .add {
      min-height: 44px;
    }
  }
</style>
