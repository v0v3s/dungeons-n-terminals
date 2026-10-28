<!-- Choose from a list: type to filter, pick one (or `count`). With
     `oninfo`, each row has a ? to read about its choice first. -->
<script lang="ts">
  import { onMount, tick } from "svelte";
  import type { Choice } from "./api";
  import { touch } from "./screen";

  let {
    choices,
    count = 1,
    initial = 0,
    onpick,
    onhighlight,
    oninfo,
  }: {
    choices: Choice[];
    count?: number;
    initial?: number;
    onpick: (values: string[]) => void;
    onhighlight?: (value: string) => void;
    oninfo?: (c: Choice) => void;
  } = $props();

  let filter = $state("");
  let cursor = $state(0);
  let chosen: string[] = $state([]);
  let input: HTMLInputElement | undefined = $state();
  let list: HTMLElement | undefined = $state();

  const shown = $derived.by(() => {
    const words = filter.toLowerCase().split(/\s+/).filter(Boolean);
    return choices.filter((c) => words.every((w) => (c.label + " " + c.hint).toLowerCase().includes(w)));
  });
  const many = $derived(count > 1);

  $effect(() => {
    const c = shown[cursor];
    if (c) onhighlight?.(c.value);
  });

  function toggle(c: Choice) {
    if (!many) return onpick([c.value]);
    if (chosen.includes(c.value)) chosen = chosen.filter((v) => v !== c.value);
    else if (chosen.length < count) chosen = [...chosen, c.value];
    if (chosen.length === count) onpick(chosen);
  }

  async function move(d: number) {
    cursor = Math.max(0, Math.min(shown.length - 1, cursor + d));
    await tick();
    list?.querySelector(`[data-i="${cursor}"]`)?.scrollIntoView({ block: "nearest" });
  }

  function onkey(e: KeyboardEvent) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      move(1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      move(-1);
    } else if (e.key === "Enter") {
      e.preventDefault();
      const c = shown[cursor];
      if (c) toggle(c);
    } else if (e.key === " " && many && !filter) {
      e.preventDefault();
      const c = shown[cursor];
      if (c) toggle(c);
    }
  }

  onMount(() => {
    cursor = Math.min(initial, Math.max(choices.length - 1, 0));
    if (!touch.current) input?.focus();
    tick().then(() => move(0));
  });
</script>

<svelte:window onkeydown={onkey} />

{#if choices.length > 8}
  <label class="filter">
    <span>❯</span>
    <input bind:this={input} bind:value={filter} oninput={() => (cursor = 0)} placeholder="type to filter" spellcheck="false" autocomplete="off" />
  </label>
{/if}
{#if many}
  <div class="count dim">{chosen.length} of {count} chosen{#if chosen.length}<button class="done" onclick={() => onpick(chosen)}>done</button>{/if}</div>
{/if}
<ul bind:this={list}>
  {#each shown as c, i (c.value)}
    <li data-i={i}>
      <button class:cursor={i === cursor} class:chosen={chosen.includes(c.value)} onclick={() => toggle(c)} onmouseenter={() => !touch.current && (cursor = i)}>
        {#if many}<span class="box">{chosen.includes(c.value) ? "◉" : "○"}</span>{/if}
        <span class="label">{c.label}</span>
        {#if c.warning}<span class="warn">⚠ {c.warning}</span>{/if}
        <span class="hint dim">{c.hint}</span>
      </button>
      {#if oninfo}<button class="info" aria-label="About {c.label}" onclick={() => oninfo(c)}>?</button>{/if}
    </li>
  {/each}
  {#if !shown.length}<li class="dim none">Nothing matches.</li>{/if}
</ul>

<style>
  .filter {
    display: flex;
    gap: 1ch;
    align-items: center;
    border-bottom: 1px solid var(--border);
    padding-bottom: 0.4em;
    margin-bottom: 0.4em;
  }
  .filter span {
    color: var(--accent);
  }
  .filter input {
    flex: 1;
    background: none;
    border: 0;
    outline: none;
    padding: 0.2em 0;
    min-width: 0;
  }
  .count {
    display: flex;
    justify-content: space-between;
    margin-bottom: 0.4em;
  }
  .done {
    background: var(--accent);
    color: var(--black);
    border: 0;
    border-radius: 3px;
    font-weight: 700;
    padding: 0 1.5ch;
    cursor: pointer;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    overflow-y: auto;
    min-height: 0;
    flex: 1;
  }
  li {
    display: flex;
    align-items: stretch;
  }
  li button:not(.info) {
    flex: 1;
    min-width: 0;
    display: flex;
    gap: 1ch;
    align-items: center;
    background: none;
    border: 0;
    border-left: 2px solid transparent;
    padding: 0.1em 0.5ch;
    text-align: left;
    cursor: pointer;
  }
  li button.cursor {
    background: var(--selected);
    border-left-color: var(--accent);
  }
  li button.chosen .label {
    color: var(--accent);
    font-weight: 700;
  }
  .box {
    color: var(--accent);
  }
  .info {
    flex: none;
    align-self: center;
    width: 4ch;
    margin-left: 0.5ch;
    background: none;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    color: var(--cyan);
    font-weight: 700;
    cursor: pointer;
  }
  .label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .hint {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 45%;
  }
  .warn {
    color: var(--yellow);
    white-space: nowrap;
    font-size: 0.9em;
  }
  .none {
    padding: 0.5em;
  }
  @media (pointer: coarse) {
    li button:not(.info) {
      min-height: 48px;
      padding: 0 12px;
      border-left-width: 3px;
    }
    li + li {
      border-top: 1px solid rgb(65 72 104 / 0.35);
    }
    .info {
      width: 44px;
      height: 44px;
    }
    /* The hint gets its own line. */
    li button:not(.info) {
      flex-wrap: wrap;
      row-gap: 0;
      padding-top: 6px;
      padding-bottom: 6px;
    }
    .label {
      flex: 1 1 60%;
      white-space: normal;
    }
    .hint {
      flex-basis: 100%;
      max-width: none;
      white-space: normal;
      font-size: 0.85em;
    }
    li button.cursor:not(:active) {
      background: none;
      border-left-color: transparent;
    }
    li button:active {
      background: var(--selected);
    }
    .filter {
      min-height: 44px;
    }
    .done {
      padding: 6px 16px;
    }
  }
</style>
