<!-- Several spells at once, by level: a heading per level with how many can
     be chosen there; the spells already had ticked (untick one to free a
     pick: the rules let a spell be removed at any time); the spell under
     the cursor read beside the list (on a phone, under its row); Confirm
     adds and removes them all. Only what can be chosen now is shown (the
     levels the class can take, the groups with room); a class's list can
     widen to all spells. Keys: the arrows move, Space ticks, Enter
     confirms. -->
<script lang="ts">
  import { onMount, tick } from "svelte";
  import * as api from "./api";
  import Blocks from "./Blocks.svelte";
  import Modal from "./Modal.svelte";
  import { desktop } from "./screen";

  let {
    wanted,
    options,
    onconfirm,
    onclose,
  }: {
    wanted: api.Wanted;
    options: api.Options;
    onconfirm: (adds: string[], options: api.Options, removes: api.RemovedSpell[]) => void;
    onclose: () => void;
  } = $props();

  // Its own copy: the switch to all spells replaces it.
  let shown: api.Options = $state((() => options)());
  let all = $state(false);
  let filter = $state("");
  let cursor = $state(0);
  let error = $state("");
  let list: HTMLElement | undefined = $state();
  let input: HTMLInputElement | undefined = $state();

  type Item = { choice: api.Choice; level: number; how: string | null };
  const lists = $derived(shown.spells);
  /// Every spell of the list: those had, then the rest.
  const items = $derived.by((): Item[] => {
    const had = (lists?.had ?? []).map((h) => ({ choice: h.choice, level: h.level, how: h.how }));
    const rest = shown.choices.map((c, i) => ({ choice: c, level: lists?.levels[i] ?? 1, how: null }));
    return [...had, ...rest];
  });
  /// Ticked: the spells had, at first.
  let ticked: string[] = $state((() => (options.spells?.had ?? []).map((h) => h.choice.value))());
  const had = $derived(new Set((lists?.had ?? []).map((h) => h.choice.value)));

  const counts = $derived(lists ? { cantrips: lists.cantrips, spells: lists.spells } : { cantrips: [0, 0], spells: [0, Math.max(1, shown.left)] });
  const levelOf = (v: string) => items.find((i) => i.choice.value === v)?.level ?? 1;
  /// Room left: cantrips, and levelled spells (one count for every level).
  const left = $derived({
    cantrips: counts.cantrips[1] - ticked.filter((v) => levelOf(v) === 0).length,
    spells: counts.spells[1] - ticked.filter((v) => levelOf(v) > 0).length,
  });
  const room = (level: number) => (level === 0 ? left.cantrips : left.spells);

  /// What can be chosen now: the groups with room (cantrips only when
  /// their count grew), at the levels the class takes; all of them when
  /// none has room (to swap), or with all spells.
  const grows = $derived({ cantrips: counts.cantrips[1] > counts.cantrips[0], spells: counts.spells[1] > counts.spells[0] });
  const showing = (level: number) => {
    if (all) return true;
    if (lists && level > lists.max_level) return false;
    if (!grows.cantrips && !grows.spells) return true;
    return level === 0 ? grows.cantrips : grows.spells;
  };
  type Group = { level: number; items: Item[] };
  const groups = $derived.by((): Group[] => {
    const words = filter.toLowerCase().split(/\s+/).filter(Boolean);
    const match = (i: Item) => words.every((w) => `${i.choice.label} ${i.choice.hint}`.toLowerCase().includes(w));
    const by = new Map<number, Item[]>();
    for (const i of items) if (showing(i.level) && match(i)) by.set(i.level, [...(by.get(i.level) ?? []), i]);
    return [...by.entries()].sort(([a], [b]) => a - b).map(([level, its]) => ({ level, items: its.sort((a, b) => a.choice.label.localeCompare(b.choice.label)) }));
  });
  const rows = $derived(groups.flatMap((g) => g.items));
  const current = $derived(rows[cursor] ?? null);
  const levelled = $derived(groups.filter((g) => g.level > 0).length);

  /// A group's heading: its level, and how many can be chosen there.
  function heading(level: number): string {
    const name = level === 0 ? "Cantrips" : `Level ${level}`;
    const n = room(level);
    const above = lists && level > lists.max_level ? " · above what you can cast" : "";
    if (n <= 0) return `${name} · none left${above}`;
    if (level > 0 && levelled > 1) return `${name} · ${n} left (shared)${above}`;
    return `${name} · choose ${n}${above}`;
  }

  const adds = $derived(ticked.filter((v) => !had.has(v)));
  const removes = $derived((lists?.had ?? []).filter((h) => !ticked.includes(h.choice.value)).map((h) => ({ id: h.choice.value, how: h.how })));
  const changed = $derived(adds.length > 0 || removes.length > 0);

  /// The spell under the cursor, read.
  let entry: api.Entry | null = $state(null);
  let asked = 0;
  $effect(() => {
    const c = current;
    const ask = ++asked;
    entry = null;
    if (!c) return;
    (async () => {
      try {
        const id = await api.follow({ tag: "spell", args: c.choice.value.replace(/^spell:/, "") } as api.Link);
        if (id !== null && ask === asked) entry = await api.entry(id);
      } catch {
        // The list is still there to choose from.
      }
    })();
  });

  function toggle(i: Item) {
    const v = i.choice.value;
    if (ticked.includes(v)) ticked = ticked.filter((x) => x !== v);
    else if (room(i.level) > 0) ticked = [...ticked, v];
  }

  async function select(i: number) {
    cursor = Math.max(0, Math.min(rows.length - 1, i));
    await tick();
    list?.querySelector(`[data-i="${cursor}"]`)?.scrollIntoView({ block: "nearest" });
  }

  function confirm() {
    if (changed) onconfirm(adds, shown, removes);
  }

  async function widen(on: boolean) {
    all = on;
    error = "";
    try {
      shown = await api.options({ ...(wanted as Extract<api.Wanted, { type: "class_spells" }>), all: on });
      cursor = 0;
    } catch (e) {
      error = String(e);
    }
  }

  function onkey(e: KeyboardEvent) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      select(cursor + 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      select(cursor - 1);
    } else if (e.key === " " && (!filter || document.activeElement !== input)) {
      e.preventDefault();
      if (current) toggle(current);
    } else if (e.key === "Enter") {
      e.preventDefault();
      confirm();
    }
  }

  onMount(() => {
    if (desktop.current) input?.focus();
  });
</script>

{#snippet preview()}
  {#if entry}
    <div class="ptitle">{entry.name}</div>
    <div class="dim psub">{entry.kind} · {entry.source}</div>
    <Blocks blocks={entry.blocks} onlink={() => {}} />
  {:else if current}
    <div class="dim">{current.choice.label}</div>
  {/if}
{/snippet}

<Modal title={shown.title} {onclose} wide>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="spellpick" onkeydown={onkey}>
    <div class="top">
      <label class="filter"
        ><span>❯</span><input bind:this={input} bind:value={filter} oninput={() => (cursor = 0)} placeholder="type to filter" spellcheck="false" autocomplete="off" /></label
      >
      <span class="count" aria-live="polite">{adds.length} new{#if removes.length} · {removes.length} given up{/if}</span>
      {#if wanted.type === "class_spells"}
        <label class="all" title="Every spell, not only this class's list: for what the rules do not list">
          <input type="checkbox" role="switch" checked={all} onchange={(e) => widen((e.currentTarget as HTMLInputElement).checked)} />
          <span>All spells <span class="dim">(not only the class's list)</span></span>
        </label>
      {/if}
    </div>
    {#if all}<p class="wide dim">All spells: every spell of the campaign's books, not only the class's list.</p>{/if}
    <div class="panes">
      <ul class="rows" bind:this={list} aria-label="Spells">
        {#each groups as g (g.level)}
          <li class="group" data-level={g.level}><span>{heading(g.level)}</span></li>
          {#each g.items as it (it.choice.value)}
            {@const i = rows.indexOf(it)}
            {@const on = ticked.includes(it.choice.value)}
            <li data-i={i} class="item" class:cursor={i === cursor} class:on>
              <input type="checkbox" checked={on} disabled={!on && room(it.level) <= 0} aria-label="Choose {it.choice.label}" onchange={() => toggle(it)} />
              <button class="row" aria-current={i === cursor} onclick={() => select(i)}>
                <span class="label">{it.choice.label}</span>
                {#if it.how}<span class="had">{it.how}</span>{/if}
                <span class="hint dim">{it.choice.hint}</span>
              </button>
            </li>
            {#if !desktop.current && i === cursor}<li class="inline">{@render preview()}</li>{/if}
          {/each}
        {/each}
        {#if !rows.length}<li class="dim none">Nothing to choose here.</li>{/if}
      </ul>
      {#if desktop.current}<div class="preview">{@render preview()}</div>{/if}
    </div>
    {#if error}<p class="err">{error}</p>{/if}
    <div class="actions">
      <span class="dim keys">↑↓ move · space ticks · enter confirms</span>
      <button class="ghost" onclick={onclose}>Cancel</button>
      <button class="primary" disabled={!changed} onclick={confirm}>Confirm</button>
    </div>
  </div>
</Modal>

<style>
  .spellpick {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    min-height: 0;
    height: min(70vh, 44rem);
  }
  .top {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5rem 1rem;
  }
  .filter {
    flex: 1 1 14rem;
    display: flex;
    gap: 1ch;
    align-items: center;
    border-bottom: 1px solid var(--border);
    padding-bottom: 0.3em;
  }
  .filter span {
    color: var(--accent);
  }
  .filter input {
    flex: 1;
    min-width: 0;
    background: none;
    border: 0;
    outline: none;
  }
  .count {
    white-space: nowrap;
    color: var(--gray);
  }
  .all {
    display: inline-flex;
    align-items: center;
    gap: 0.5ch;
    white-space: nowrap;
    cursor: pointer;
    padding: 0.15rem 0.6rem;
    border: 1px solid var(--border);
    border-radius: 1rem;
  }
  .all:has(input:checked) {
    border-color: var(--cyan);
    color: var(--cyan);
  }
  .all input {
    accent-color: var(--cyan);
  }
  .wide {
    margin: 0;
    font-size: 0.88em;
  }
  .panes {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1.2fr);
    gap: 1rem;
  }
  .rows {
    list-style: none;
    margin: 0;
    padding: 0;
    overflow-y: auto;
    min-height: 0;
  }
  /* A level's heading: a line with the level and its count. */
  .rows li.group {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    margin: 0.7rem 0 0.25rem;
    font-size: 0.85em;
    font-weight: 600;
    color: var(--accent);
    white-space: nowrap;
  }
  .rows li.group:first-child {
    margin-top: 0;
  }
  .rows li.group::after {
    content: "";
    flex: 1;
    border-top: 1px solid var(--edge);
  }
  .rows li.item {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    border-radius: 0.45rem;
    padding: 0 0.3rem;
  }
  .rows li.cursor {
    background: var(--selected);
  }
  .rows li.on .label {
    color: var(--accent);
    font-weight: 600;
  }
  .rows input[type="checkbox"] {
    flex: none;
    width: 1.05em;
    height: 1.05em;
    accent-color: var(--accent);
    cursor: pointer;
  }
  .row {
    flex: 1;
    min-width: 0;
    display: flex;
    gap: 1ch;
    align-items: baseline;
    background: none;
    border: 0;
    padding: 0.35rem 0.2rem;
    text-align: left;
    cursor: pointer;
  }
  /* The name keeps its room; the details after it give way first. */
  .label {
    flex: none;
    max-width: 75%;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .had {
    flex: none;
    font-size: 0.78em;
    color: var(--green);
    border: 1px solid rgb(158 206 106 / 0.5);
    border-radius: 0.6rem;
    padding: 0 0.45rem;
    white-space: nowrap;
  }
  .hint {
    flex: 0 100 auto;
    min-width: 0;
    margin-left: auto;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font-size: 0.85em;
  }
  .preview {
    min-height: 0;
    overflow-y: auto;
    border-left: 1px solid var(--edge);
    padding-left: 1rem;
  }
  .ptitle {
    color: var(--accent);
    font-weight: 700;
  }
  .psub {
    margin-bottom: 0.6rem;
    font-size: 0.88em;
  }
  .rows li.inline {
    display: block;
    padding: 0.4rem 0.6rem 0.8rem 2rem;
    font-size: 0.92em;
  }
  .none {
    padding: 0.5rem;
  }
  .err {
    color: var(--red);
    margin: 0;
  }
  .actions {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 1ch;
  }
  .keys {
    margin-right: auto;
    font-size: 0.85em;
  }
  /* A phone: one column, the spell read under its row. */
  @media (max-width: 760px), (pointer: coarse) {
    .spellpick {
      height: auto;
      max-height: 75vh;
    }
    .panes {
      grid-template-columns: minmax(0, 1fr);
    }
    .keys {
      display: none;
    }
    .rows li.item {
      min-height: 44px;
    }
    .rows input[type="checkbox"] {
      width: 1.4em;
      height: 1.4em;
    }
    .actions button {
      flex: 1;
      min-height: 44px;
    }
  }
</style>
