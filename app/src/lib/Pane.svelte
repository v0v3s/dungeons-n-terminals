<!-- A rounded box with its title set into the top border, as the TUI's panes.
     With `fold`, tapping the title folds it to that line (and back); the
     state is kept per `fold` key on this device. -->
<script lang="ts">
  import type { Snippet } from "svelte";
  import { untrack } from "svelte";

  let {
    title = "",
    focused = false,
    footer = "",
    fold = "",
    summary = "",
    children,
    class: cls = "",
  }: {
    title?: string;
    focused?: boolean;
    footer?: string;
    /// A key to fold the pane by (empty: it cannot be folded).
    fold?: string;
    /// What a folded pane shows after its title.
    summary?: string;
    children: Snippet;
    class?: string;
  } = $props();

  const KEY = "dnt.folded";
  function load(): string[] {
    try {
      return JSON.parse(localStorage.getItem(KEY) ?? "[]");
    } catch {
      return [];
    }
  }

  let folded = $state(untrack(() => !!fold && load().includes(fold)));

  function toggle() {
    folded = !folded;
    try {
      const keys = load().filter((k) => k !== fold);
      localStorage.setItem(KEY, JSON.stringify(folded ? [...keys, fold] : keys));
    } catch {
      // Not kept: it still folds for now.
    }
  }
</script>

<section class="pane {cls}" class:focused class:folded>
  {#if title && fold}
    <h2 class="title">
      <button class="fold" aria-expanded={!folded} onclick={toggle}>
        <span class="chev">{folded ? "▸" : "▾"}</span>{title}{#if folded && summary}<span class="summary">{summary}</span>{/if}
      </button>
    </h2>
  {:else if title}
    <h2 class="title">{title}</h2>
  {/if}
  {#if !folded}<div class="body">{@render children()}</div>{/if}
  {#if footer && !folded}<div class="footer">{footer}</div>{/if}
</section>

<style>
  .pane {
    position: relative;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 1.1em 1.5ch 0.6em;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .pane.focused {
    border-color: var(--accent);
  }
  /* Folded: one line with the title on it, as a TUI rule. */
  .pane.folded {
    border-width: 1px 0 0;
    border-radius: 0;
    padding: 0;
    height: 0.75em;
    flex: none !important;
  }
  .title {
    position: absolute;
    top: -0.75em;
    left: 1ch;
    margin: 0;
    padding: 0 1ch;
    background: var(--bg);
    font-size: 1em;
    font-weight: 700;
    color: var(--gray);
    line-height: 1.5;
    white-space: nowrap;
    max-width: calc(100% - 2ch);
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .focused .title {
    color: var(--accent);
  }
  .fold {
    display: flex;
    align-items: baseline;
    gap: 1ch;
    max-width: 100%;
    background: none;
    border: 0;
    padding: 0;
    font: inherit;
    color: inherit;
    cursor: pointer;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .chev {
    color: var(--accent);
  }
  .summary {
    color: var(--dim);
    font-weight: 400;
  }
  .body {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .footer {
    position: absolute;
    bottom: -0.75em;
    right: 1ch;
    padding: 0 1ch;
    background: var(--bg);
    color: var(--dim);
    line-height: 1.5;
  }
  @media (pointer: coarse) {
    /* A bigger target than the line of text. */
    .title:has(.fold) {
      top: -18px;
      line-height: 36px;
    }
  }
</style>
