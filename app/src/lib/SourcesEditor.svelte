<!-- The books a campaign uses, and its homebrew. Official sources: a
     preset ticks a set of them (to change as you like), a filter, and a
     tick per source in a few sections that can be ticked as a whole.
     Homebrew (off unless ticked): searched by name or author, the ticked
     ones first. -->
<script lang="ts">
  import { onMount, untrack } from "svelte";
  import * as api from "./api";
  import { working } from "./busy.svelte";
  import Modal from "./Modal.svelte";

  let { campaign, onsave, onclose }: { campaign: api.CampaignInfo; onsave: (s: api.Status) => void; onclose: () => void } = $props();

  // Edited here, saved with Save.
  let picked: string[] = $state(untrack(() => [...campaign.sources]));
  let brew: string[] = $state(untrack(() => [...campaign.homebrew]));
  let reprinted = $state(untrack(() => campaign.show_reprinted));
  /// Showing what "only the newest printing" means.
  let explain = $state(false);
  let available = $state<api.Sources | null>(null);
  let tab: "official" | "homebrew" = $state("official");
  let filter = $state("");
  /// Homebrew of one edition only: "" (any), "one" (2024) or "classic".
  let edition = $state("");
  let error = $state("");

  onMount(async () => {
    try {
      available = await api.sources();
    } catch (e) {
      error = String(e);
    }
  });

  async function save() {
    try {
      onsave(await working("Loading the sources…", () => api.setCampaignSources(campaign.slug, picked, brew, reprinted)));
      onclose();
    } catch (e) {
      error = String(e);
    }
  }

  // No sources listed means all of them: shown as all ticked, and kept
  // that way when everything is ticked again.
  const allIds = $derived((available?.sources ?? []).map((s) => s.id));
  const ticked = $derived(picked.length ? picked : allIds);
  const isOn = (id: string) => !picked.length || picked.includes(id);
  const settle = (ids: string[]) => (allIds.every((id) => ids.includes(id)) ? [] : ids);

  function toggle(id: string) {
    picked = settle(ticked.includes(id) ? ticked.filter((s) => s !== id) : [...ticked, id]);
  }

  function toggleBrew(id: string) {
    brew = brew.includes(id) ? brew.filter((s) => s !== id) : [...brew, id];
  }

  /// Tick a whole section, or untick it when it is all ticked.
  function toggleAll(list: api.Source[]) {
    const ids = list.map((s) => s.id);
    picked = settle(ids.every(isOn) ? ticked.filter((id) => !ids.includes(id)) : [...new Set([...ticked, ...ids])]);
  }

  /// The preset the ticks match, if any.
  const preset = $derived(
    !picked.length ? "all" : (available?.presets.find(([, ids]) => same(picked, ids))?.[0] ?? "custom"),
  );

  function choosePreset(name: string) {
    if (name === "all") picked = [];
    else {
      const ids = available?.presets.find(([n]) => n === name)?.[1];
      if (ids) picked = [...ids];
    }
  }

  const words = $derived(filter.toLowerCase().split(/\s+/).filter(Boolean));

  const groups = $derived.by(() => {
    const out = new Map<string, api.Source[]>();
    for (const s of available?.sources ?? []) {
      if (!words.every((w) => (s.name + " " + s.id).toLowerCase().includes(w))) continue;
      const g = section(s.group);
      out.set(g, [...(out.get(g) ?? []), s]);
    }
    return [...out.entries()].sort(([a], [b]) => SECTIONS.indexOf(a) - SECTIONS.indexOf(b));
  });

  /// The data's book groups, merged into a few readable sections.
  const SECTIONS = ["Core rulebooks", "Supplements", "Settings", "Adventures", "Other"];
  function section(group: string): string {
    if (group === "core") return SECTIONS[0];
    if (group.startsWith("supplement")) return SECTIONS[1];
    if (group.startsWith("setting")) return SECTIONS[2];
    if (group === "adventure") return SECTIONS[3];
    return SECTIONS[4];
  }

  /// At most this many homebrew rows; the filter narrows the rest.
  const MAX_BREW = 120;

  const brewRows = $derived.by(() => {
    const all = available?.homebrew ?? [];
    const on = all.filter((b) => brew.includes(b.id));
    const matching = all.filter(
      (b) =>
        !brew.includes(b.id) &&
        (!edition || (edition === "one") === (b.edition === "one")) &&
        words.every((w) => (b.name + " " + b.id + " " + b.authors.join(" ")).toLowerCase().includes(w)),
    );
    return { on, matching: matching.slice(0, MAX_BREW), more: Math.max(0, matching.length - MAX_BREW) };
  });

  const entries = (b: api.BrewSource) => b.kinds.reduce((n, [, k]) => n + k, 0);
  const contents = (b: api.BrewSource) =>
    b.kinds
      .filter(([k]) => !k.endsWith("Feature"))
      .map(([k, n]) => `${n} ${k}`)
      .join(" · ");
  const same = (a: readonly string[], b: readonly string[]) => a.length === b.length && a.every((x) => b.includes(x));
</script>

{#snippet brewRow(b: api.BrewSource)}
  <button class="source brew" class:on={brew.includes(b.id)} onclick={() => toggleBrew(b.id)}>
    <span class="tick">{brew.includes(b.id) ? "◉" : "○"}</span>
    <span class="what">
      <span class="sname">{b.name}{#if b.edition === "one"}<span class="ed">2024</span>{/if}</span>
      <span class="dim small">{b.id} · {b.authors.length ? `${b.authors.slice(0, 2).join(", ")} · ` : ""}{contents(b) || `${entries(b)} entries`}</span>
    </span>
  </button>
{/snippet}

<Modal title="Sources · {campaign.name}" {onclose} wide>
  <div class="tabs">
    <button class:active={tab === "official"} onclick={() => (tab = "official")}>
      Official <span class="count">{picked.length ? picked.length : "all"}</span>
    </button>
    <button class:active={tab === "homebrew"} onclick={() => (tab = "homebrew")}>
      Homebrew <span class="count">{brew.length ? `${brew.length} on` : (available?.homebrew.length ?? "")}</span>
    </button>
  </div>

  {#if tab === "official"}
    <div class="row">
      <label class="line"><span>❯</span><input bind:value={filter} placeholder="filter" spellcheck="false" autocomplete="off" autocapitalize="off" /></label>
      <select aria-label="Tick a preset" value={preset} onchange={(e) => choosePreset(e.currentTarget.value)}>
        <option value="all">All official</option>
        {#each available?.presets ?? [] as [name] (name)}<option value={name}>{name}</option>{/each}
        {#if preset === "custom"}<option value="custom">Your choice</option>{/if}
      </select>
    </div>
    <div class="sources">
      {#if !available && !error}<p class="dim">Loading…</p>{/if}
      {#each groups as [group, list] (group)}
        {@const all = list.every((s) => isOn(s.id))}
        <button class="group" onclick={() => toggleAll(list)}>
          <span>{group}</span><span class="dim">{all ? "untick all" : "tick all"}</span>
        </button>
        {#each list as s (s.id)}
          <button class="source" class:on={isOn(s.id)} onclick={() => toggle(s.id)}>
            <span class="tick">{isOn(s.id) ? "◉" : "○"}</span>
            <span class="sname">{s.name}</span>
            <span class="dim">{s.id}</span>
          </button>
        {/each}
      {/each}
    </div>
    <div class="reprinted">
      <label><input type="checkbox" checked={!reprinted} onchange={(e) => (reprinted = !e.currentTarget.checked)} /> Only the newest printing</label>
      <button class="hint" class:on={explain} aria-label="What is this?" onclick={() => (explain = !explain)}>?</button>
    </div>
    {#if explain}
      <p class="dim explain">An entry reprinted in another ticked book shows only in the newer one: with both PHB and XPHB ticked, Fireball shows once, from XPHB.</p>
    {/if}
  {:else}
    <div class="row">
      <label class="line"><span>❯</span><input bind:value={filter} placeholder="name or author" spellcheck="false" autocomplete="off" autocapitalize="off" /></label>
      <select aria-label="Rules" bind:value={edition}>
        <option value="">Any rules</option>
        <option value="one">2024</option>
        <option value="classic">2014</option>
      </select>
    </div>
    <div class="sources">
      {#if !available && !error}<p class="dim">Loading…</p>{/if}
      {#if available && !available.homebrew.length}
        <p class="dim">No homebrew imported. Import a zip of homebrew files on the Data screen.</p>
      {/if}
      {#if brewRows.on.length}
        <button class="group" onclick={() => (brew = [])}><span>Enabled</span><span class="dim">untick all</span></button>
        {#each brewRows.on as b (b.id)}{@render brewRow(b)}{/each}
      {/if}
      {#if brewRows.matching.length}
        <div class="group plain"><span>{words.length || edition ? "Matching" : "All homebrew"}</span></div>
        {#each brewRows.matching as b (b.id)}{@render brewRow(b)}{/each}
      {/if}
      {#if brewRows.more}<p class="dim more">… {brewRows.more} more: narrow the filter</p>{/if}
    </div>
  {/if}
  {#if error}<p class="err">{error}</p>{/if}
  <div class="buttons">
    <button class="ghost" onclick={onclose}>Cancel</button>
    <button class="primary" onclick={save}>Save</button>
  </div>
</Modal>

<style>
  p {
    margin: 0 0 0.5em;
  }
  .line {
    display: flex;
    gap: 1ch;
    align-items: center;
    border-bottom: 1px solid var(--border);
    padding: 0.3em 0;
    margin-bottom: 0.75em;
  }
  .line span {
    color: var(--accent);
  }
  .line input {
    flex: 1;
    background: none;
    border: 0;
    outline: none;
    min-width: 0;
  }
  .sources {
    overflow-y: auto;
    max-height: 50vh;
    display: flex;
    flex-direction: column;
  }
  .group {
    flex-shrink: 0;
    display: flex;
    justify-content: space-between;
    background: none;
    border: 0;
    border-bottom: 1px solid rgb(65 72 104 / 0.5);
    color: var(--accent);
    font-weight: 700;
    padding: 0.2em 0.5ch;
    margin: 0.75em 0 0.25em;
    text-align: left;
    cursor: pointer;
  }
  .group .dim {
    font-weight: 400;
  }
  .source {
    flex-shrink: 0;
    display: flex;
    gap: 1ch;
    background: none;
    border: 0;
    text-align: left;
    padding: 0.1em 0.5ch;
    cursor: pointer;
  }
  .source .tick {
    color: var(--dim);
  }
  .source.on .tick,
  .source.on .sname {
    color: var(--accent);
  }
  .sname {
    flex: 1;
  }
  .what {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .small {
    font-size: 0.9em;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ed {
    margin-left: 1ch;
    padding: 0 0.6ch;
    border: 1px solid var(--magenta);
    border-radius: 3px;
    color: var(--magenta);
    font-size: 0.85em;
  }
  .source.brew {
    padding: 0.25em 0.5ch;
    align-items: flex-start;
  }
  .group.plain {
    cursor: default;
  }
  .more {
    margin-top: 0.5em;
  }
  .tabs {
    display: flex;
    gap: 1ch;
    margin-bottom: 0.9em;
  }
  .tabs button {
    background: none;
    border: 0;
    color: var(--gray);
    padding: 0 1ch;
    cursor: pointer;
  }
  .tabs .count {
    color: var(--dim);
  }
  .tabs button.active {
    background: var(--cyan);
    color: var(--black);
    font-weight: 700;
  }
  .tabs button.active .count {
    color: var(--black);
  }
  .row {
    display: flex;
    gap: 1.5ch;
    align-items: center;
    margin-bottom: 0.5em;
  }
  .row .line {
    flex: 1;
    min-width: 0;
    margin-bottom: 0;
  }
  select {
    flex-shrink: 0;
    max-width: 50%;
    background: var(--bg);
    color: var(--fg);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 0.25em 1ch;
    font: inherit;
  }
  .reprinted {
    display: flex;
    gap: 1.5ch;
    align-items: center;
    margin-top: 0.6em;
  }
  .reprinted label {
    display: flex;
    gap: 1ch;
    align-items: center;
  }
  .reprinted input {
    accent-color: var(--accent);
  }
  .hint {
    width: 1.8em;
    height: 1.8em;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: 50%;
    background: none;
    color: var(--dim);
    cursor: pointer;
  }
  .hint.on {
    border-color: var(--accent);
    color: var(--accent);
  }
  .explain {
    margin: 0.4em 0 0;
  }
  .err {
    color: var(--red);
  }
  .buttons {
    display: flex;
    justify-content: flex-end;
    gap: 1.5ch;
    margin-top: 1em;
  }
  @media (pointer: coarse) {
    .group,
    .source {
      min-height: 44px;
      align-items: center;
    }
    .tabs button {
      min-height: 40px;
      flex: 1;
    }
    .reprinted {
      min-height: 44px;
      align-items: center;
    }
    .buttons button {
      flex: 1;
    }
  }
</style>
