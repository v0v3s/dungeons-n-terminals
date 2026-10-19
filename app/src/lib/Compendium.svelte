<!-- The compendium: search with facets, results, and the entry. On a narrow
     screen the entry replaces the list until you go back; on touch screens
     the search gets a row of extra keys and completions as chips. -->
<script lang="ts">
  import { onMount, tick, untrack } from "svelte";
  import * as api from "./api";
  import { GLYPH } from "./api";
  import Blocks from "./Blocks.svelte";
  import Pane from "./Pane.svelte";
  import SourcesEditor from "./SourcesEditor.svelte";

  let {
    touch = false,
    narrow = false,
    reading = $bindable(null),
    campaign = null,
    onstatus,
    oncampaigns,
  }: {
    touch?: boolean;
    narrow?: boolean;
    reading?: string | null;
    /// The active campaign, whose sources the compendium shows.
    campaign?: api.CampaignInfo | null;
    onstatus: (s: api.Status) => void;
    /// Go to the campaigns (to create one).
    oncampaigns: () => void;
  } = $props();

  let editingSources = $state(false);

  let category = $state(0);
  let query = $state("");
  let found: api.Found | null = $state(null);
  let selected = $state(0);
  let entry: api.Entry | null = $state(null);
  /// Entries left by following links.
  let back: api.Entry[] = $state([]);
  let error = $state("");
  /// Showing the entry instead of the list (narrow screens).
  let open = $state(false);
  let searching = $state(false);

  let input: HTMLInputElement;
  let list: HTMLElement;
  let detail: HTMLElement | undefined = $state();
  let asked = 0;

  // The app bar shows the open entry's name on a phone.
  $effect(() => {
    reading = open && entry ? entry.name : null;
  });

  async function run() {
    const ask = ++asked;
    try {
      const f = await api.search(category, query);
      if (ask !== asked) return;
      found = f;
      selected = 0;
      error = "";
      // On a phone nothing opens by itself; elsewhere the first result
      // shows (or is ready to open with Enter).
      if (!(narrow && touch)) await show(f.rows[0]?.id, false);
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

  function pick(i: number, tap = false) {
    if (!found || i < 0 || i >= found.rows.length) return;
    selected = i;
    show(found.rows[i].id, false);
    list?.querySelector(`[data-i="${i}"]`)?.scrollIntoView({ block: "nearest" });
    if (tap && narrow) open = true;
  }

  async function onlink(link: api.Link) {
    const id = await api.follow(link);
    if (id === null) {
      error = `Not in the library: ${link.args.split("|")[0]}`;
      return;
    }
    error = "";
    await show(id, true);
    if (narrow) open = true;
  }

  function goBack() {
    const prev = back.at(-1);
    if (prev) {
      back = back.slice(0, -1);
      entry = prev;
    } else {
      open = false;
    }
  }

  /// Android's back (and the app bar's): one step back, if there is one.
  export function stepBack(): boolean {
    if (!back.length && !open) return false;
    goBack();
    return true;
  }

  function setCategory(i: number) {
    const n = found?.categories.length ?? 1;
    category = (i + n) % n;
    run();
  }

  /// Complete the facet being typed (with the first completion by default).
  function complete(value?: string) {
    const v = value ?? found?.suggestions[0]?.[0];
    if (!v) return false;
    const start = query.search(/\S+$/);
    const token = query.slice(start);
    const head = token.slice(0, token.match(/^!*/)![0].length + 1);
    query = query.slice(0, start) + head + v.replaceAll(" ", "_") + " ";
    run();
    input?.focus();
    return true;
  }

  /// Type a character at the cursor (the extra keys).
  function insert(text: string) {
    const at = input.selectionStart ?? query.length;
    const end = input.selectionEnd ?? at;
    // Facets start a word.
    const pad = /[@#*^!"]/.test(text) && at > 0 && query[at - 1] !== " " ? " " : "";
    query = query.slice(0, at) + pad + text + query.slice(end);
    run();
    tick().then(() => {
      input.focus();
      const pos = at + pad.length + text.length;
      input.setSelectionRange(pos, pos);
    });
  }

  function clear() {
    query = "";
    run();
    input?.focus();
  }

  function onkey(e: KeyboardEvent) {
    if (touch) return;
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
      if (narrow) open = true;
    } else if (e.key === "Escape") {
      if (back.length || open) goBack();
      else if (typing) input.blur();
      else if (query) clear();
    } else if (e.key === "Backspace" && !typing && back.length) {
      goBack();
    }
  }

  /// Whether the on-screen keyboard is up: the page is clearly shorter
  /// than it has been (the Android app shrinks it by the keyboard). When it
  /// goes down, typing is over.
  let keyboard = $state(false);
  onMount(() => {
    if (!touch) return;
    let full = window.innerHeight;
    const check = () => {
      full = Math.max(full, window.innerHeight);
      const up = window.innerHeight < full - 150;
      if (keyboard && !up) input?.blur();
      keyboard = up;
    };
    window.addEventListener("resize", check);
    return () => window.removeEventListener("resize", check);
  });

  // Search again when the campaign's sources change (and at the start).
  $effect(() => {
    void campaign?.sources.join();
    void campaign?.show_reprinted;
    untrack(run);
  });

  const sourcesLabel = $derived(
    !campaign ? "all sources" : !campaign.sources.length ? "all sources" : campaign.sources.length > 4 ? `${campaign.sources.length} sources` : campaign.sources.join(" "),
  );

  const FACET: Record<string, string> = { "#": "var(--green)", "*": "var(--red)", "^": "var(--magenta)" };
  const KEYS = ["@", "#", "*", "^", '"', "!"];
</script>

<svelte:window onkeydown={onkey} />

{#snippet hints()}
  {#if found?.suggestions.length && !touch}
    <button class="keycap tabkey" onclick={() => complete()}>tab</button>
    {#each found.suggestions as [value, n], i (value)}
      {#if i > 0}<span class="dim">{" · "}</span>{/if}<span class={i === 0 ? "first" : "muted"}>{value}</span><span class="dim">{" " + n}</span>
    {/each}
  {:else}
    <span style:color="var(--blue)">@</span><span class="dim">{"source "}</span>
    {#each found?.hints ?? [] as [p, what] (p)}<span style:color={FACET[p]}>{p}</span><span class="dim">{what + " "}</span>{/each}
    <span style:color="var(--yellow)">"</span><span class="dim">{'text" '}</span><span style:color="var(--accent)">!</span><span class="dim">not</span>
  {/if}
{/snippet}

{#snippet searchbox()}
  <label class="prompt">
    <span class:on={searching}>❯</span>
    <input
      bind:this={input}
      bind:value={query}
      oninput={run}
      onfocus={() => (searching = true)}
      onblur={() => (searching = false)}
      placeholder={touch ? "search the compendium" : "search  /"}
      spellcheck="false"
      autocomplete="off"
      autocapitalize="off"
      enterkeyhint="search"
    />
    {#if touch && query}<button class="clear" aria-label="Clear" onclick={clear}>✕</button>{/if}
  </label>
{/snippet}

{#snippet rows()}
  <ul class="rows" bind:this={list}>
    {#each found?.rows ?? [] as row, i (row.id)}
      <li data-i={i}>
        <button class:selected={i === selected && !(narrow && touch)} onclick={() => pick(i, true)}>
          <span class="glyph g-{row.group}">{GLYPH[row.group]}</span>
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
{/snippet}

{#snippet sourcesButton()}
  <button class="sources" title="The books the compendium shows" onclick={() => (campaign ? (editingSources = true) : oncampaigns())}>
    <span class="at">@</span> {sourcesLabel} <span class="dim">▾</span>
  </button>
{/snippet}

{#snippet categories()}
  <div class="categories no-bar">
    {#each found?.categories ?? [] as [label, count], i (label)}
      <button class:active={i === category} onclick={() => setCategory(i)}>{label} <span class="count">{count}</span></button>
    {/each}
  </div>
{/snippet}

<div class="compendium" class:touch class:narrow class:open>
  {#if touch}
    <!-- Touch: search on top, extra keys while typing, then chips. -->
    <div class="search-area">
      <Pane title="search" focused={searching} class="searchpane">
        {@render searchbox()}
      </Pane>
      {#if searching && keyboard}
        <div class="extra no-bar">
          {#each KEYS as k (k)}
            <button class="key" onmousedown={(e) => e.preventDefault()} onclick={() => insert(k)}>{k}</button>
          {/each}
          <button class="key wide" onmousedown={(e) => e.preventDefault()} onclick={() => complete()} disabled={!found?.suggestions.length}>tab</button>
        </div>
      {/if}
      <div class="bar">
      {#if found?.suggestions.length}
        <div class="chips no-bar">
          {#each found.suggestions as [value, n] (value)}
            <button class="chip" onmousedown={(e) => e.preventDefault()} onclick={() => complete(value)}>{value} <span class="dim">{n}</span></button>
          {/each}
        </div>
      {:else}
        {@render categories()}
      {/if}
      {@render sourcesButton()}
      </div>
    </div>
  {:else}
    <div class="bar">
      {@render categories()}
      {@render sourcesButton()}
    </div>
  {/if}

  <div class="split">
    <Pane
      title={touch ? `${found?.total ?? 0} results` : "Browse"}
      focused={!touch && (searching || !open)}
      footer={!touch && found ? `${found.total} results` : ""}
      class="browse"
    >
      {#if !touch}
        {@render searchbox()}
        <div class="hint">{@render hints()}</div>
        <div class="rule"></div>
      {/if}
      {@render rows()}
    </Pane>

    <Pane focused={open} class="detail">
      {#if entry}
        <div class="head">
          {#if !touch && (open || back.length)}<button class="back" onclick={goBack}>‹ back</button>{/if}
          {#if touch && back.length && !narrow}<button class="back" onclick={goBack}>‹ {back.at(-1)?.name}</button>{/if}
          <h1><span class="g-{entry.group}">{GLYPH[entry.group]}</span> {entry.name}</h1>
          <div class="sub">
            <span class="g-{entry.group}">{entry.kind}</span><span class="dim">{`  ·  ${entry.source}${entry.page ? ` · p. ${entry.page}` : ""}`}</span>
          </div>
        </div>
        <div class="doc" bind:this={detail}>
          <Blocks blocks={entry.blocks} {onlink} />
        </div>
      {:else}
        <div class="empty dim">{narrow ? "" : "Nothing matches."}</div>
      {/if}
    </Pane>
  </div>
  {#if error}<div class="error">{error}</div>{/if}
</div>

{#if editingSources && campaign}
  <SourcesEditor {campaign} onsave={onstatus} onclose={() => (editingSources = false)} />
{/if}

<style>
  .compendium {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .bar {
    display: flex;
    align-items: baseline;
    gap: 2ch;
    min-width: 0;
  }
  .bar > :global(:first-child) {
    flex: 1;
    min-width: 0;
  }
  .sources {
    flex-shrink: 0;
    background: none;
    border: 0;
    padding: 0 1ch;
    color: var(--gray);
    white-space: nowrap;
    cursor: pointer;
  }
  .sources .at {
    color: var(--blue);
  }
  .categories {
    display: flex;
    gap: 2ch;
    padding: 0 1ch 0.75em;
    overflow-x: auto;
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
    grid-template-columns: minmax(36ch, 56ch) minmax(0, 1fr);
    gap: 0 2ch;
    padding-top: 0.4em;
  }
  .split :global(.browse),
  .split :global(.detail) {
    min-height: 0;
  }
  .prompt {
    display: flex;
    align-items: center;
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
  .tabkey {
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
    align-items: center;
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
    min-width: 0;
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

  /* ---- touch screens ---- */
  .search-area {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding-top: 10px;
  }
  .search-area :global(.searchpane) {
    padding: 0 6px 0 14px;
  }
  .touch .prompt {
    min-height: 48px;
  }
  .touch .prompt span {
    font-size: 1.1em;
  }
  .clear {
    width: 44px;
    height: 44px;
    background: none;
    border: 0;
    color: var(--gray);
    font-size: 1.1em;
    border-radius: 50%;
    cursor: pointer;
  }
  .clear:active,
  .key:active,
  .chip:active,
  .touch .rows button:active {
    background: var(--dark);
  }
  /* Termux-style extra keys for what phone keyboards hide. */
  .extra {
    display: flex;
    gap: 6px;
    overflow-x: auto;
  }
  .key {
    min-width: 44px;
    height: 40px;
    flex: 1;
    background: var(--bar);
    border: 1px solid var(--border);
    border-radius: 6px;
    color: var(--accent);
    font-weight: 700;
    cursor: pointer;
  }
  .key.wide {
    flex: 1.6;
    color: var(--green);
  }
  .key:disabled {
    color: var(--dim);
  }
  .chips,
  .touch .categories {
    display: flex;
    gap: 8px;
    overflow-x: auto;
    padding: 0;
  }
  .chip,
  .touch .categories button {
    flex-shrink: 0;
    height: 36px;
    padding: 0 14px;
    border: 1px solid var(--border);
    border-radius: 18px;
    background: none;
    color: var(--gray);
    white-space: nowrap;
    cursor: pointer;
  }
  .chip {
    color: var(--green);
  }
  .touch .categories button.active {
    background: var(--cyan);
    border-color: var(--cyan);
    color: var(--black);
    font-weight: 700;
  }
  .touch .split {
    padding-top: 20px;
  }
  .touch :global(.browse) {
    padding: 14px 0 0;
  }
  .touch .rows button {
    min-height: 48px;
    padding: 0 14px;
    border-left-width: 3px;
  }
  .touch .rows li + li button {
    border-top: 1px solid rgb(65 72 104 / 0.35);
  }
  .glyph {
    width: 1.2em;
    text-align: center;
  }
  .touch .src {
    font-size: 0.85em;
  }
  .touch .more {
    padding: 14px;
  }
  .touch .doc {
    font-size: 1em;
    padding-right: 4px;
  }
  .touch .head h1 {
    font-size: 1.15em;
  }

  /* Tablets: the entry gets the larger share. */
  .touch:not(.narrow) .split {
    grid-template-columns: minmax(260px, 2fr) minmax(0, 3fr);
    gap: 0 16px;
  }
  .touch:not(.narrow) :global(.detail) {
    padding: 18px 20px 8px;
  }

  /* Narrow screens: one pane at a time. */
  .narrow .split {
    grid-template-columns: minmax(0, 1fr);
  }
  .narrow .split :global(.detail),
  .narrow.open .search-area,
  .narrow.open .categories {
    display: none;
  }
  .narrow.open .split :global(.browse) {
    display: none;
  }
  .narrow.open .split :global(.detail) {
    display: flex;
  }
  .narrow.open .split {
    padding-top: 10px;
  }
</style>
