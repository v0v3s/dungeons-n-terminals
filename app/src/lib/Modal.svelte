<!-- A dialog: a centred dnt pane on desktop, a bottom sheet on touch
     screens. Esc, Android's back and the scrim close it. -->
<script lang="ts">
  import type { Snippet } from "svelte";
  import { fade, fly } from "svelte/transition";
  import { onMount } from "svelte";
  import { opened } from "./modals";
  import { touch } from "./screen";

  let {
    title,
    onclose,
    children,
    footer = "",
    wide = false,
  }: { title: string; onclose: () => void; children: Snippet; footer?: string; wide?: boolean } = $props();

  onMount(() => opened(() => onclose()));

  function onkey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.stopPropagation();
      onclose();
    }
  }
</script>

<svelte:window onkeydown={onkey} />

<button class="scrim" aria-label="Close" onclick={onclose} transition:fade={{ duration: 120 }}></button>
<div
  class="modal"
  class:sheet={touch.current}
  class:wide
  role="dialog"
  aria-label={title}
  transition:fly={touch.current ? { y: 400, duration: 180, opacity: 1 } : { y: 12, duration: 120 }}
>
  {#if touch.current}<div class="grip"></div>{/if}
  <h2 class="title">{title}</h2>
  <div class="body">{@render children()}</div>
  {#if footer && !touch.current}<div class="footer">{footer}</div>{/if}
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    border: 0;
    padding: 0;
    background: rgb(0 0 0 / 0.5);
    z-index: 20;
  }
  .modal {
    position: fixed;
    z-index: 21;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    width: min(76ch, calc(100vw - 4ch));
    max-height: calc(100vh - 6em);
    display: flex;
    flex-direction: column;
    background: var(--bg);
    border: 1px solid var(--accent);
    border-radius: var(--radius);
    padding: 1.2em 2ch 1em;
  }
  .modal.wide {
    width: min(110ch, calc(100vw - 4ch));
  }
  .title {
    position: absolute;
    top: -0.75em;
    left: 1ch;
    margin: 0;
    padding: 0 1ch;
    background: var(--bg);
    font-size: 1em;
    color: var(--accent);
    max-width: calc(100% - 2ch);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .body {
    min-height: 0;
    overflow-y: auto;
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
  }
  /* Touch: a sheet from the bottom. */
  .modal.sheet {
    left: 0;
    right: 0;
    top: auto;
    bottom: 0;
    transform: none;
    width: auto;
    max-height: 88vh;
    border-radius: 14px 14px 0 0;
    border-bottom: 0;
    padding: 8px 16px max(env(safe-area-inset-bottom), 24px);
  }
  .sheet .title {
    position: static;
    padding: 4px 0 12px;
    font-size: 1.1em;
    white-space: normal;
  }
  .grip {
    width: 40px;
    height: 4px;
    border-radius: 2px;
    background: var(--dark);
    margin: 4px auto 8px;
  }
</style>
