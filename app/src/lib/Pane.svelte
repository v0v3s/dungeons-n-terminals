<!-- A rounded box with its title set into the top border, as the TUI's panes.
     With `fold`, tapping the title folds it to that line (and back); the
     state is kept per `fold` key on this device. `aside` sits in the top
     border on the right (a select for the list below). `section`: on a
     phone or a tablet no box, a gold title over a thin line, `aside` on
     that line (a computer keeps its card). -->
<script lang="ts">
  import type { Snippet } from "svelte";
  import { untrack } from "svelte";

  let {
    title = "",
    focused = false,
    footer = "",
    fold = "",
    summary = "",
    aside,
    children,
    section = false,
    ontitle,
    class: cls = "",
  }: {
    title?: string;
    focused?: boolean;
    footer?: string;
    /// A key to fold the pane by (empty: it cannot be folded).
    fold?: string;
    /// What a folded pane shows after its title.
    summary?: string;
    aside?: Snippet;
    children: Snippet;
    /// A section of a page rather than a box (see above).
    section?: boolean;
    /// What a tap on the title does instead of folding (an edit).
    ontitle?: () => void;
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

  // A title that edits does not fold: never folded away.
  let folded = $state(untrack(() => !!fold && !ontitle && load().includes(fold)));

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

<section class="pane {cls}" class:focused class:folded class:section>
  {#if title && ontitle}
    <h2 class="title"><button class="fold" onclick={ontitle}>{title}<span class="edit" aria-hidden="true">✎</span></button></h2>
  {:else if title && fold}
    <h2 class="title">
      <button class="fold" aria-expanded={!folded} onclick={toggle}>
        <span class="chev">{folded ? "▸" : "▾"}</span>{title}{#if folded && summary}<span class="summary">{summary}</span>{/if}
      </button>
    </h2>
  {:else if title}
    <h2 class="title">{title}</h2>
  {/if}
  {#if aside && !folded}<div class="aside">{@render aside()}</div>{/if}
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
  .edit {
    color: var(--dim);
    font-weight: 400;
    font-size: 0.8em;
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
  .aside {
    position: absolute;
    top: -0.85em;
    right: 1ch;
    padding: 0 1ch;
    background: var(--bg);
    line-height: 1.5;
    max-width: 55%;
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
  /* A section on a phone or a tablet: its title a gold line, the aside on
     it, no box. */
  @media not ((pointer: fine) and (min-width: 761px)) {
    .pane.section,
    .pane.section.folded {
      border: 0;
      border-radius: 0;
      padding: 0;
      height: auto;
    }
    .section .title {
      position: static;
      max-width: none;
      display: flex;
      align-items: center;
      min-height: 32px;
      padding: 0 0 1px;
      margin: 0 0 0.3em;
      background: none;
      color: var(--accent);
      font-size: 1.15em;
      line-height: 1.3;
      border-bottom: 1px solid rgb(224 175 104 / 0.55);
    }
    .section.folded .title {
      margin: 0;
    }
    .section .title:has(.fold) {
      top: 0;
      line-height: 1.3;
    }
    .section .fold {
      flex: 1;
      min-height: 32px;
      align-items: center;
    }
    .section .aside {
      top: 0;
      right: 0;
      padding: 0;
      height: 32px;
      display: flex;
      align-items: center;
      background: none;
      max-width: 70%;
    }
  }
  /* On a computer: a card, its title a small heading inside it, the aside
     and the footer in its corners. */
  @media (pointer: fine) and (min-width: 761px) {
    .pane {
      background: var(--card);
      border-color: var(--edge);
      padding: 0.9rem 1.1rem 1rem;
    }
    .pane.focused {
      border-color: var(--edge);
    }
    .pane.folded {
      border-width: 1px;
      border-radius: var(--radius);
      padding: 0.6rem 1.1rem;
      height: auto;
    }
    .title {
      position: static;
      max-width: 100%;
      padding: 0;
      margin: 0 0 0.6rem;
      background: none;
      font-size: 0.9em;
      font-weight: 600;
      letter-spacing: 0.02em;
      color: var(--gray);
    }
    .folded .title {
      margin: 0;
    }
    .focused .title {
      color: var(--gray);
    }
    .aside {
      top: 0.55rem;
      right: 1.1rem;
      padding: 0;
      background: none;
    }
    .footer {
      position: static;
      align-self: flex-end;
      padding: 0.4rem 0 0;
      background: none;
      font-size: 0.85em;
    }
  }
</style>
