<!-- The navigation drawer: the sections under a gold heading, sliding in
     from the left over a dimmed screen; the open one in gold. What a section
     has inside (the compendium's categories, the open character's tabs)
     unfolds under it with its ▸: the first few, the rest a tap away. -->
<script lang="ts">
  import { fade, fly } from "svelte/transition";
  import Icon from "./Icon.svelte";
  import { ICONS, type IconName } from "./icons";
  import { GLYPH, type Group } from "./api";

  let {
    open = $bindable(false),
    sections,
    current,
    currentChild = "",
    onpick,
    campaign = "",
    footer = "",
  }: {
    open: boolean;
    /// Each section: its id, label, hint and what is inside it (id, label,
    /// hint, and a compendium category's colour group for its mark).
    sections: [string, string, string, [string, string, string, string?][]?][];
    current: string;
    /// What is open inside the current section.
    currentChild?: string;
    onpick: (id: string, child?: string) => void;
    /// The active campaign, over the footer.
    campaign?: string;
    footer?: string;
  } = $props();

  /// The sections unfolded (none at first; kept while the app runs).
  let unfolded: Record<string, boolean> = $state({});
  /// What is inside a section: the first few, the rest a tap away (not
  /// for one or two more).
  const FEW = 4;
  let all: Record<string, boolean> = $state({});
  const shown = (id: string, children: [string, string, string, string?][]) => (all[id] || children.length <= FEW + 2 ? children : children.slice(0, FEW));
</script>

{#if open}
  <button class="scrim" aria-label="Close menu" onclick={() => (open = false)} transition:fade={{ duration: 150 }}></button>
  <nav class="drawer" transition:fly={{ x: -320, duration: 200, opacity: 1 }}>
    <div class="head">
      <img class="brand" src="/dnt.svg" alt="dnt" />
      <div>
        <div class="name">Dungeons &amp; Terminals</div>
        <div class="dim">a D&amp;D 5e companion</div>
      </div>
    </div>
    <div class="box">
      <h2 class="box-title">Sections</h2>
      <div class="list">
        {#each sections as [id, label, hint, children], i (id)}
          <div class="row" class:active={id === current}>
            <button
              class="item"
              onclick={() => {
                onpick(id);
                open = false;
              }}
            >
              {#if id in ICONS}<span class="sicon"><Icon name={id as IconName} size="1.4em" /></span>{:else}<span class="keycap">{i + 1}</span>{/if}
              <span class="label">{label}</span>
              {#if hint}<span class="dim hint">{hint}</span>{/if}
            </button>
            {#if children?.length}
              <button class="fold" aria-label="What is in {label}" aria-expanded={!!unfolded[id]} onclick={() => (unfolded[id] = !unfolded[id])}>
                {unfolded[id] ? "▾" : "▸"}
              </button>
            {/if}
          </div>
          {#each unfolded[id] ? shown(id, children ?? []) : [] as [child, text, n, g] (child)}
            <button
              class="item child"
              data-in={id}
              class:active={id === current && child === currentChild}
              onclick={() => {
                onpick(id, child);
                open = false;
              }}
            >
              <!-- A category's mark, as the compendium's rows have it. -->
              <span class="mark g-{g}" aria-hidden="true">{g === "all" ? "∗" : g ? GLYPH[g as Group] : ""}</span>
              <span class="label">{text}</span>
              {#if n}<span class="dim hint">{n}</span>{/if}
            </button>
          {/each}
          {#if unfolded[id] && children && shown(id, children).length < children.length}
            <button class="item child more" aria-label="More in {label}" onclick={() => (all[id] = true)}>
              <span class="mark"></span>
              <span class="label">{children.length - FEW} more ▾</span>
            </button>
          {/if}
        {/each}
      </div>
    </div>
    {#if campaign || footer}
      <div class="foot dim">
        {#if campaign}<div class="campaign">◈ {campaign}</div>{/if}
        {footer}
      </div>
    {/if}
  </nav>
{/if}

<style>
  .sicon {
    color: var(--dim);
    display: inline-flex;
  }
  .row.active .sicon,
  .row.active .label {
    color: var(--accent);
  }
  .scrim {
    position: fixed;
    inset: 0;
    border: 0;
    padding: 0;
    background: rgb(0 0 0 / 0.55);
    z-index: 10;
  }
  .drawer {
    position: fixed;
    top: 0;
    bottom: 0;
    left: 0;
    width: min(84vw, 320px);
    z-index: 11;
    background: var(--bg);
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    padding: calc(env(safe-area-inset-top) + 20px) 14px calc(env(safe-area-inset-bottom) + 24px);
    gap: 18px;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 1.5ch;
  }
  .brand {
    height: 2em;
    width: auto;
    flex-shrink: 0;
    align-self: center;
  }
  .name {
    font-weight: 700;
  }
  .box {
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  /* A section's title, as every page's. */
  .box-title {
    margin: 0;
    font-size: 1.1em;
    font-weight: 700;
    color: var(--accent);
    min-height: 32px;
    display: flex;
    align-items: center;
    border-bottom: 1px solid rgb(224 175 104 / 0.55);
  }
  .item {
    width: 100%;
    min-height: 44px;
    display: flex;
    align-items: center;
    gap: 1.5ch;
    padding: 0 2px;
    background: none;
    border: 0;
    text-align: left;
    cursor: pointer;
  }
  .row {
    display: flex;
    border-bottom: 1px solid var(--raised);
  }
  .row .item {
    flex: 1;
    width: auto;
    min-width: 0;
    border-left: 0;
  }
  .row.active,
  .item.active {
    font-weight: 700;
  }
  .row.active .keycap {
    background: var(--accent);
    color: var(--black);
  }
  .fold {
    flex-shrink: 0;
    width: 44px;
    background: none;
    border: 0;
    color: var(--cyan);
    font-size: 1.2em;
    cursor: pointer;
  }
  .label {
    flex: 1;
  }
  .list {
    max-height: 100%;
    overflow-y: auto;
  }
  .item.child {
    min-height: 34px;
    padding-left: calc(2px + 1.4em + 1.5ch);
    font-size: 0.93em;
    color: var(--gray);
    border-bottom: 1px solid var(--raised);
  }
  .item.child.active,
  .item.child.active .label {
    color: var(--accent);
  }
  .mark {
    width: 1.2em;
    margin-left: calc(-1.2em - 1.5ch);
    text-align: center;
    flex-shrink: 0;
  }
  .g-all {
    color: var(--dim);
  }
  .item.child.more {
    color: var(--dim);
    font-size: 0.85em;
  }
  .hint {
    font-size: 0.85em;
  }
  .foot {
    margin-top: auto;
    font-size: 0.85em;
    border-top: 1px solid var(--raised);
    padding-top: 10px;
  }
  .campaign {
    color: var(--accent);
  }
</style>
