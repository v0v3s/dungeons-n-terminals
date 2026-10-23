<!-- A game-icons.net icon, as big as the text and in its colour (see
     icons.ts and the About pane for the credits). -->
<script lang="ts">
  import { ICONS, type IconName } from "./icons";

  let { name, size = "1.15em", label = "" }: { name: IconName; size?: string; label?: string } = $props();
</script>

<svg
  class="icon"
  viewBox="0 0 512 512"
  width={size}
  height={size}
  fill="currentColor"
  role={label ? "img" : undefined}
  aria-label={label || undefined}
  aria-hidden={label ? undefined : "true"}
>
  {#each ICONS[name].paths as d, i (i)}<path {d} />{/each}
</svg>

<style>
  .icon {
    display: inline-block;
    vertical-align: -0.18em;
    flex-shrink: 0;
  }
</style>
