<!-- The mobile top bar: the menu button (and Back when something is
     open), the place as a prompt, and the screen's actions on the right. -->
<script lang="ts">
  let {
    title,
    back = false,
    onmenu,
    onback,
    actions = [],
  }: {
    title: string;
    back?: boolean;
    onmenu: () => void;
    onback: () => void;
    /// The screen's actions: what each shows, its name, and what it does.
    actions?: { label: string; name: string; onclick: () => void }[];
  } = $props();
</script>

<header class="bar">
  <!-- The menu is always there; Back comes after it when something is open. -->
  <button class="icon" aria-label="Menu" onclick={onmenu}>☰</button>
  {#if back}
    <button class="icon back" aria-label="Back" onclick={onback}>‹</button>
  {/if}
  <div class="title">
    {#if !back}<img class="brand" src="/dnt.svg" alt="dnt" />{/if}
    <span class="prompt">❯</span>
    <span class="name">{title}</span>
  </div>
  {#if actions.length}
    <div class="actions">
      {#each actions as a (a.name)}
        <button class="action" aria-label={a.name} onclick={a.onclick}>
          {#if a.label.startsWith("@")}<span class="at">@</span>{a.label.slice(1)}{:else}{a.label}{/if}
        </button>
      {/each}
    </div>
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
    /* Coloured: it can be tapped (the prompt sign is not). */
    color: var(--accent);
    font-size: 1.5em;
    line-height: 1;
    cursor: pointer;
  }
  .back {
    font-size: 2em;
    font-weight: 700;
  }
  .icon:active {
    background: var(--dark);
  }
  .actions {
    margin-left: auto;
    display: flex;
    gap: 6px;
    flex-shrink: 0;
  }
  .action {
    flex-shrink: 0;
    min-width: 40px;
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
    color: var(--dim);
  }
  .name {
    font-weight: 700;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
