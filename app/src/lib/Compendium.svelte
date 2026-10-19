<!-- The compendium: search with facets, results, and the entry. On a
     narrow screen the entry replaces the list until you go back. -->
<script lang="ts">
  import { onMount, tick } from "svelte";
  import * as api from "./api";
  import { GLYPH } from "./api";
  import Blocks from "./Blocks.svelte";
  import Pane from "./Pane.svelte";

  let category = $state(0);
  let query = $state("");
  let found: api.Found | null = $state(null);
  let selected = $state(0);
  let entry: api.Entry | null = $state(null);
  /// Entries left by following links.
  let back: api.Entry[] = $state([]);
  let error = $state("");
  /// On a phone: showing the entry instead of the list.
  let reading = $state(false);
  let searching = $state(false);

  let input: HTMLInputElement;
  let list: HTMLElement;
  let detail: HTMLElement | undefined = $state();
  let asked = 0;

  async function run() {
    const ask = ++asked;
    try {
      const f = await api.search(category, query);
      if (ask !== asked) return;
      found = f;
      selected = 0;
      await show(f.rows[0]?.id, false);
    } catch (e) {
      error = String(e);
    }
  }

  async function show(id: number | undefined, keep: boolean) {
    if (id === undefined) {
      entry = null;
      return;
    }
    const e = await api.entry(id);
    if (keep && entry) back = [...back, entry];
    else if (!keep) back = [];
    entry = e;
    await tick();
    detail?.scrollTo({ top: 0 });
  }

  function pick(i: number, open = false) {
    if (!found || i < 0 || i >= found.rows.length) return;
    selected = i;
    show(found.rows[i].id, false);
    list?.querySelector(`[data-i="${i}"]`)?.scrollIntoView({ block: "nearest" });
    if (open) reading = true;
  }

  async function onlink(link: api.Link) {
    const id = await api.follow(link);
    if (id === null) {
      error = `Not in the library: ${link.args.split("|")[0]}`;
      return;
    }
    error = "";
    await show(id, true);
  }

  function goBack() {
    const prev = back.at(-1);
    if (prev) {
      back = back.slice(0, -1);
      entry = prev;
    } else {
      reading = false;
    }
  }

  function setCategory(i: number) {
    const n = found?.categories.length ?? 1;
    category = (i + n) % n;
    run();
  }

  function complete() {
    const first = found?.suggestions[0];
    if (!first) return false;
    const start = query.search(/\S+$/);
    const token = query.slice(start);
    const head = token.slice(0, token.match(/^!*/)![0].length + 1);
    query = query.slice(0, start) + head + first[0].replaceAll(" ", "_") + " ";
    run();
    return true;
  }

  function onkey(e: KeyboardEvent) {
    const typing = document.activeElement === input;
    if (e.key === "Tab") {
      e.preventDefault();
      if (typing && !e.shiftKey && complete()) return;
      setCategory(category + (e.shiftKey ? -1 : 1));
    } else if (e.key === "/" && !typing) {
      e.preventDefault();
      input.focus();
    } else if (e.key === "ArrowDown" || (e.key === "j" && !typing)) {
      e.preventDefault();
      pick(selected + 1);
    } else if (e.key === "ArrowUp" || (e.key === "k" && !typing)) {
      e.preventDefault();
      pick(selected - 1);
    } else if (e.key === "Enter" && typing) {
      input.blur();
    } else if (e.key === "Escape") {
      if (back.length || reading) goBack();
      else if (typing) input.blur();
      else if (query) {
        query = "";
        run();
      }
    } else if (e.key === "Backspace" && !typing && back.length) {
      goBack();
    }
  }

  onMount(run);

  const FACET: Record<string, string> = { "#": "var(--green)", "*": "var(--red)", "^": "var(--magenta)" };
</script>

<svelte:window onkeydown={onkey} />

<div class="categories">
  {#each found?.categories ?? [] as [label, count], i (label)}
    <button class:active={i === category} onclick={() => setCategory(i)}>{label} <span class="count">{count}</span></button>
  {/each}
</div>

<div class="split" class:reading>
  <Pane title="Browse" focused={searching || !reading} footer={found ? `${found.total} results` : ""} class="browse">
    <label class="prompt">
      <span class:on={searching}>❯</span>
      <input
        bind:this={input}
        bind:value={query}
        oninput={run}
        onfocus={() => (searching = true)}
        onblur={() => (searching = false)}
        placeholder="search  /"
        spellcheck="false"
        autocomplete="off"
        autocapitalize="off"
      />
    </label>
    <div class="hint">
      {#if found?.suggestions.length}
        <button class="keycap" onclick={complete}>tab</button>
        {#each found.suggestions as [value, n], i (value)}
          {#if i > 0}<span class="dim"> · </span>{/if}<span class={i === 0 ? "first" : "muted"}>{value}</span><span class="dim"> {n}</span>
        {/each}
      {:else}
        <span style:color="var(--blue)">@</span><span class="dim">{"source "}</span>
        {#each found?.hints ?? [] as [p, what] (p)}<span style:color={FACET[p]}>{p}</span><span class="dim">{what + " "}</span>{/each}
        <span style:color="var(--yellow)">"</span><span class="dim">{'text" '}</span><span style:color="var(--accent)">!</span><span class="dim">not</span>
      {/if}
    </div>
    <div class="rule"></div>
    <ul class="rows" bind:this={list}>
      {#each found?.rows ?? [] as row, i (row.id)}
        <li data-i={i}>
          <button class:selected={i === selected} onclick={() => pick(i, true)}>
            <span class="g-{row.group}">{GLYPH[row.group]}</span>
            <span class="name">{row.name}</span>
            <span class="dim src">{row.source}</span>
          </button>
        </li>
      {/each}
      {#if found && found.total > found.rows.length}
        <li class="dim more">… {found.total - found.rows.length} more: narrow the search</li>
      {/if}
      {#if found && !found.total}<li class="dim more">Nothing matches.</li>{/if}
    </ul>
  </Pane>

  <Pane focused={reading} class="detail">
    {#if entry}
      <div class="head">
        {#if reading || back.length}<button class="back" onclick={goBack}>‹ back</button>{/if}
        <h1><span class="g-{entry.group}">{GLYPH[entry.group]}</span> {entry.name}</h1>
        <div class="sub">
          <span class="g-{entry.group}">{entry.kind}</span><span class="dim">{`  ·  ${entry.source}${entry.page ? ` · p. ${entry.page}` : ""}`}</span>
        </div>
      </div>
      <div class="doc" bind:this={detail}>
        <Blocks blocks={entry.blocks} {onlink} />
      </div>
    {:else}
      <div class="empty dim">Nothing matches.</div>
    {/if}
  </Pane>
</div>
{#if error}<div class="error">{error}</div>{/if}

<style>
  .categories {
    display: flex;
    gap: 2ch;
    padding: 0 1ch 0.75em;
    overflow-x: auto;
    scrollbar-width: none;
    flex-shrink: 0;
  }
  .categories button {
    background: none;
    border: 0;
    padding: 0 0.5ch;
    color: var(--gray);
    white-space: nowrap;
    cursor: pointer;
  }
  .categories .count {
    color: var(--dim);
  }
  .categories button.active {
    background: var(--cyan);
    color: var(--black);
    font-weight: 700;
  }
  .categories button.active .count {
    color: var(--black);
  }
  .split {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(36ch, 56ch) 1fr;
    gap: 0 2ch;
    padding-top: 0.4em;
  }
  .split :global(.browse),
  .split :global(.detail) {
    min-height: 0;
  }
  .prompt {
    display: flex;
    gap: 1ch;
  }
  .prompt span {
    color: var(--dim);
  }
  .prompt span.on {
    color: var(--accent);
  }
  .prompt input {
    flex: 1;
    min-width: 0;
    background: none;
    border: 0;
    outline: none;
    padding: 0;
    caret-color: var(--accent);
  }
  .prompt input::placeholder {
    color: var(--dim);
  }
  .hint {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .hint .keycap {
    border: 0;
    margin-right: 1ch;
    cursor: pointer;
  }
  .hint .first {
    color: var(--green);
  }
  .rule {
    border-top: 1px solid var(--border);
    margin: 0.25em 0 0.4em;
  }
  .rows {
    list-style: none;
    margin: 0;
    padding: 0;
    overflow-y: auto;
    flex: 1;
    min-height: 0;
  }
  .rows button {
    width: 100%;
    display: flex;
    gap: 1ch;
    background: none;
    border: 0;
    border-left: 2px solid transparent;
    padding: 0 0.5ch;
    text-align: left;
    cursor: pointer;
  }
  .rows button.selected {
    background: var(--selected);
    border-left-color: var(--accent);
    font-weight: 700;
  }
  .rows button.selected .src {
    color: var(--gray);
  }
  .name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .more {
    padding: 0.5em 1ch;
  }
  .head h1 {
    font-size: 1em;
    margin: 0;
    color: var(--accent);
  }
  .head .sub {
    padding-bottom: 0.5em;
    border-bottom: 1px solid var(--border);
    margin-bottom: 0.75em;
  }
  .back {
    background: none;
    border: 0;
    padding: 0;
    color: var(--link);
    cursor: pointer;
    margin-bottom: 0.25em;
  }
  .doc {
    overflow-y: auto;
    flex: 1;
    min-height: 0;
    max-width: 96ch;
    padding-right: 1ch;
  }
  .empty {
    margin: auto;
  }
  .error {
    color: var(--red);
    padding: 0.25em 1ch 0;
  }

  /* Phones: one pane at a time. */
  @media (max-width: 760px) {
    .split {
      grid-template-columns: 1fr;
    }
    .split :global(.detail) {
      display: none;
    }
    .split.reading :global(.browse) {
      display: none;
    }
    .split.reading :global(.detail) {
      display: flex;
    }
    .rows button {
      padding: 0.35em 0.5ch;
    }
  }
</style>
