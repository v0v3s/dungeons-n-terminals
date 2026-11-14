<!-- The app: on a computer a header with the sections, the search, the
     campaign and ☰ for a section's sidebar; on a touch screen, or in a
     window too narrow for the header, an app bar with a drawer. -->
<script lang="ts">
  import { onMount, tick } from "svelte";
  import { sidebar } from "../lib/sidebar.svelte";
  import UpdateNotice from "../lib/UpdateNotice.svelte";
  import { checkForUpdate } from "../lib/update.svelte";
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
  import Drawer from "../lib/Drawer.svelte";
  import Notes from "../lib/Notes.svelte";
  import { closeTop } from "../lib/modals";
  import { narrow, touch } from "../lib/screen";
  import { SHEET_TABS, type SheetTab } from "../lib/sheetctx";
  import { dataTabs, type DataTab } from "../lib/datatabs";

  type Section = "compendium" | "characters" | "notes" | "campaigns" | "data";
  const SECTIONS: [Section, string][] = [
    ["compendium", "Compendium"],
    ["characters", "Characters"],
    ["notes", "Notes"],
    ["campaigns", "Campaigns"],
    ["data", "Data"],
  ];

  let section = $state<Section>("compendium");
  /// The app bar and drawer: a touch screen, or a window too narrow for
  /// the header's sections.
  const mobile = $derived(touch.current || narrow.current);
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
  let dataTab = $state<DataTab>("Import");
  /// The open sheet's actions (warnings, ⋯): in the app bar on a phone.
  let sheetActions = $state<{ label: string; name: string; onclick: () => void }[]>([]);
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
          : id === "data"
            ? dataTabs(status).map(([t, l]) => [t, l, ""])
            : [],
    ]),
  );
  const menuChild = $derived(
    section === "compendium" ? String(category) : section === "characters" && sheetOpen ? sheetTab : section === "data" ? dataTab : "",
  );

  /// A section from the menu, or something inside it.
  function pick(id: string, child?: string) {
    if (child !== undefined && id === "compendium") {
      category = Number(child);
      // Shown already: show that category (its list, not an open entry).
      if (section === "compendium" && !welcome) compendium?.showCategory(category);
    } else if (child !== undefined && id === "characters") {
      sheetTab = child as SheetTab;
    } else if (child !== undefined && id === "data") {
      dataTab = child as DataTab;
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
    if (!welcome && section === "characters" && sheetOpen) return sheetActions;
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
      // The day's check for a newer dnt, in the background.
      checkForUpdate(false);
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

  /// The section has a sidebar for ☰ to hide and show.
  const hasSidebar = $derived(!welcome && section === "notes" && !!status?.campaign);

  /// The compendium, the cursor in its search (the header's search, Ctrl+K).
  async function search() {
    if (!status?.loaded) return;
    go("compendium");
    await tick();
    compendium?.focusSearch();
  }

  function onkey(e: KeyboardEvent) {
    if (e.altKey && e.key >= "1" && e.key <= String(SECTIONS.length)) {
      go(SECTIONS[Number(e.key) - 1][0]);
      e.preventDefault();
    } else if (e.ctrlKey && !e.altKey && e.key.toLowerCase() === "k") {
      e.preventDefault();
      search();
    }
  }

  const counts = $derived(status?.loaded ? `${status.entries} entries · ${status.imported.length} sources` : "no data yet");
</script>

<svelte:window onkeydown={onkey} />

<div class="app" class:touch={touch.current}>
  {#if mobile}
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
    <header class="top">
      {#if hasSidebar}
        <button class="icon" aria-label={sidebar.open ? "Hide the sidebar" : "Show the sidebar"} aria-expanded={sidebar.open} onclick={() => sidebar.toggle()}>
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><path d="M4 6h16M4 12h16M4 18h16" /></svg>
        </button>
      {/if}
      <img class="brand" src="/dnt.svg" alt="dnt" />
      <nav aria-label="Sections">
        {#each SECTIONS as [id, l], i (id)}
          <button class:active={!welcome && section === id} aria-current={!welcome && section === id ? "page" : undefined} title="Alt+{i + 1}" onclick={() => go(id)}>{l}</button>
        {/each}
      </nav>
      {#if status?.loaded}
        <button class="search" onclick={search}>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><circle cx="11" cy="11" r="7" /><path d="m20 20-4-4" /></svg>
          <span>Search the compendium…</span>
          <span class="kbd">Ctrl K</span>
        </button>
      {/if}
      <button class="campaign" class:none={!status?.campaign} onclick={() => go("campaigns")} title="Campaigns">
        {#if status?.campaign}<span class="acc">◆</span> {status.campaign}{:else}No campaign{/if}
      </button>
    </header>
  {/if}

  <main>
    {#if status}<UpdateNotice />{/if}
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
      {#key status.campaign}<Characters bind:this={characters} bind:open={characterOpen} bind:tab={sheetTab} bind:actions={sheetActions} {status} onnote={openNote} />{/key}
    {:else if section === "notes"}
      {#key status.campaign}<Notes bind:this={notes} bind:openPath={notePath} bind:reading={noteOpen} {status} narrow={narrow.current} />{/key}
    {:else if section === "campaigns"}
      <Campaigns {status} onchange={(s) => (status = s)} />
    {:else}
      <Data {status} bind:tab={dataTab} onchange={(s) => (status = s)} onreset={() => (welcome = true)} />
    {/if}
  </main>

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
  .top {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 56px;
    flex-shrink: 0;
    padding: 0 16px;
    background: var(--bar);
    border-bottom: 1px solid var(--dark);
    white-space: nowrap;
  }
  .top button {
    font: inherit;
    font-weight: 500;
    border-radius: 8px;
    cursor: pointer;
  }
  .icon {
    width: 36px;
    height: 36px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: 1px solid transparent;
    color: var(--fg);
  }
  .icon:hover {
    background: var(--raised);
  }
  .brand {
    height: 1.4em;
    width: auto;
    flex-shrink: 0;
    margin: 0 10px 0 4px;
  }
  nav {
    display: flex;
    gap: 2px;
  }
  nav button {
    height: 36px;
    padding: 0 14px;
    background: none;
    border: 0;
    color: var(--gray);
  }
  nav button:hover {
    background: var(--raised);
    color: var(--fg);
  }
  nav button.active {
    background: var(--raised);
    color: var(--accent);
  }
  .search {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 8px;
    height: 36px;
    width: min(300px, 28vw);
    min-width: 0;
    padding: 0 12px;
    background: var(--card);
    border: 1px solid var(--border);
    color: var(--dim);
    text-align: left;
  }
  .search > span:first-of-type {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    font-weight: 400;
  }
  .search:hover {
    border-color: var(--gray);
  }
  .kbd {
    font-family: var(--mono);
    font-size: 11px;
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 1px 5px;
    color: var(--gray);
  }
  .campaign {
    height: 36px;
    margin-left: 8px;
    padding: 0 14px;
    background: var(--raised);
    border: 1px solid var(--border);
    color: var(--fg);
  }
  .campaign.none {
    color: var(--dim);
  }
  nav + .campaign {
    margin-left: auto;
  }
  .campaign:hover {
    background: var(--hover);
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
  .err {
    color: var(--red);
  }
</style>
