<!-- The app: a header with the sections, the section, and the keys. -->
<script lang="ts">
  import { onMount } from "svelte";
  import * as api from "$lib/api";
  import Compendium from "$lib/Compendium.svelte";
  import Data from "$lib/Data.svelte";

  type Section = "compendium" | "data";
  const SECTIONS: [Section, string][] = [
    ["compendium", "Compendium"],
    ["data", "Data"],
  ];

  let section: Section = $state("compendium");
  let status: api.Status | null = $state(null);
  let error = $state("");

  onMount(async () => {
    try {
      status = await api.openLibrary();
      if (!status.loaded) section = "data";
    } catch (e) {
      error = String(e);
    }
  });

  function changed(s: api.Status) {
    status = s;
  }

  function onkey(e: KeyboardEvent) {
    if (e.altKey && e.key >= "1" && e.key <= String(SECTIONS.length)) {
      section = SECTIONS[Number(e.key) - 1][0];
      e.preventDefault();
    }
  }
</script>

<svelte:window onkeydown={onkey} />

<div class="app">
  <header>
    <span class="brand">dnt</span>
    <nav>
      {#each SECTIONS as [id, label], i (id)}
        <button class:active={section === id} onclick={() => (section = id)}>
          <span class="n">{i + 1}</span>
          {label}
        </button>
      {/each}
    </nav>
    <span class="right dim">{status?.loaded ? `${status.entries} entries` : ""}</span>
  </header>

  <main>
    {#if error}
      <p class="err">{error}</p>
    {:else if !status}
      <p class="dim">Loading…</p>
    {:else if section === "compendium" && status.loaded}
      {#key status.entries}<Compendium />{/key}
    {:else}
      <Data {status} onchange={changed} />
    {/if}
  </main>

  <footer class="keys">
    {#if section === "compendium"}
      <span><span class="keycap">↑↓</span> move</span>
      <span><span class="keycap">/</span> search</span>
      <span><span class="keycap">tab</span> complete · category</span>
      <span><span class="keycap">esc</span> back</span>
    {/if}
    <span><span class="keycap">alt+1-2</span> section</span>
  </footer>
</div>

<style>
  .app {
    height: 100vh;
    height: 100dvh;
    display: flex;
    flex-direction: column;
    padding: env(safe-area-inset-top) env(safe-area-inset-right) env(safe-area-inset-bottom) env(safe-area-inset-left);
  }
  header {
    display: flex;
    align-items: center;
    gap: 2ch;
    padding: 0.25em 1ch;
    white-space: nowrap;
  }
  .brand {
    background: var(--accent);
    color: var(--black);
    font-weight: 700;
    padding: 0 1ch;
  }
  nav {
    display: flex;
    gap: 1ch;
    overflow-x: auto;
    scrollbar-width: none;
  }
  nav button {
    background: none;
    border: 0;
    color: var(--gray);
    padding: 0 1ch;
    cursor: pointer;
  }
  nav .n {
    color: var(--dim);
    margin-right: 0.5ch;
  }
  nav button.active {
    background: var(--gray);
    color: var(--black);
    font-weight: 700;
  }
  nav button.active .n {
    color: var(--black);
  }
  .right {
    margin-left: auto;
  }
  main {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    padding: 0.75em 1ch 0.5em;
  }
  .keys {
    display: flex;
    gap: 3ch;
    padding: 0.25em 1ch 0.4em;
    color: var(--dim);
    white-space: nowrap;
    overflow: hidden;
  }
  .err {
    color: var(--red);
  }
  /* Touch screens have no keys to show. */
  @media (pointer: coarse) {
    .keys {
      display: none;
    }
  }
</style>
