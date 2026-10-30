<!-- The data, in tabs: Import (zips of a whole data folder, homebrew files
     or anything exported from dnt; what has been imported; starting over),
     Rules (the rules library), Backups, Problems (while there are some) and
     About. -->
<script lang="ts">
  import { getVersion } from "@tauri-apps/api/app";
  import { openFile, pickFolder } from "./files";
  import * as api from "./api";
  import { working } from "./working.svelte";
  import { ICONS } from "./icons";
  import Modal from "./Modal.svelte";
  import RulesLibrary from "./RulesLibrary.svelte";
  import ReleaseNotes from "./ReleaseNotes.svelte";
  import { checkForUpdate, installUpdate, update } from "./update.svelte";
  import Pane from "./Pane.svelte";
  import TabSelect from "./TabSelect.svelte";
  import { dataTabs, type DataTab } from "./datatabs";
  import type { IconName } from "./icons";

  let version = $state("");
  getVersion().then((v) => (version = v));

  let {
    status,
    onchange,
    onreset = () => {},
    tab = $bindable("Import"),
  }: { status: api.Status | null; onchange: (s: api.Status) => void; onreset?: () => void; tab?: DataTab } = $props();

  const tabs = $derived(dataTabs(status));
  // The problems solved: back to the first tab.
  $effect(() => {
    if (!tabs.some(([t]) => t === tab)) tab = "Import";
  });
  const TAB_ICONS: Record<DataTab, IconName> = { Import: "data", Rules: "features", Backups: "shield", Problems: "warning", About: "d20" };

  /// What starting over would remove, while asking.
  type Reset = "homebrew" | "data" | "all";
  let resetting: Reset | null = $state(null);

  const RESETS: Record<Reset, { title: string; goes: string; stays: string; action: string; label: string }> = {
    homebrew: {
      title: "Remove the imported homebrew",
      goes: "every imported homebrew source",
      stays: "The official data, campaigns, characters, notes and each campaign's own homebrew stay. Import homebrew again to have it back.",
      action: "Remove homebrew",
      label: "Removing the homebrew…",
    },
    data: {
      title: "Remove all imported data",
      goes: "all imported data: the official sources and the homebrew",
      stays: "Campaigns, characters, notes and each campaign's own homebrew stay; import a zip again to use them.",
      action: "Remove the data",
      label: "Removing the data…",
    },
    all: {
      title: "Reset the app",
      goes: "everything: the imported data, every campaign with its characters, notes and homebrew, and the settings",
      stays: "The app starts as new. This cannot be undone.",
      action: "Delete everything",
      label: "Resetting…",
    },
  };

  /// Who drew the icons, for the credits.
  const iconAuthors = [...new Set(Object.values(ICONS).map((i) => i.author))].sort();

  async function reset(what: Reset) {
    error = "";
    result = null;
    try {
      const s = await working(RESETS[what].label, () => (what === "all" ? api.resetApp() : api.clearImported(what === "homebrew")));
      resetting = null;
      onchange(s);
      if (what !== "homebrew") onreset();
    } catch (e) {
      error = String(e);
    }
  }

  let busy = $state(false);
  let result: api.Imported | null = $state(null);
  /// What an import of a character, campaign or entries did.
  let done = $state("");
  let error = $state("");
  /// A picked file that asks first: a clash, missing books, or a backup.
  let asking: { path: string; plan: api.ImportPlan } | null = $state(null);

  /// Import any file: data or homebrew, a character, a campaign, entries,
  /// or a backup to restore.
  async function pick() {
    error = "";
    done = "";
    result = null;
    const path = await openFile();
    if (!path) return;
    try {
      const plan = await api.importPlan(path);
      if (plan.what === "data") return importData(path);
      if (plan.what === "backup" || plan.clash || plan.missing.length) asking = { path, plan };
      else await importAs(path, "refuse");
    } catch (e) {
      error = String(e);
    }
  }

  async function importData(path: string) {
    busy = true;
    try {
      result = await working("Importing…", () => api.importZip(path));
      onchange(result.status);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function importAs(path: string, onClash: api.OnClash) {
    asking = null;
    busy = true;
    try {
      const r = await working("Importing…", () => api.importFile(path, onClash));
      done = r.summary;
      onchange(r.status);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  // ---- backups ----
  let backups: api.Backups | null = $state(null);
  api.backups().then((b) => (backups = b), () => {});
  /// A backup to restore, while asking: a picked file or one of the list.
  let restoring: { path: string; name: string } | null = $state(null);
  const EVERY: [api.Every, string][] = [
    ["off", "off"],
    ["day", "every day"],
    ["week", "every week"],
    ["month", "every month"],
  ];

  async function backUp() {
    error = "";
    try {
      backups = await working("Backing up…", () => api.backupNow());
      done = `Backed up to ${backups.list[0]?.name}`;
    } catch (e) {
      error = String(e);
    }
  }

  async function schedule(every: api.Every, keep: number) {
    try {
      backups = await api.backupSchedule(every, Math.max(1, keep));
    } catch (e) {
      error = String(e);
    }
  }

  /// Back up into another folder (null: the default one).
  async function moveBackups(folder: string | null) {
    error = "";
    try {
      backups = await api.backupFolder(folder);
      done = `Backups go to ${backups.folder}`;
    } catch (e) {
      error = String(e);
    }
  }

  async function restore(path: string) {
    restoring = null;
    asking = null;
    error = "";
    try {
      onchange(await working("Restoring…", () => api.restoreBackup(path)));
      backups = await api.backups();
      done = "Restored the backup; what was here is backed up first";
    } catch (e) {
      error = String(e);
    }
  }

  const size = (bytes: number) => (bytes < 1e6 ? `${Math.ceil(bytes / 1e3)} kB` : `${(bytes / 1e6).toFixed(1)} MB`);
  /// The rules library, open.
  let library = $state(false);
  /// Whether dnt checks for updates by itself.
  let updates: api.UpdateState | null = $state(null);
  api.updateState().then((u) => (updates = u));
  async function setAuto(on: boolean) {
    updates = await api.updateAuto(on);
  }
</script>

<div class="data">
  <div class="inner">
  <TabSelect label="Data" options={tabs} value={tab} icon={TAB_ICONS[tab]} onpick={(t) => (tab = t as DataTab)} />
  <!-- What the last action did, whichever tab it came from. -->
  {#if done}<p class="ok">✓ {done}</p>{/if}
  {#if error}<p class="err">{error}</p>{/if}
  {#if tab === "Import"}
  <Pane title="Import" focused>
    <p>
      The rules of the SRD 5.2.1 are built in. Your own books and homebrew come from data you import, in the format
      described in the project's <span class="muted">schemas/</span>: a <span class="b">zip</span> of a whole data
      folder, or of homebrew files in any layout; or a character, a campaign or an entry exported from dnt.
    </p>
    <p class="dim">
      Importing again updates entries instead of doubling them; entries a zip does not have are kept.
    </p>
    <p class="dim">
      Homebrew (files that name their sources) is kept apart and stays off until a campaign enables it, under
      Sources › Homebrew. For a large homebrew collection, zip just its .json files, not its images.
    </p>
    <div>
      <button class="action" onclick={pick} disabled={busy}>{busy ? "Importing…" : "Import…"}</button>
    </div>
    {#if result}
      <p class="ok">✓ {result.summary}</p>
      {#if result.skipped.length}
        <details>
          <summary class="dim">{result.skipped.length} file(s) skipped</summary>
          <ul class="dim">
            {#each result.skipped.slice(0, 50) as [file, why] (file)}<li>{file}: {why}</li>{/each}
          </ul>
        </details>
      {/if}
    {/if}
  </Pane>

  {#if status?.homebrew[0]}
    <Pane title="Imported homebrew">
      <p>
        <b>{status.homebrew[0]}</b> sources, <b>{status.homebrew[1]}</b> entries.
        <span class="dim">Enable them per campaign: Campaigns › Sources › Homebrew, or the compendium's @ button.</span>
      </p>
    </Pane>
  {/if}

  {#if status?.imported.length}
    <Pane title="Imported sources">
      <details>
        <summary>{status.imported.length} sources, {status.entries} entries</summary>
        <div class="sources">
          {#each status.imported as [source, n] (source)}
            <div><span>{source}</span> <span class="dim">{n}</span></div>
          {/each}
        </div>
      </details>
    </Pane>
  {/if}

  {#if status?.loaded || status?.campaigns.length}
    <Pane title="Start over">
      <p class="dim">If an import went wrong, remove what was imported and import again.</p>
      <div class="resets">
        {#if status.homebrew[0]}<button class="ghost" onclick={() => (resetting = "homebrew")}>Remove the imported homebrew</button>{/if}
        {#if status.loaded}<button class="ghost" onclick={() => (resetting = "data")}>Remove all imported data</button>{/if}
        <button class="ghost danger" onclick={() => (resetting = "all")}>Reset the app</button>
      </div>
    </Pane>
  {/if}
  {:else if tab === "Rules"}
  <Pane title="Rules library" focused>
    <p class="dim">
      Rules make the sheet do a feature's maths (its uses, the Armor Class or speed it gives). The rules library
      (<span class="url">github.com/v0v3s/dnt-rules</span>) has rules for the SRD and for homebrew that dnt does not
      have built in: pick some, or a whole folder, and they become your rules.
    </p>
    <div><button class="action" onclick={() => (library = true)}>Open the rules library…</button></div>
  </Pane>
  {:else if tab === "Backups"}
  <Pane title="Backups" focused>
    <p class="dim">
      Everything (campaigns with their characters and notes, the imported data, your homebrew and rules) in one zip, to
      restore here or on another device.
    </p>
    <div class="resets">
      <button class="action" onclick={backUp}>Back up now</button>
      <button class="ghost" onclick={async () => {
        const path = await openFile();
        if (path) restoring = { path, name: path.split(/[/\\]/).pop() ?? path };
      }}>Restore from a file…</button>
    </div>
    {#if backups}
      <div class="schedule">
        <label>
          <span class="dim">Automatically</span>
          <select aria-label="Automatic backups" value={backups.every} onchange={(e) => schedule(e.currentTarget.value as api.Every, backups!.keep)}>
            {#each EVERY as [v, label] (v)}<option value={v}>{label}</option>{/each}
          </select>
        </label>
        {#if backups.every !== "off"}
          <span class="keep">
            <span class="dim">keep</span>
            <button class="step" aria-label="Keep fewer" onclick={() => schedule(backups!.every, backups!.keep - 1)}>−</button>
            <b>{backups.keep}</b>
            <button class="step" aria-label="Keep more" onclick={() => schedule(backups!.every, backups!.keep + 1)}>+</button>
          </span>
        {/if}
      </div>
      <div class="where">
        <p class="dim folder">In {backups.folder}</p>
        <button class="ghost" onclick={async () => {
          const folder = await pickFolder();
          if (folder) moveBackups(folder);
        }}>Change…</button>
        {#if backups.chosen}<button class="ghost" onclick={() => moveBackups(null)}>Default</button>{/if}
      </div>
      {#each backups.list as b (b.name)}
        <div class="saved">
          <span class="bname">{b.name}</span>
          <span class="dim">{size(b.bytes)}</span>
          <button class="ghost" onclick={() => (restoring = { path: b.path, name: b.name })}>Restore</button>
        </div>
      {/each}
    {/if}
  </Pane>
  {:else if tab === "Problems"}
    <Pane title="Problems" focused>
      {#each status?.problems ?? [] as p (p)}<p class="err">{p}</p>{/each}
    </Pane>
  {:else}
  <Pane title="Updates" focused>
    <p>dnt {version}</p>
    <div class="row">
      <button class="action" onclick={() => checkForUpdate(true)}>Check for updates</button>
    </div>
    {#if update.newer}
      <p class="ok">dnt {update.newer.version} is out.</p>
      <ReleaseNotes text={update.newer.notes} />
      <div><button class="primary" onclick={installUpdate}>Update to {update.newer.version}</button></div>
    {:else if update.newest}
      <p class="ok">✓ dnt {version} is the newest.</p>
    {/if}
    {#if update.message}<p class="ok">{update.message}</p>{/if}
    {#if update.error}<p class="err">{update.error}</p>{/if}
    <label class="check">
      <input type="checkbox" checked={updates?.auto ?? true} onchange={(e) => setAuto(e.currentTarget.checked)} />
      Check for a newer dnt once a day
    </label>
  </Pane>

  <Pane title="About">
    <p>Dungeons &amp; Terminals: a D&amp;D 5e companion. It has the SRD 5.2.1 built in, and reads the books you import.</p>
    <p>dnt {version} · MIT license · <span class="url">github.com/v0v3s/dungeons-n-terminals</span></p>
    <p class="dim">
      Icons by {iconAuthors.join(" and ")} from <span class="url">game-icons.net</span>, under CC BY 3.0
      (<span class="url">creativecommons.org/licenses/by/3.0</span>).
    </p>
    <p class="dim srd">
      This work includes material from the System Reference Document 5.2.1 ("SRD 5.2.1") by Wizards of the Coast
      LLC, available at <span class="url">https://www.dndbeyond.com/srd</span>. The SRD 5.2.1 is licensed under the
      Creative Commons Attribution 4.0 International License, available at
      <span class="url">https://creativecommons.org/licenses/by/4.0/legalcode</span>.
    </p>
    <p class="dim">Not affiliated with Wizards of the Coast.</p>
  </Pane>

  {#if status?.timings.length}
    <Pane title="Last load">
      <div class="timings">
        {#each status.timings as [stage, ms], i (i)}
          <div><span>{stage}</span> <span class="dim">{ms < 1000 ? `${ms} ms` : `${(ms / 1000).toFixed(1)} s`}</span></div>
        {/each}
      </div>
    </Pane>
  {/if}
  {/if}
  </div>
</div>

{#if asking}
  {@const a = asking}
  {#if a.plan.what === "backup"}
    <Modal title="Restore this backup?" onclose={() => (asking = null)}>
      <p>Everything here is replaced by the backup: campaigns, characters, notes and the data.</p>
      <p class="dim">What is here now is backed up first, so you can go back to it.</p>
      <div class="buttons">
        <button class="ghost" onclick={() => (asking = null)}>Cancel</button>
        <button class="primary danger" onclick={() => restore(a.path)}>Restore</button>
      </div>
    </Modal>
  {:else}
    <Modal title="Import {a.plan.name}" onclose={() => (asking = null)}>
      {#if a.plan.missing.length}
        <p>It needs {a.plan.missing.join(", ")}, which {a.plan.missing.length === 1 ? "is" : "are"} not here: import {a.plan.missing.length === 1 ? "it" : "them"} too for everything to show.</p>
      {/if}
      {#if a.plan.clash}
        <p><b>{a.plan.clash}</b> is here already.</p>
        <div class="buttons">
          <button class="ghost" onclick={() => (asking = null)}>Cancel</button>
          <button class="ghost" onclick={() => importAs(a.path, "keep_both")}>Keep both</button>
          <button class="primary" onclick={() => importAs(a.path, "replace")}>Replace</button>
        </div>
      {:else}
        <div class="buttons">
          <button class="ghost" onclick={() => (asking = null)}>Cancel</button>
          <button class="primary" onclick={() => importAs(a.path, "refuse")}>Import</button>
        </div>
      {/if}
    </Modal>
  {/if}
{/if}

{#if restoring}
  {@const r = restoring}
  <Modal title="Restore this backup?" onclose={() => (restoring = null)}>
    <p><b>{r.name}</b> replaces everything here: campaigns, characters, notes and the data.</p>
    <p class="dim">What is here now is backed up first, so you can go back to it.</p>
    <div class="buttons">
      <button class="ghost" onclick={() => (restoring = null)}>Cancel</button>
      <button class="primary danger" onclick={() => restore(r.path)}>Restore</button>
    </div>
  </Modal>
{/if}

{#if resetting}
  {@const r = RESETS[resetting]}
  {@const what = resetting}
  <Modal title={r.title} onclose={() => (resetting = null)}>
    <p>This deletes {r.goes}.</p>
    <p class="dim">{r.stays}</p>
    <div class="buttons">
      <button class="ghost" onclick={() => (resetting = null)}>Cancel</button>
      <button class="primary" class:danger={what !== "homebrew"} onclick={() => reset(what)}>{r.action}</button>
    </div>
  </Modal>
{/if}

{#if library}<RulesLibrary onclose={() => (library = false)} />{/if}

<style>
  .url {
    color: var(--link);
  }
  .check {
    display: flex;
    gap: 1ch;
    align-items: center;
    margin-top: 0.6em;
  }
  .srd .url {
    overflow-wrap: anywhere;
  }
  summary {
    cursor: pointer;
    color: var(--gray);
  }
  details[open] summary {
    margin-bottom: 0.5em;
  }
  .resets {
    display: flex;
    flex-wrap: wrap;
    gap: 1ch;
  }
  .ghost.danger {
    color: var(--red);
  }
  .primary.danger {
    background: var(--red);
    border-color: var(--red);
  }
  .buttons {
    display: flex;
    justify-content: flex-end;
    gap: 1.5ch;
    margin-top: 1em;
  }
  @media (pointer: coarse) {
    .resets {
      flex-direction: column;
    }
    .buttons button {
      flex: 1;
    }
  }
  .schedule {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 1ch 2ch;
    margin-top: 0.75em;
  }
  .schedule label,
  .keep {
    display: flex;
    align-items: center;
    gap: 1ch;
  }
  .step {
    min-width: 2.4em;
    min-height: 2.2em;
  }
  .where {
    display: flex;
    align-items: center;
    gap: 1ch;
    margin: 0.5em 0 0.25em;
  }
  .folder {
    flex: 1;
    min-width: 0;
    margin: 0;
    overflow-wrap: anywhere;
  }
  .saved {
    display: flex;
    align-items: center;
    gap: 2ch;
    padding: 0.25em 0;
    border-top: 1px solid rgb(65 72 104 / 0.35);
  }
  .saved .bname {
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .timings div {
    display: flex;
    justify-content: space-between;
    gap: 2ch;
  }
  /* The whole area scrolls, with its scrollbar just inside the window's
     edge (most of main's padding); the content keeps clear of it. */
  .data {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    margin-right: calc(5px - 1ch);
  }
  .inner {
    display: flex;
    flex-direction: column;
    gap: 2em;
    max-width: calc(80ch + 2ch);
    padding: 0.8em 2ch 1em 0;
  }
  /* Panes keep their height; the area scrolls instead. */
  .inner > :global(.pane) {
    flex-shrink: 0;
  }
  p {
    margin: 0 0 1em;
  }
  .b {
    font-weight: 700;
  }
  .action {
    background: var(--accent);
    color: var(--black);
    font-weight: 700;
    border: 0;
    border-radius: 3px;
    padding: 0.4em 2ch;
    cursor: pointer;
  }
  @media (pointer: coarse) {
    .action {
      min-height: 48px;
      width: 100%;
      font-size: 1.05em;
    }
    .inner {
      padding-right: 4px;
    }
  }
  .action:disabled {
    opacity: 0.6;
  }
  .ok {
    color: var(--green);
    margin-top: 1em;
  }
  .err {
    color: var(--red);
    margin-top: 1em;
  }
  .sources {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(20ch, 1fr));
    column-gap: 2ch;
  }
</style>
