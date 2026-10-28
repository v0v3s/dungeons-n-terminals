<!-- Spells per casting class and per feature, favorites first; in each,
     by level, with the level's slots in its heading and whether a spell is
     prepared on its left. Tap a spell for what you can do with it. -->
<script lang="ts">
  import type { SpellRow, Tracker } from "./api";
  import Icon from "./Icon.svelte";
  import { pool, type SheetCtx } from "./sheetctx";

  let { rows, slots = [], ctx }: { rows: SpellRow[]; slots?: Tracker[]; ctx: SheetCtx } = $props();

  const MARK = { cantrip: "", prepared: "●", book: "○", granted: "◆" };

  type Spell = Extract<SpellRow, { type: "spell" }>;
  type Choice = Extract<SpellRow, { type: "choice" }>;
  type Section = { header: Extract<SpellRow, { type: "header" }> | null; spells: Spell[]; choices: Choice[] };

  /// The rows as sections: a class, a feature or the favorites, each with
  /// its spells and the choices still to make.
  const sections = $derived.by(() => {
    const out: Section[] = [];
    for (const r of rows) {
      if (r.type === "header" || !out.length) out.push({ header: r.type === "header" ? r : null, spells: [], choices: [] });
      const sec = out[out.length - 1];
      if (r.type === "spell") sec.spells.push(r);
      else if (r.type === "choice") sec.choices.push(r);
    }
    return out;
  });

  /// A section's spells by level: a class shows every level it casts
  /// (with its slots), even with no spells there yet.
  function levels(sec: Section): [number | null, Spell[]][] {
    const by = new Map<number | null, Spell[]>();
    const h = sec.header;
    if (h?.class_id) {
      if (h.counts.some(([label]) => label === "cantrips")) by.set(0, []);
      for (let l = 1; l <= (h.max_level ?? 0); l++) by.set(l, []);
    }
    for (const r of sec.spells) by.set(r.level, [...(by.get(r.level) ?? []), r]);
    return [...by].sort(([a], [b]) => (a ?? 99) - (b ?? 99));
  }

  /// The slots for a spell level: the level's own, and pact slots of that level.
  const slotsAt = (level: number) =>
    slots.filter((t) => t.id === `slot:${level}` || (t.id === "pact" && t.name.includes(`level ${level})`)));

  function add(class_id: string) {
    ctx.pick({ type: "class_spells", class_id }, (ids, o) => ctx.act({ command: "add_spells", to: o.target, ids }));
  }

  function choose(r: Choice) {
    ctx.pick({ type: "grant_spells", key: r.key, filter: r.filter, missing: r.missing }, (ids, o) =>
      ctx.act({ command: "add_spells", to: o.target, ids }),
    );
  }

  function cast(id: string, free: boolean) {
    if (free) return ctx.act({ command: "cast", id, how: "free" });
    ctx.pick({ type: "cast", id }, ([how]) => ctx.act({ command: "cast", id, how }));
  }

  function menu(r: Spell) {
    const actions: [string, () => void][] = [];
    if (r.entity !== null) actions.push(["Read it", () => ctx.show(r.entity!)]);
    if (r.castable) actions.push([r.free ? "Cast (a free cast)" : "Cast…", () => cast(r.id, r.free)]);
    if (r.free) actions.push(["Cast with a slot…", () => cast(r.id, false)]);
    actions.push(["Roll it", () => ctx.roll({ type: "spell", id: r.id, caster: r.caster })]);
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
  {#if !sections.some((sec) => sec.header?.class_id) && slots.length}
    <!-- Slots without a casting class (a feature's spells): on their own. -->
    <div class="level">{#each slots as t (t.id)}{@render slot(t)}{/each}</div>
  {/if}
  {#each sections as sec, si (si)}
    {#if sec.header}
      {@const h = sec.header}
      <div class="header">
        <div class="htitle">
          <span class="title">{h.title}</span>
          {#if h.class_id}<button class="ghost add" onclick={() => add(h.class_id!)}>+ add</button>{/if}
        </div>
        {#if h.dc !== null}
          <div class="boxes">
            <span class="box"><b>{h.dc}</b><span class="dim">save DC</span></span>
            <span class="box"><b>{h.attack}</b><span class="dim">attack</span></span>
            <span class="box"><b>{h.max_level}</b><span class="dim">up to level</span></span>
          </div>
        {/if}
        {#if h.counts.length}
          <div class="counts">
            {#each h.counts as [label, have, max] (label)}<span><span class="dim">{label}</span> <b class:over={have > max}>{have}/{max}</b></span>{/each}
          </div>
        {/if}
      </div>
    {/if}
    {#each sec.choices as c (c.key)}<button class="choice" onclick={() => choose(c)}>+ {c.text}</button>{/each}
    {#each levels(sec) as [level, spells] (level)}
      <div class="level">
        <span class="ltitle">{level === 0 ? "Cantrips" : level === null ? "Other" : `Level ${level}`}</span>
        {#if sec.header?.class_id && level}{#each slotsAt(level) as t (t.id)}{@render slot(t)}{/each}{/if}
      </div>
      {#each spells as r, i (i)}
        <div class="srow">
          {#if r.class_id && r.mark !== "cantrip"}
            <button
              class="prep"
              role="checkbox"
              aria-checked={r.mark === "prepared"}
              aria-label="Prepared: {r.name}"
              onclick={() => ctx.act({ command: "toggle_prepared", class_id: r.class_id, id: r.id })}>{r.mark === "prepared" ? "✓" : ""}</button
            >
          {:else}
            <span class="mark">{MARK[r.mark]}</span>
          {/if}
          <button class="spell" onclick={() => menu(r)}>
            <span class="name">{r.name}{#if r.favorite && !r.in_favorites}<span class="star"> ★</span>{/if}</span>
            <span class="detail dim">{r.detail}</span>
          </button>
          {#if r.rollable}
            <button class="rollbox roll" aria-label="Roll {r.name}" onclick={() => ctx.roll({ type: "spell", id: r.id, caster: r.caster })}><Icon name="d20" /></button>
          {/if}
        </div>
      {/each}
      {#if !spells.length}<p class="dim none">none yet</p>{/if}
    {/each}
  {/each}
  {#if rows.length}<p class="legend dim">✓ prepared from the spellbook · ● prepared or known · ◆ from a feature</p>{/if}
</div>

{#snippet slot(t: Tracker)}
  <span class="slot" data-pool={t.name}>
    {#if t.id === "pact"}<span class="dim">pact</span>{/if}
    <span class="pips"><span class="acc">{"●".repeat(Math.max(t.left, 0))}</span><span class="dim">{"○".repeat(Math.max(t.max - t.left, 0))}</span></span>
    <span class="count">{t.left}/{t.max}</span>
    <button class="step" aria-label="Use" onclick={() => ctx.act({ command: "spend", pool: pool(t.id), amount: 1 })}>−</button>
    <button class="step" aria-label="Restore" onclick={() => ctx.act({ command: "spend", pool: pool(t.id), amount: -1 })}>+</button>
  </span>
{/snippet}

<style>
  .level {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.3em 2ch;
    margin: 0.9em 0 0.2em;
    padding-bottom: 0.15em;
    border-bottom: 1px solid rgb(65 72 104 / 0.6);
  }
  .ltitle {
    flex: 1;
    color: var(--gray);
    font-weight: 700;
  }
  .slot {
    display: inline-flex;
    align-items: center;
    gap: 1ch;
  }
  .slot .count {
    color: var(--gray);
  }
  .slot .pips {
    letter-spacing: 0.15ch;
  }
  .boxes {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 1ch;
    margin: 0.5em 0;
  }
  .box {
    display: flex;
    flex-direction: column;
    align-items: center;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 0.4em 0.5ch;
  }
  .box b {
    color: var(--accent);
    font-size: 1.2em;
  }
  .box .dim {
    font-size: 0.8em;
  }
  .srow {
    display: grid;
    grid-template-columns: 3ch minmax(0, 1fr) auto;
    gap: 1ch;
    align-items: center;
  }
  .prep {
    width: 2.4ch;
    height: 2.4ch;
    display: inline-flex !important;
    align-items: center;
    justify-content: center;
    border: 1px solid var(--accent) !important;
    border-radius: 3px;
    color: var(--accent);
    font-weight: 700;
    padding: 0 !important;
  }
  .roll {
    min-width: 0;
    padding: 0.3em 1ch !important;
    border: 1px solid var(--border) !important;
    color: var(--accent);
  }
  .prep[aria-checked="false"] {
    border-color: var(--border) !important;
  }
  .none {
    margin: 0.2em 0 0 4ch;
  }
  .step {
    border: 1px solid var(--border) !important;
    border-radius: 4px;
    text-align: center !important;
    min-height: 2em;
  }
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
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 1ch;
    padding: 0.1em 0.5ch;
  }
  .spell:hover {
    background: var(--selected);
  }
  .mark {
    color: var(--accent);
    text-align: center;
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
    .level {
      min-height: 44px;
    }
    .step {
      min-height: 40px;
      min-width: 40px;
    }
    .prep {
      width: 32px;
      height: 32px;
    }
    .spell {
      grid-template-columns: minmax(0, 1fr);
      min-height: 48px;
      align-content: center;
    }
    .srow {
      grid-template-columns: 36px minmax(0, 1fr) auto;
      border-top: 1px solid rgb(65 72 104 / 0.3);
    }
    .detail {
      font-size: 0.85em;
      white-space: normal;
    }
    .choice,
    .add {
      min-height: 44px;
    }
  }
</style>
