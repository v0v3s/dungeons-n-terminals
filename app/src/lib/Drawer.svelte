<!-- The navigation drawer: the sections as a dnt pane sliding in from the
     left over a dimmed screen. -->
<script lang="ts">
  import { fade, fly } from "svelte/transition";
  import Icon from "./Icon.svelte";
  import { ICONS, type IconName } from "./icons";

  let {
    open = $bindable(false),
    sections,
    current,
    onpick,
    footer = "",
  }: {
    open: boolean;
    sections: [string, string, string][];
    current: string;
    onpick: (id: string) => void;
    footer?: string;
  } = $props();
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
      {#each sections as [id, label, hint], i (id)}
        <button
          class="item"
          class:active={id === current}
          onclick={() => {
            onpick(id);
            open = false;
          }}
        >
          {#if id in ICONS}<span class="sicon"><Icon name={id as IconName} size="1.4em" /></span>{:else}<span class="keycap">{i + 1}</span>{/if}
          <span class="label">{label}</span>
          {#if hint}<span class="dim hint">{hint}</span>{/if}
        </button>
      {/each}
    </div>
    {#if footer}<div class="foot dim">{footer}</div>{/if}
  </nav>
{/if}

<style>
  .sicon {
    color: var(--dim);
    display: inline-flex;
  }
  .item.active .sicon {
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
  .item.active {
    background: var(--selected);
    border-left-color: var(--accent);
    font-weight: 700;
  }
  .item.active .keycap {
    background: var(--accent);
    color: var(--black);
  }
  .item.active .hint {
    color: var(--gray);
  }
  .label {
    flex: 1;
  }
  .hint {
    font-size: 0.85em;
  }
  .foot {
    margin-top: auto;
    font-size: 0.9em;
  }
</style>
