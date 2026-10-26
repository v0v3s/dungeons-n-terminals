<!-- What a swipe goes to, in the gap the dragged view leaves: a line along
     its edge and the next view's icon and name, lit once letting go opens
     it. -->
<script lang="ts" generics="T">
  import Icon from "./Icon.svelte";
  import type { IconName } from "./icons";
  import type { Swipe } from "./swipe.svelte";

  let { swipe, label, icon }: { swipe: Swipe<T>; label: string; icon?: IconName } = $props();
</script>

<div
  class="swipe-hint"
  class:ready={swipe.ready}
  class:left={swipe.drag > 0}
  aria-hidden="true"
  style:top="{swipe.top}px"
  style:height="{swipe.height}px"
  style:width="{Math.abs(swipe.drag)}px"
>
  <span class="what">
    {#if icon}<Icon name={icon} size="1.6em" />{/if}
    <span class="name">{label}</span>
  </span>
</div>

<style>
  .swipe-hint {
    position: absolute;
    right: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
    border-left: 2px solid var(--border);
    color: var(--gray);
    pointer-events: none;
  }
  .swipe-hint.left {
    right: auto;
    left: 0;
    border-left: 0;
    border-right: 2px solid var(--border);
  }
  .swipe-hint.ready {
    border-color: var(--accent);
    color: var(--accent);
  }
  .what {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.4em;
    padding: 0 0.5ch;
    font-weight: 700;
    white-space: nowrap;
  }
</style>
