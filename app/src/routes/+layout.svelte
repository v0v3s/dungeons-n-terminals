<script lang="ts">
  import "../app.css";
  import { fullTextTooltips } from "../lib/overflow";
  let { children } = $props();
  $effect(() => fullTextTooltips());
</script>

{@render children()}
