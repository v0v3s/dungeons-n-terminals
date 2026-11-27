<!-- All of a character's spells in one list by level, whichever class or
     feature gave them (a letter says which); the slots, one pool, and the
     casters' numbers at the top. Chips show one class or what can be cast
     now; a level folds away. Tap a spell for what you can do with it. -->
<script lang="ts">
  import * as api from "./api";
  import type { SpellRow, Tracker } from "./api";
  import Icon from "./Icon.svelte";
  import Pane from "./Pane.svelte";
  import { pool, type SheetCtx } from "./sheetctx";
  import { desktop } from "./screen";

  let { rows, slots = [], ctx }: { rows: SpellRow[]; slots?: Tracker[]; ctx: SheetCtx } = $props();

  type Spell = Extract<SpellRow, { type: "spell" }>;
  type Choice = Extract<SpellRow, { type: "choice" }>;
  type Header = Extract<SpellRow, { type: "header" }>;
  /// A casting class: its header, a short tag for its spells and a colour.
  type Caster = { header: Header; id: string; name: string; short: string; tag: string; color: string };
  /// A spell in the list: its row, and where it is from (a class, or a
  /// feature's name).
  type Entry = { r: Spell; caster: Caster | null; feature: string | null; favorite: boolean };

  const COLORS = ["var(--accent)", "var(--magenta)", "var(--cyan)", "var(--green)"];

  /// The first letters of `names[i]`, at least `min`, as many as tell it
  /// from the others (Warlock and Wizard: Wa, Wi).
  function prefix(names: string[], i: number, min: number): string {
    let n = min;
    while (n < names[i].length && names.some((o, j) => j !== i && o.slice(0, n) === names[i].slice(0, n))) n++;
    return names[i].slice(0, n);
  }

  /// The casting classes: a letter (or two) to tag their spells, and a
  /// short name for when the full names do not fit.
  const casters = $derived.by(() => {
    const heads = rows.filter((r): r is Header => r.type === "header" && !!r.class_id);
    const names = heads.map((h) => h.title.replace(/\s+\d+$/, ""));
    return heads.map(
      (header, i): Caster => ({
        header,
        id: header.class_id!,
        name: names[i],
        short: prefix(names, i, 3),
        tag: prefix(names, i, 1),
        color: COLORS[i % COLORS.length],
      }),
    );
  });

  /// The filter chips on one line: full names if they fit, else short
  /// names, else the letters. Monospace: widths count in characters.
  let chipsWidth = $state(0);
  let ch = $state(8);
  /// A phone: the filters go on the Spellcasting title line, beside its
  /// title (as wide as the tab, less the title's room).
  let tabWidth = $state(0);
  const chipLabel = $derived.by(() => {
    const room = desktop.current ? chipsWidth : tabWidth - 16 * ch;
    const fits = (labels: string[]) =>
      [...labels, "All", ...(features && !desktop.current ? ["◆"] : [])].reduce((w, l) => w + (desktop.current ? (l.length + 4) * ch + 2 : l.length * ch + 18), 0) <= room;
    for (const pick of [(c: Caster) => c.name, (c: Caster) => c.short]) if (fits(casters.map(pick))) return pick;
    return (c: Caster) => c.tag;
  });

  /// Every spell with where it is from, and the choices still to make.
  const listed = $derived.by(() => {
    const entries: Entry[] = [];
    const choices: Choice[] = [];
    let head: Header | null = null;
    for (const r of rows) {
      if (r.type === "header") head = r;
      else if (r.type === "choice") choices.push(r);
      else {
        const caster = casters.find((c) => c.id === r.origin) ?? null;
        const feature = caster || !head || head.class_id || r.in_favorites ? null : head.title;
        entries.push({ r, caster, feature, favorite: r.in_favorites });
      }
    }
    return { entries, choices };
  });

  /// Spells from feats and other features (not a class's), if any.
  const features = $derived(listed.entries.some((e) => e.feature));

  /// What the list shows: every spell, one class's, or the features'.
  let show = $state<string>("all");
  const picked = $derived(casters.find((c) => c.id === show) ?? null);

  /// Whether a spell can be cast now: a cantrip, a cast without a slot, or
  /// a slot (or pact slot) of its level or higher left.
  function castableNow(r: Spell): boolean {
    if (!r.level || r.free) return true;
    return slots.some((t) => t.left > 0 && slotLevel(t) >= r.level!);
  }
  const slotLevel = (t: Tracker) => Number(t.id.startsWith("slot:") ? t.id.slice(5) : (/level (\d+)/.exec(t.name)?.[1] ?? 0));

  const visible = $derived(
    listed.entries.filter((e) => show === "all" || (show === "features" ? !!e.feature : e.caster?.id === show)),
  );

  /// The spells by level, favorites first; a class's levels show even
  /// with no spells yet.
  const levels = $derived.by(() => {
    const by = new Map<string, Entry[]>();
    if (visible.some((e) => e.favorite)) by.set("fav", []);
    const classes = picked ? [picked] : show === "features" ? [] : casters;
    if (classes.some((c) => c.header.counts.some(([label]) => label === "cantrips"))) by.set("0", []);
    for (let l = 1; l <= Math.max(0, ...classes.map((c) => c.header.max_level ?? 0)); l++) by.set(String(l), []);
    for (const e of visible) {
      const key = e.favorite ? "fav" : e.r.level === null ? "other" : String(e.r.level);
      by.set(key, [...(by.get(key) ?? []), e]);
    }
    const order = (k: string) => (k === "fav" ? -1 : k === "other" ? 99 : Number(k));
    return [...by].sort(([a], [b]) => order(a) - order(b));
  });
  /// A level's slots left, its own and pact slots of that level together:
  /// `3/4`; none for a level without slots.
  function slotsOf(k: string): string | null {
    const at = slots.filter((t) => (t.id.startsWith("slot:") || t.id === "pact") && String(slotLevel(t)) === k);
    if (!at.length) return null;
    return `${at.reduce((n, t) => n + t.left, 0)}/${at.reduce((n, t) => n + t.max, 0)}`;
  }
  const levelTitle = (k: string) => (k === "fav" ? "★ Favorites" : k === "0" ? "Cantrips" : k === "other" ? "Other" : `Level ${k}`);

  /// Folded levels, remembered on this device.
  let folded = $state<string[]>(load());
  function load(): string[] {
    try {
      return JSON.parse(localStorage.getItem("dnt.spells.folded") ?? "[]");
    } catch {
      return [];
    }
  }
  function fold(k: string) {
    folded = folded.includes(k) ? folded.filter((x) => x !== k) : [...folded, k];
    try {
      localStorage.setItem("dnt.spells.folded", JSON.stringify(folded));
    } catch {
      // Not remembered; it still folds.
    }
  }

  /// A slot pool's pips: tap to use one or get one back.
  function slotMenu(t: Tracker) {
    ctx.menu(t.name, [
      ["Use", () => ctx.act({ command: "spend", pool: pool(t.id), amount: 1 })],
      ["Restore", () => ctx.act({ command: "spend", pool: pool(t.id), amount: -1 })],
    ]);
  }

  function add(class_id: string) {
    ctx.pick({ type: "class_spells", class_id }, (ids, o) => ctx.act({ command: "add_spells", to: o.target, ids }));
  }

  /// Add spells: to the class shown, the only one, or the one chosen.
  function addAny() {
    if (picked) return add(picked.id);
    if (casters.length === 1) return add(casters[0].id);
    ctx.menu(
      "Add spells to…",
      casters.map((c) => [c.header.title, () => add(c.id)]),
    );
  }

  function choose(r: Choice) {
    ctx.pick({ type: "grant_spells", key: r.key, filter: r.filter, missing: r.missing }, (ids, o) =>
      ctx.act({ command: "add_spells", to: o.target, ids }),
    );
  }

  /// Cast a spell from where the character has it (its origin: the class
  /// or the feature). Its feature's cast without a slot goes at once; so
  /// does the only way to cast it (no higher slot, no ritual); otherwise
  /// it asks how. `ask` always asks.
  async function cast(r: Spell, ask = false) {
    const { id, origin } = r;
    if (r.free && !ask) return ctx.act({ command: "cast", id, how: "free", origin });
    let ways: api.Options;
    try {
      ways = await api.options({ type: "cast", id, origin });
    } catch (e) {
      return ctx.say(String(e));
    }
    const usable = ways.choices.filter((c) => !c.warning);
    if (!ask && usable.length === 1 && ways.choices.length === 1) return ctx.act({ command: "cast", id, how: usable[0].value, origin });
    ctx.pick({ type: "cast", id, origin }, ([how]) => ctx.act({ command: "cast", id, how, origin }));
  }

  function menu(r: Spell) {
    ctx.menu(r.name, spellActions(r));
  }

  /// The spell chosen on a computer: its actions in sight under it.
  let chosen = $state<string | null>(null);
  /// A row by where it is: a favorite shows twice (Favorites and its level).
  /// A tap opens the menu; on a computer a click shows the actions instead.
  const open = (r: Spell, at: string) => (desktop.current ? (chosen = at) : menu(r));

  function spellActions(r: Spell): [string, () => void][] {
    const actions: [string, () => void][] = [];
    if (r.entity !== null) actions.push(["Read it", () => ctx.show(r.entity!)]);
    if (r.castable) actions.push([r.cost ? `Cast (${r.cost})` : r.free ? "Cast (no slot)" : "Cast…", () => cast(r)]);
    if (r.free) actions.push(["Cast with a slot…", () => cast(r, true)]);
    actions.push(["Roll it", () => ctx.roll({ type: "spell", id: r.id, origin: r.origin })]);
    if (r.class_id && r.mark !== "cantrip") {
      actions.push([r.mark === "prepared" ? "Unprepare" : "Prepare", () => ctx.act({ command: "toggle_prepared", class_id: r.class_id, id: r.id })]);
    }
    actions.push([r.favorite ? "No longer a favorite" : "Make it a favorite", () => ctx.act({ command: "toggle_favorite", id: r.id })]);
    if (r.target) actions.push(["Remove", () => ctx.act({ command: "remove_spell", from: r.target, id: r.id })]);
    return actions;
  }
</script>

<div class="spells" bind:clientWidth={tabWidth}>
  {#if !rows.length}
    <p class="muted">No spellcasting yet.</p>
    <p class="dim">Spellcasting classes, subclasses and feats like Magic Initiate add spells here.</p>
  {:else}
    {#if desktop.current}
      {@render casting()}
    {:else}
      <!-- A phone: the tab starts with a section as every tab does. -->
      <Pane title="Spellcasting" class="casting" fold="sheet.spellcasting" section>
        {#snippet aside()}{@render filters()}{/snippet}
        {@render castingPhone()}
      </Pane>
    {/if}
    {#each levels as [k, entries] (k)}
      <button class="level" aria-expanded={!folded.includes(k)} onclick={() => fold(k)}>
        <span class="ltitle">{folded.includes(k) ? "▸" : "▾"} {levelTitle(k)}{#if slotsOf(k)}{" "}<span class="lslots">{slotsOf(k)}</span>{/if}</span>
        <span class="dim">{entries.length}</span>
      </button>
      {#if !folded.includes(k)}
        {#each entries as { r, caster, feature }, i (i)}
          {@const at = `${k}|${r.origin}|${r.id}`}
          <div class="srow">
            {#if r.class_id && r.mark !== "cantrip"}
              <button
                class="prep"
                role="checkbox"
                aria-checked={r.mark === "prepared"}
                aria-label="Prepared: {r.name}"
                style:color={caster?.color}
                onclick={() => ctx.act({ command: "toggle_prepared", class_id: r.class_id, id: r.id })}>{r.mark === "prepared" ? "✓" : ""}</button
              >
            {:else}
              <span class="mark" style:color={caster?.color ?? "var(--magenta)"} title={caster?.name ?? feature ?? ""}>{caster?.tag ?? "◆"}</span>
            {/if}
            <button class="spell" class:dimmed={!castableNow(r)} class:chosen={chosen === at} aria-expanded={desktop.current ? chosen === at : undefined} onclick={() => open(r, at)} oncontextmenu={(e) => (e.preventDefault(), menu(r))}>
              <span class="name">{r.name}{#if r.favorite && !r.in_favorites}<span class="star"> ★</span>{/if}</span>
              <span class="detail dim">{[feature, r.detail].filter(Boolean).join(" · ")}</span>
            </button>
            {#if r.castable}
              <button class="rollbox roll cast" aria-label="Cast {r.name}" title="Cast it" onclick={() => cast(r)}><Icon name="spells" /></button>
            {:else if r.rollable}
              <button class="rollbox roll" aria-label="Roll {r.name}" onclick={() => ctx.roll({ type: "spell", id: r.id, origin: r.origin })}><Icon name="d20" /></button>
            {/if}
            {#if desktop.current}<button class="more" aria-label="More" title="The menu" onclick={() => menu(r)}>⋯</button>{/if}
          </div>
          {#if desktop.current && chosen === at}
            <!-- Everything the spell's menu does, in sight. -->
            <div class="acts" role="dialog" aria-label={r.name}>
              {#each spellActions(r) as [label, run] (label)}<button class="ghost" class:bad={label === "Remove"} onclick={run}>{label}</button>{/each}
            </div>
          {/if}
        {/each}
        {#if !entries.length}<p class="dim none">none yet</p>{/if}
      {/if}
    {/each}
    <div class="adds">
      {#each listed.choices as c (c.key)}<button class="choice" onclick={() => choose(c)}>+ {c.text}</button>{/each}
      {#if casters.length}<button class="ghost add" onclick={addAny}>+ add</button>{/if}
    </div>
    <p class="legend dim">
      {#each casters as c (c.id)}<span><b style:color={c.color}>{c.tag}</b> {c.name}{" · "}</span>{/each}◆ from a feature{" · "}✓ prepared from the spellbook
    </p>
  {/if}
</div>

<!-- A phone: the filters for the Spellcasting title line: All, a button
     per class (its name, or shorter when there is no room), ◆ for the
     spells of feats and other features. -->
{#snippet filters()}
  {#if casters.length > 1 || features}
    <span class="chips" role="group" aria-label="Show">
      <span class="ruler" aria-hidden="true" bind:clientWidth={ch}>0</span>
      <button class="chip" aria-pressed={show === "all"} onclick={() => (show = "all")}>All</button>
      {#each casters as c (c.id)}
        <button class="chip" style:color={c.color} aria-pressed={show === c.id} aria-label={c.name} title={c.name} onclick={() => (show = show === c.id ? "all" : c.id)}
          >{chipLabel(c)}</button
        >
      {/each}
      {#if features}
        <button class="chip feat" aria-pressed={show === "features"} aria-label="Features" title="Feats and other features" onclick={() => (show = show === "features" ? "all" : "features")}>◆</button>
      {/if}
    </span>
  {/if}
{/snippet}

<!-- A phone: one class (or the only one): its numbers on a line, then the
     slots and its counts; all of several: a small table of the classes,
     then the slots. -->
{#snippet castingPhone()}
  <div class="top">
    {#if picked || (casters.length === 1 && show !== "features")}
      {@const c = picked ?? casters[0]}
      <div class="castrow caster">
        <b style:color={c.color}>{c.header.title}</b>
        <span><span class="lbl">DC</span> <b>{c.header.dc}</b></span>
        <span><span class="lbl">attack</span> <b>{c.header.attack}</b></span>
        <span><span class="lbl">up to</span> <b>L{c.header.max_level}</b></span>
      </div>
      <div class="castrow">
        {#each slots as t (t.id)}{@render slot(t)}{/each}
        <span class="counts">
          {#each c.header.counts as [label, have, max] (label)}<span><span class="lbl">{label}</span> <b class:over={have > max}>{have}/{max}</b></span>{/each}
        </span>
      </div>
    {:else}
      {#if show !== "features"}
        <div class="casttable">
          <span></span><span class="lbl">DC</span><span class="lbl">attack</span><span class="lbl">up to</span>
          {#each casters as c (c.id)}
            <span class="caster"><b style:color={c.color}>{c.tag}</b>&#160;{c.header.title}</span>
            <b>{c.header.dc}</b><b>{c.header.attack}</b><b>L{c.header.max_level}</b>
          {/each}
        </div>
      {/if}
      {#if slots.length}<div class="castrow">{#each slots as t (t.id)}{@render slot(t)}{/each}</div>{/if}
    {/if}
  </div>
{/snippet}

{#snippet casting()}
    {#if casters.length > 1 || listed.entries.some((e) => e.feature)}
    <div class="chips" role="group" aria-label="Show" bind:clientWidth={chipsWidth}>
      <span class="ruler" aria-hidden="true" bind:clientWidth={ch}>0</span>
      <button class="chip" aria-pressed={show === "all"} onclick={() => (show = "all")}>All</button>
      {#each casters as c (c.id)}
        <button class="chip" style:color={c.color} aria-pressed={show === c.id} aria-label={c.name} title={c.name} onclick={() => (show = show === c.id ? "all" : c.id)}
          >{chipLabel(c)}</button
        >
      {/each}
    </div>
  {/if}
  <div class="top">
    {#if slots.length}<div class="slots">{#each slots as t (t.id)}{@render slot(t)}{/each}</div>{/if}
    <div class="casters">
      {#each picked ? [picked] : casters as c (c.id)}
        <span class="caster"><b style:color={c.color}>{casters.length > 1 ? c.tag : c.header.title}</b> DC <b class="num">{c.header.dc}</b> <b class="num">{c.header.attack}</b></span>
      {/each}
    </div>
    {#if picked || casters.length === 1}
      {@const c = picked ?? casters[0]}
      <div class="counts dim">
        <span>{c.header.title}, up to level {c.header.max_level}</span>
        {#each c.header.counts as [label, have, max] (label)}<span>{label} <b class="num" class:over={have > max}>{have}/{max}</b></span>{/each}
      </div>
    {/if}
  </div>
{/snippet}

{#snippet slot(t: Tracker)}
  <button class="slot" data-pool={t.name} onclick={() => slotMenu(t)}>
    <span class="label">{t.id === "pact" ? "pact" : `L${slotLevel(t)}`}</span>
    <span class="pips"><span class="acc">{"●".repeat(Math.max(t.left, 0))}</span><span class="used">{"○".repeat(Math.max(t.max - t.left, 0))}</span></span>
    <span class="count">{t.left}/{t.max}</span>
  </button>
{/snippet}

<style>
  .chips {
    position: relative;
    display: flex;
    gap: 1ch;
    overflow: hidden;
    margin-bottom: 0.5em;
  }
  .ruler {
    position: absolute;
    visibility: hidden;
  }
  .chip {
    border: 1px solid var(--border) !important;
    border-radius: 99px;
    padding: 0.15em 1.5ch !important;
    white-space: nowrap;
    flex: none;
    color: var(--gray);
    font: inherit;
    font-size: 0.9em;
  }
  .chip[aria-pressed="true"] {
    background: var(--dark) !important;
    color: var(--fg) !important;
  }
  .top {
    display: flex;
    flex-direction: column;
    gap: 0.15em;
    padding: 0.35em 1.2ch;
    background: var(--card);
    border-radius: var(--radius);
  }
  .slots,
  .casters,
  .counts {
    display: flex;
    flex-wrap: wrap;
    gap: 0 2.5ch;
  }
  .slot {
    display: inline-flex;
    gap: 0.6ch;
    padding: 0 !important;
    font: inherit;
    color: inherit;
  }
  .slot .count {
    font-size: 0.8em;
    align-self: center;
    color: var(--dim);
  }
  .slot .label {
    color: var(--gray);
  }
  .pips .acc {
    color: var(--accent);
  }
  .pips .used {
    color: var(--dim);
  }
  .caster {
    white-space: nowrap;
  }
  .counts {
    font-size: 0.9em;
  }
  .num {
    color: var(--fg);
  }
  .level {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 2ch;
    margin: 0.9em 0 0.2em;
    padding: 0 0 0.15em !important;
    border-bottom: 1px solid rgb(65 72 104 / 0.6) !important;
    font: inherit;
  }
  .lslots {
    margin-left: 1.5ch;
    color: var(--accent);
    font-weight: 400;
    font-size: 0.85em;
  }
  .dimmed .name {
    color: var(--dim);
  }
  .adds {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.4em 2ch;
    margin-top: 1em;
  }
  .ltitle {
    flex: 1;
    text-align: left;
    color: var(--gray);
    font-weight: 700;
  }
  .slot .pips {
    letter-spacing: 0.15ch;
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
  .spells {
    padding-top: 0.6em;
  }
  :where(button:not(.ghost):not(.primary)) {
    background: none;
    border: 0;
    text-align: left;
    cursor: pointer;
  }
  .add {
    padding: 0 1.5ch;
  }
  .over {
    color: var(--red);
  }
  .spell {
    width: 100%;
    display: flex;
    align-items: baseline;
    gap: 1.5ch;
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
    flex: 0 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .star {
    color: var(--accent);
  }
  .detail {
    flex: 1 1 0;
    min-width: 4ch;
    font-size: 0.85em;
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
  /* A computer: a click shows the spell's actions under it (⋯ and a right
     click still open its menu). */
  @media (pointer: fine) and (min-width: 761px) {
    .srow {
      grid-template-columns: 2rem minmax(0, 1fr) auto auto;
    }
    .spell.chosen .name {
      color: var(--accent);
    }
    .more {
      background: none;
      border: 0;
      color: var(--gray);
      cursor: pointer;
      font: inherit;
      padding: 0 0.5rem;
    }
    .acts {
      display: flex;
      flex-wrap: wrap;
      gap: 0.4rem;
      margin: 0.2rem 0 0.6rem 2.6rem;
    }
    .acts .ghost {
      min-height: 2rem;
      padding: 0 0.75rem;
      font-size: 0.9em;
    }
    .acts .bad {
      color: var(--red);
    }
  }
  @media (max-width: 760px) {
    .prep {
      width: 32px;
      height: 32px;
    }
    .spell {
      min-height: 40px;
      align-items: center;
    }
    .srow {
      grid-template-columns: 3ch minmax(0, 1fr) auto;
      border-bottom: 1px solid rgb(65 72 104 / 0.3);
    }
    .roll {
      padding: 0.15em 1ch !important;
    }
    .choice,
    .add {
      min-height: 44px;
    }
  }
  /* Phones and tablets: a level's heading as every section's title (see
     Pane): gold, over a gold line; the filters boxy buttons, the
     slots and the numbers plain lines under the Spellcasting title. */
  @media not ((pointer: fine) and (min-width: 761px)) {
    .chips {
      gap: 4px;
      margin: 0;
      overflow: visible;
    }
    .chip {
      border-radius: 6px;
      min-height: 24px;
      min-width: 26px;
      padding: 0 6px !important;
      font-size: 0.85em;
      text-align: center !important;
    }
    .chip.feat {
      color: var(--magenta);
    }
    .chip[aria-pressed="true"] {
      border-color: var(--accent) !important;
    }
    .lbl {
      color: var(--gray);
      font-size: 0.85em;
    }
    .castrow {
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      gap: 0 2ch;
      min-height: 34px;
      border-bottom: 1px solid var(--raised);
    }
    .castrow .counts {
      display: contents;
    }
    .casttable {
      display: grid;
      grid-template-columns: minmax(0, 1fr) 5ch 7ch 6ch;
      align-items: center;
    }
    .casttable > * {
      min-height: 30px;
      display: flex;
      align-items: center;
      border-bottom: 1px solid var(--raised);
    }
    .casttable > :not(.caster) {
      justify-content: center;
    }
    .casttable > :nth-child(-n + 4) {
      min-height: 22px;
      border: none;
    }
    .casttable .caster {
      white-space: nowrap;
      overflow: hidden;
    }
    .top {
      background: none;
      padding: 0;
    }
    .level {
      min-height: 32px;
      margin: 1em 0 0.3em;
      border-bottom: 1px solid rgb(224 175 104 / 0.55) !important;
    }
    .ltitle {
      color: var(--accent);
      font-size: 1.15em;
      line-height: 1.3;
    }
    .lslots {
      font-size: 0.75em;
    }
  }
</style>
