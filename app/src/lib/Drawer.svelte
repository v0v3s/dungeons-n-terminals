<!-- The navigation drawer: the sections as a dnt pane sliding in from the
     left over a dimmed screen. What a section has inside (the compendium's
     categories, the open character's tabs) unfolds under it with its ▸. -->
<script lang="ts">
  import { fade, fly } from "svelte/transition";
  import Icon from "./Icon.svelte";
  import { ICONS, type IconName } from "./icons";

  let {
    open = $bindable(false),
    sections,
    current,
    currentChild = "",
    onpick,
    footer = "",
  }: {
    open: boolean;
    /// Each section: its id, label, hint and what is inside it (id, label, hint).
    sections: [string, string, string, [string, string, string][]?][];
    current: string;
    /// What is open inside the current section.
    currentChild?: string;
    onpick: (id: string, child?: string) => void;
    footer?: string;
  } = $props();

  /// The sections unfolded (none at first; kept while the app runs).
  let unfolded: Record<string, boolean> = $state({});
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
      <span class="box-title">sections</span>
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
          {#each unfolded[id] ? (children ?? []) : [] as [child, text, n] (child)}
            <button
              class="item child"
              data-in={id}
              class:active={id === current && child === currentChild}
              onclick={() => {
                onpick(id, child);
                open = false;
              }}
            >
              <span class="label">{text}</span>
              {#if n}<span class="dim hint">{n}</span>{/if}
            </button>
          {/each}
        {/each}
      </div>
    </div>
    {#if footer}<div class="foot dim">{footer}</div>{/if}
  </nav>
{/if}

<style>
  .sicon {
    color: var(--dim);
    display: inline-flex;
  }
  .row.active .sicon {
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
    padding: calc(env(safe-area-inset-top) + 20px) 16px calc(env(safe-area-inset-bottom) + 36px);
    gap: 28px;
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
    position: relative;
    min-height: 0;
    display: flex;
    flex-direction: column;
    border: 1px solid var(--accent);
    border-radius: var(--radius);
    padding: 16px 0 8px;
  }
  .box-title {
    position: absolute;
    top: -0.75em;
    left: 1ch;
    padding: 0 1ch;
    background: var(--bg);
    color: var(--accent);
    font-weight: 700;
  }
  .item {
    width: 100%;
    min-height: 52px;
    display: flex;
    align-items: center;
    gap: 1.5ch;
    padding: 0 14px;
    background: none;
    border: 0;
    border-left: 3px solid transparent;
    text-align: left;
    cursor: pointer;
    font-size: 1.05em;
  }
  .row {
    display: flex;
    border-left: 3px solid transparent;
  }
  .row .item {
    flex: 1;
    width: auto;
    min-width: 0;
    border-left: 0;
  }
  .row.active,
  .item.active {
    background: var(--selected);
    border-left-color: var(--accent);
    font-weight: 700;
  }
  .row.active .keycap {
    background: var(--accent);
    color: var(--black);
  }
  .row.active .hint,
  .item.active .hint {
    color: var(--gray);
  }
  .fold {
    flex-shrink: 0;
    width: 52px;
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
    min-height: 40px;
    padding-left: calc(14px + 1.4em + 1.5ch);
    font-size: 0.95em;
    color: var(--gray);
  }
  .item.child.active {
    color: var(--fg);
  }
  .hint {
    font-size: 0.85em;
  }
  .foot {
    margin-top: auto;
    font-size: 0.9em;
  }
</style>
