<!-- Tabs as a select: the open one (with its icon) in a box, every one in
     its list. A pick that does not change the tab (one that goes
     elsewhere) leaves the box on the open tab. -->
<script lang="ts">
  import Icon from "./Icon.svelte";
  import type { IconName } from "./icons";

  let {
    label,
    options,
    value,
    icon,
    onpick,
  }: {
    /// What the tabs are, for screen readers ("Tab", "Category").
    label: string;
    /// Each tab: its value and what it says.
    options: [string, string][];
    value: string;
    icon?: IconName;
    onpick: (value: string) => void;
  } = $props();
</script>

<label class="tabsel">
  {#if icon}<span class="icon"><Icon name={icon} /></span>{/if}
  <select
    aria-label={label}
    {value}
    onchange={(e) => {
      const el = e.currentTarget;
      onpick(el.value);
      el.value = value;
    }}
  >
    {#each options as [v, text] (v)}<option value={v}>{text}</option>{/each}
  </select>
  <span class="caret" aria-hidden="true">▾</span>
</label>

<style>
  .tabsel {
    position: relative;
    display: flex;
    align-items: center;
    gap: 1ch;
    border: 1px solid var(--cyan);
    border-radius: var(--radius);
    padding: 0 1ch;
    min-height: 2.2em;
    flex-shrink: 0;
  }
  .icon {
    color: var(--cyan);
    display: inline-flex;
  }
  select {
    flex: 1;
    min-width: 0;
    appearance: none;
    background: none;
    border: 0;
    color: var(--fg);
    font: inherit;
    font-weight: 700;
    padding: 0.3em 2ch 0.3em 0;
    cursor: pointer;
  }
  select:focus {
    outline: none;
  }
  .tabsel:focus-within {
    border-color: var(--accent);
  }
  option {
    background: var(--bg);
    color: var(--fg);
    font-weight: 400;
  }
  .caret {
    position: absolute;
    right: 1ch;
    color: var(--cyan);
    pointer-events: none;
  }
  :global(.touch) .tabsel {
    min-height: 44px;
  }
</style>
