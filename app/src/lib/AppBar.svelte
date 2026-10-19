<!-- The mobile top bar: menu (or back) button, then the place as a prompt. -->
<script lang="ts">
  let {
    title,
    back = false,
    onmenu,
    onback,
  }: { title: string; back?: boolean; onmenu: () => void; onback: () => void } = $props();
</script>

<header class="bar">
  {#if back}
    <button class="icon" aria-label="Back" onclick={onback}>‹</button>
  {:else}
    <button class="icon" aria-label="Menu" onclick={onmenu}>☰</button>
  {/if}
  <div class="title">
    {#if !back}<span class="brand">dnt</span>{/if}
    <span class="prompt">❯</span>
    <span class="name">{title}</span>
  </div>
</header>

<style>
  .bar {
    height: 56px;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 0 8px 0 4px;
    border-bottom: 1px solid var(--border);
    background: var(--bar);
  }
  .icon {
    width: 48px;
    height: 48px;
    flex-shrink: 0;
    background: none;
    border: 0;
    border-radius: 50%;
    color: var(--fg);
    font-size: 1.5em;
    line-height: 1;
    cursor: pointer;
  }
  .icon:active {
    background: var(--dark);
  }
  .title {
    display: flex;
    align-items: baseline;
    gap: 1ch;
    min-width: 0;
    font-size: 1.1em;
  }
  .brand {
    background: var(--accent);
    color: var(--black);
    font-weight: 700;
    padding: 0 0.6ch;
    border-radius: 3px;
    font-size: 0.85em;
  }
  .prompt {
    color: var(--accent);
  }
  .name {
    font-weight: 700;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
