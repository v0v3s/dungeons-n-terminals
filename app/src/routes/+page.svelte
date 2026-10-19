<!-- The app: on desktop a TUI-style header with the sections and a footer
     with the keys; on touch screens an app bar with a drawer. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { exit, onBackButtonPress } from "@tauri-apps/api/app";
  import * as api from "$lib/api";
  import AppBar from "$lib/AppBar.svelte";
  import Busy from "$lib/Busy.svelte";
  import Campaigns from "$lib/Campaigns.svelte";
  import Characters from "$lib/Characters.svelte";
  import Compendium from "$lib/Compendium.svelte";
  import Data from "$lib/Data.svelte";
  import Drawer from "$lib/Drawer.svelte";
  import Notes from "$lib/Notes.svelte";
  import { closeTop } from "$lib/modals";
  import { narrow, touch } from "$lib/screen";

  type Section = "compendium" | "characters" | "notes" | "campaigns" | "data";
  const SECTIONS: [Section, string][] = [
    ["compendium", "Compendium"],
    ["characters", "Characters"],
    ["notes", "Notes"],
    ["campaigns", "Campaigns"],
    ["data", "Data"],
  ];

  let section = $state<Section>("compendium");
  let status = $state<api.Status | null>(null);
  let error = $state("");
  let compendium: Compendium | undefined = $state();
  let characters: Characters | undefined = $state();
  let notes: Notes | undefined = $state();
  /// What a section has open full screen (for the app bar), if anything.
  let characterOpen = $state<string | null>(null);
  let noteOpen = $state<string | null>(null);
  /// A note to open in the Notes section.
  let notePath = $state<string | null>(null);
  let drawer = $state(false);
  /// The entry open full-screen on a phone (its name), if any.
  let reading = $state<string | null>(null);

  const label = (s: Section) => SECTIONS.find(([id]) => id === s)![1];

  function go(s: string) {
    section = s as Section;
  }

  /// Android's back button: close what is open, step back inside the
  /// section, go back to the compendium, and only then leave.
  function back() {
    const focused = document.activeElement;
    if (closeTop()) {
      // closed a dialog
    } else if (drawer) {
      drawer = false;
    } else if (focused instanceof HTMLInputElement) {
      focused.blur();
    } else {
      stepBack().then((done) => {
        if (done) return;
        if (section !== "compendium" && status?.loaded) section = "compendium";
        else exit(0);
      });
    }
  }

  /// One step back inside the section.
  async function stepBack(): Promise<boolean> {
    if (section === "compendium") return compendium?.stepBack() ?? false;
    if (section === "characters") return (await characters?.back()) ?? false;
    if (section === "notes") return (await notes?.stepBack()) ?? false;
    return false;
  }

  /// The app bar's title: what is open, or the section.
  const title = $derived(
    (section === "compendium" && narrow.current && reading) ||
      (section === "characters" && characterOpen) ||
      (section === "notes" && narrow.current && noteOpen) ||
      label(section).toLowerCase(),
  );
  const opened = $derived(
    (section === "compendium" && narrow.current && !!reading) || (section === "characters" && !!characterOpen) || (section === "notes" && narrow.current && !!noteOpen),
  );

  function openNote(path: string) {
    notePath = path;
    section = "notes";
  }

  onMount(() => {
    const listener = onBackButtonPress(back);
    return () => void listener.then((l) => l.unregister());
  });

  onMount(async () => {
    try {
      status = await api.status();
      if (!status.loaded) section = "data";
      else if (!status.campaigns.length) section = "campaigns";
      // Development: `#characters/<slug>` (or VITE_OPEN) opens a section.
      const target = import.meta.env.DEV ? location.hash.slice(1) || import.meta.env.VITE_OPEN : "";
      if (target) {
        const [sec, slug] = target.split("/");
        if (SECTIONS.some(([id]) => id === sec)) section = sec as Section;
        if (sec === "characters" && slug) await api.openCharacter(slug);
      }
    } catch (e) {
      error = String(e);
    }
  });

  function onkey(e: KeyboardEvent) {
    if (e.altKey && e.key >= "1" && e.key <= String(SECTIONS.length)) {
      section = SECTIONS[Number(e.key) - 1][0];
      e.preventDefault();
    }
  }

  const counts = $derived(status?.loaded ? `${status.entries} entries · ${status.imported.length} sources` : "no data yet");
</script>

<svelte:window onkeydown={onkey} />

<div class="app" class:touch={touch.current}>
  {#if touch.current}
    <AppBar {title} back={opened} onmenu={() => (drawer = true)} onback={stepBack} />
    <Drawer
      bind:open={drawer}
      sections={SECTIONS.map(([id, l]) => [id, l, ""])}
      current={section}
      onpick={go}
      footer={status?.campaign ? `◈ ${status.campaign} · ${counts}` : counts}
    />
  {:else}
    <header class="tabs">
      <span class="brand">dnt</span>
      <nav class="no-bar">
        {#each SECTIONS as [id, l], i (id)}
          <button class:active={section === id} onclick={() => (section = id)}>
            <span class="n">{i + 1}</span>
            {l}
          </button>
        {/each}
      </nav>
      <span class="right">{#if status?.campaign}<span class="acc">◈</span> <b>{status.campaign}</b>{:else}<span class="dim">no campaign</span>{/if}</span>
    </header>
  {/if}

  <main>
    {#if error}
      <p class="err">{error}</p>
    {:else if !status}
      <p class="dim">Loading…</p>
    {:else if section === "compendium" && status.loaded}
      {#key `${status.entries} ${status.campaign}`}<Compendium
          bind:this={compendium}
          bind:reading
          touch={touch.current}
          narrow={narrow.current}
          campaign={status.campaigns.find((c) => c.active) ?? null}
          onstatus={(s) => (status = s)}
          oncampaigns={() => (section = "campaigns")}
        />{/key}
    {:else if section === "characters"}
      {#key status.campaign}<Characters bind:this={characters} bind:open={characterOpen} {status} onnote={openNote} />{/key}
    {:else if section === "notes"}
      {#key status.campaign}<Notes bind:this={notes} bind:openPath={notePath} bind:reading={noteOpen} {status} narrow={narrow.current} />{/key}
    {:else if section === "campaigns"}
      <Campaigns {status} onchange={(s) => (status = s)} />
    {:else}
      <Data {status} onchange={(s) => (status = s)} />
    {/if}
  </main>

  {#if !touch.current}
    <footer class="keys">
      {#if section === "compendium"}
        <span><span class="keycap">↑↓</span> move</span>
        <span><span class="keycap">/</span> search</span>
        <span><span class="keycap">tab</span> complete · category</span>
        <span><span class="keycap">esc</span> back</span>
      {/if}
      <span><span class="keycap">alt+1-5</span> section</span>
    </footer>
  {/if}
  <Busy />
</div>

<style>
  .app {
    height: 100vh;
    height: 100dvh;
    display: flex;
    flex-direction: column;
  }
  .app.touch {
    padding: env(safe-area-inset-top) env(safe-area-inset-right) 0 env(safe-area-inset-left);
  }
  .tabs {
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
  .acc {
    color: var(--accent);
  }
  main {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    padding: 0.75em 1ch 0.5em;
  }
  .touch main {
    padding: 12px 12px max(env(safe-area-inset-bottom), 24px);
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
</style>
