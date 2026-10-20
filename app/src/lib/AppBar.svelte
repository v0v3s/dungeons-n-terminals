<!-- The mobile top bar: menu (or back) button, the place as a prompt, and
     the screen's action on the right, if it has one. -->
<script lang="ts">
  let {
    title,
    back = false,
    onmenu,
    onback,
    action = null,
  }: {
    title: string;
    back?: boolean;
    onmenu: () => void;
    onback: () => void;
    /// The screen's action: what it shows, its name, and what it does.
    action?: { label: string; name: string; onclick: () => void } | null;
  } = $props();
</script>

<header class="bar">
  {#if back}
    <button class="icon" aria-label="Back" onclick={onback}>‹</button>
  {:else}
    <button class="icon" aria-label="Menu" onclick={onmenu}>☰</button>
  {/if}
  <div class="title">
    {#if !back}<img class="brand" src="/dnt.svg" alt="dnt" />{/if}
    <span class="prompt">❯</span>
    <span class="name">{title}</span>
  </div>
  {#if action}
    <button class="action" aria-label={action.name} onclick={action.onclick}><span class="at">@</span> {action.label}</button>
  {/if}
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
  .action {
    margin-left: auto;
    flex-shrink: 0;
    min-height: 40px;
    max-width: 45vw;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    background: none;
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 0 1.2ch;
    color: var(--fg);
  }
  .action:active {
    background: var(--dark);
  }
  .at {
    color: var(--blue);
  }
  .title {
    display: flex;
    align-items: baseline;
    gap: 1ch;
    min-width: 0;
    font-size: 1.1em;
  }
  .brand {
    height: 1.45em;
    width: auto;
    flex-shrink: 0;
    align-self: center;
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
