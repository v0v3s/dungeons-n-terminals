<!-- A rounded box with its title set into the top border, as the TUI's panes. -->
<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    title = "",
    focused = false,
    footer = "",
    children,
    class: cls = "",
  }: { title?: string; focused?: boolean; footer?: string; children: Snippet; class?: string } = $props();
</script>

<section class="pane {cls}" class:focused>
  {#if title}<h2 class="title">{title}</h2>{/if}
  <div class="body">{@render children()}</div>
  {#if footer}<div class="footer">{footer}</div>{/if}
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
</style>
