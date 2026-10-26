<!-- The app: on desktop a TUI-style header with the sections and a footer
     with the keys; on touch screens an app bar with a drawer. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { exit, onBackButtonPress } from "@tauri-apps/api/app";
  import * as api from "../lib/api";
  import AppBar from "../lib/AppBar.svelte";
  import Busy from "../lib/Busy.svelte";
  import Loading from "../lib/Loading.svelte";
  import Campaigns from "../lib/Campaigns.svelte";
  import Characters from "../lib/Characters.svelte";
  import Compendium from "../lib/Compendium.svelte";
  import Data from "../lib/Data.svelte";
  import Welcome from "../lib/Welcome.svelte";
  import Icon from "../lib/Icon.svelte";
  import Drawer from "../lib/Drawer.svelte";
  import Notes from "../lib/Notes.svelte";
  import { closeTop } from "../lib/modals";
  import { narrow, touch } from "../lib/screen";
  import { SHEET_TABS, type SheetTab } from "../lib/sheetctx";

  type Section = "compendium" | "characters" | "notes" | "campaigns" | "data";
  const SECTIONS: [Section, string][] = [
    ["compendium", "Compendium"],
    ["characters", "Characters"],
    ["notes", "Notes"],
    ["campaigns", "Campaigns"],
    ["data", "Data"],
  ];

  let section = $state<Section>("compendium");
  /// Getting started (a new app, or one just reset) instead of a section.
  let welcome = $state(false);
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
  /// The open character's tab, and the compendium's category with every
  /// one there is: the menu lists them under their sections.
  let sheetTab = $state<SheetTab>("Overview");
  let category = $state(0);
  let categories = $state<[string, number][]>([]);
  // A sheet, not a character being made (`new: <name>`).
  const sheetOpen = $derived(!!characterOpen && !characterOpen.startsWith("new: "));
  const menu = $derived(
    SECTIONS.map(([id, l]): [string, string, string, [string, string, string][]] => [
      id,
      l,
      "",
      id === "compendium" && status?.loaded
        ? categories.map(([c, n], i) => [String(i), c, String(n)])
        : id === "characters" && sheetOpen
          ? SHEET_TABS.map((t) => [t, t, ""])
          : [],
    ]),
  );
  const menuChild = $derived(section === "compendium" ? String(category) : section === "characters" && sheetOpen ? sheetTab : "");

  /// A section from the menu, or something inside it.
  function pick(id: string, child?: string) {
    if (child !== undefined && id === "compendium") {
      category = Number(child);
      // Shown already: show that category (its list, not an open entry).
      if (section === "compendium" && !welcome) compendium?.showCategory(category);
    } else if (child !== undefined && id === "characters") {
      sheetTab = child as SheetTab;
    }
    go(id);
  }
  /// The entry open full-screen on a phone (its name), if any.
  let reading = $state<string | null>(null);

  const label = (s: Section) => SECTIONS.find(([id]) => id === s)![1];

  function go(s: string) {
    section = s as Section;
    welcome = false;
    returnTo = null;
  }

  /// A note opened from somewhere else (a character's): Back returns there.
  let returnTo = $state<Section | null>(null);

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
    if (section === "notes") {
      // Opened from a character: back to it once there is no note to go back to.
      if (returnTo && !notes?.canGoBack()) {
        await notes?.save();
        section = returnTo;
        returnTo = null;
        return true;
      }
      return (await notes?.stepBack()) ?? false;
    }
    return false;
  }

  /// The compendium's actions in the app bar: a new homebrew entry, and
  /// its sources.
  const appActions = $derived.by(() => {
    if (welcome || section !== "compendium" || opened || !status?.loaded) return [];
    const c = status.campaigns.find((c) => c.active);
    const books = !c?.sources.length ? "all" : String(c.sources.length);
    const label = c?.homebrew.length ? `${books} +${c.homebrew.length}` : books;
    return [
      { label: "+", name: "New homebrew entry", onclick: () => compendium?.newEntry() },
      { label: `@ ${label}`, name: "Sources", onclick: () => compendium?.openSources() },
    ];
  });

  /// The app bar's title: what is open, or the section.
  const title = $derived(
    (section === "compendium" && narrow.current && reading) ||
      (section === "characters" && characterOpen) ||
      (section === "notes" && narrow.current && noteOpen) ||
      (welcome ? "getting started" : label(section).toLowerCase()),
  );
  const opened = $derived(
    (section === "compendium" && narrow.current && !!reading) || (section === "characters" && !!characterOpen) || (section === "notes" && ((narrow.current && !!noteOpen) || !!returnTo)),
  );

  function openNote(path: string) {
    notePath = path;
    returnTo = section === "characters" ? "characters" : null;
    section = "notes";
  }

  onMount(() => {
    const listener = onBackButtonPress(back);
    return () => void listener.then((l) => l.unregister());
  });

  /// What the backend is loading, until it is ready.
  let stage = $state("");

  onMount(async () => {
    try {
      // Ask what is loading (it answers at once) until it is done.
      for (;;) {
        const s = await api.startup();
        if (s.ready) break;
        stage = s.stage;
        await new Promise((r) => setTimeout(r, 150));
      }
      status = await api.status();
      welcome = !status.loaded || !status.campaigns.length;
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
      go(SECTIONS[Number(e.key) - 1][0]);
      e.preventDefault();
    }
  }

  const counts = $derived(status?.loaded ? `${status.entries} entries · ${status.imported.length} sources` : "no data yet");
</script>

<svelte:window onkeydown={onkey} />

<div class="app" class:touch={touch.current}>
  {#if touch.current}
    <AppBar {title} back={opened} onmenu={() => (drawer = true)} onback={stepBack} actions={appActions} />
    <Drawer
      bind:open={drawer}
      sections={menu}
      current={welcome ? "" : section}
      currentChild={menuChild}
      onpick={pick}
      footer={status?.campaign ? `◈ ${status.campaign} · ${counts}` : counts}
    />
  {:else}
    <header class="tabs">
      <img class="brand" src="/dnt.svg" alt="dnt" />
      <nav class="no-bar">
        {#each SECTIONS as [id, l], i (id)}
          <button class:active={!welcome && section === id} onclick={() => go(id)}>
            <span class="n">{i + 1}</span>
            <Icon name={id} />
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
      <Loading {stage} />
    {:else if welcome}
      <Welcome {status} onchange={(s) => (status = s)} ondone={go} />
    {:else if section === "compendium" && status.loaded}
      {#key `${status.entries} ${status.campaign}`}<Compendium
          bind:this={compendium}
          bind:reading
          bind:category
          bind:categories
          touch={touch.current}
          narrow={narrow.current}
          campaign={status.campaigns.find((c) => c.active) ?? null}
          onstatus={(s) => (status = s)}
          oncampaigns={() => (section = "campaigns")}
        />{/key}
    {:else if section === "characters"}
      {#key status.campaign}<Characters bind:this={characters} bind:open={characterOpen} bind:tab={sheetTab} {status} onnote={openNote} />{/key}
    {:else if section === "notes"}
      {#key status.campaign}<Notes bind:this={notes} bind:openPath={notePath} bind:reading={noteOpen} {status} narrow={narrow.current} />{/key}
    {:else if section === "campaigns"}
      <Campaigns {status} onchange={(s) => (status = s)} />
    {:else}
      <Data {status} onchange={(s) => (status = s)} onreset={() => (welcome = true)} />
    {/if}
  </main>

  {#if !touch.current}
    <footer class="keys">
      {#if section === "compendium" && !welcome}
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
    height: 1.3em;
    width: auto;
    flex-shrink: 0;
    align-self: center;
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
