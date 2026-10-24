<!-- A character sheet: the header, the tabs, and the dialogs they use. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { save } from "@tauri-apps/plugin-dialog";
  import * as api from "./api";
  import Ask from "./Ask.svelte";
  import EntryModal from "./EntryModal.svelte";
  import FeaturesTab from "./FeaturesTab.svelte";
  import InventoryTab from "./InventoryTab.svelte";
  import Modal from "./Modal.svelte";
  import OverviewTab from "./OverviewTab.svelte";
  import Picker from "./Picker.svelte";
  import Prompt from "./Prompt.svelte";
  import RulesEditor from "./RulesEditor.svelte";
  import Icon from "./Icon.svelte";
  import type { IconName } from "./icons";
  import SpellsTab from "./SpellsTab.svelte";
  import type { SheetCtx } from "./sheetctx";

  let {
    sheet = $bindable(),
    onclose,
    onnote,
  }: { sheet: api.Sheet; onclose: () => void; onnote: (path: string) => void } = $props();

  const TABS = ["Overview", "Features", "Spells", "Inventory", "Notes"] as const;
  const TAB_ICONS: Record<(typeof TABS)[number], IconName> = {
    Overview: "characters",
    Features: "features",
    Spells: "spells",
    Inventory: "inventory",
    Notes: "notes",
  };
  type Tab = (typeof TABS)[number];
  let tab: Tab = $state("Overview");

  // ---- status line ----
  let message = $state("");
  let rolled: api.Rolled | null = $state(null);
  let timer: ReturnType<typeof setTimeout> | undefined;
  function say(m: string, r: api.Rolled | null = null) {
    message = m;
    rolled = r;
    clearTimeout(timer);
    timer = setTimeout(() => ((message = ""), (rolled = null)), r ? 9000 : 4000);
  }

  // ---- rolls into the session note ----
  /// Write each roll into the latest session note too (remembered here).
  let logRolls = $state(load());
  function load() {
    try {
      return localStorage.getItem("dnt.logRolls") === "1";
    } catch {
      return false;
    }
  }
  function setLogRolls(on: boolean) {
    logRolls = on;
    try {
      localStorage.setItem("dnt.logRolls", on ? "1" : "0");
    } catch {
      // A preference only.
    }
    say(on ? "Rolls also go into the latest session note" : "Rolls are no longer written into the session note");
  }
  async function log(line: string) {
    if (!logRolls) return;
    try {
      if ((await api.noteLog(line)) === null) say(`${line} · no session note yet (Notes: + Note in Sessions)`);
    } catch (e) {
      say(String(e));
    }
  }

  // ---- dialogs ----
  type Dialog =
    | { kind: "menu"; title: string; actions: [string, () => void][] }
    | { kind: "pick"; options: api.Options; then: (v: string[], o: api.Options) => void }
    | { kind: "prompt"; title: string; then: (v: string) => void; number?: boolean; value?: string; hint?: string }
    | { kind: "entry"; id: number }
    | { kind: "breakdown"; b: api.Breakdown }
    | { kind: "conditions"; list: [api.Condition, boolean][] }
    | { kind: "levelup"; step: api.Step }
    | { kind: "pending"; p: api.PendingChoice }
    | { kind: "confirm"; title: string; text: string; then: () => void; danger?: string }
    | { kind: "file"; text: string; error: string }
    | { kind: "rules"; entity: number }
    | { kind: "warnings" };
  let dialog: Dialog | null = $state(null);
  const close = () => (dialog = null);

  async function act(cmd: api.Command): Promise<boolean> {
    try {
      const done = await api.command(cmd);
      sheet = done.sheet;
      const r = done.outcome.rolled;
      say(done.outcome.message, r);
      if (r) log(done.outcome.message);
      return true;
    } catch (e) {
      say(String(e));
      return false;
    }
  }

  const ctx: SheetCtx = {
    act,
    async roll(what) {
      try {
        const r = await api.roll(what);
        sheet = r.sheet;
        say(r.line, r.rolled);
        log(r.line);
      } catch (e) {
        say(String(e));
      }
    },
    async pick(wanted, then) {
      try {
        const options = await api.options(wanted);
        if (!options.choices.length) return say("Nothing to choose from");
        dialog = { kind: "pick", options, then };
      } catch (e) {
        say(String(e));
      }
    },
    menu: (title, actions) => (dialog = { kind: "menu", title, actions }),
    prompt: (title, then, opts = {}) => (dialog = { kind: "prompt", title, then, ...opts }),
    show: (id) => (dialog = { kind: "entry", id }),
    rules: (entity) => (dialog = { kind: "rules", entity }),
    async explain(key) {
      const b = await api.breakdown(key);
      if (b) dialog = { kind: "breakdown", b };
    },
    async cycle() {
      sheet = await api.cycleAdvantage();
    },
    say,
  };

  async function conditions() {
    dialog = { kind: "conditions", list: await api.conditions() };
  }

  async function setCondition(name: string, on: boolean) {
    if (await act({ command: "set_condition", condition: name, on })) dialog = { kind: "conditions", list: await api.conditions() };
  }

  async function exhaustion(d: number) {
    const level = Math.max(0, Math.min(6, sheet.overview.exhaustion + d));
    if (await act({ command: "set_exhaustion", level })) dialog = { kind: "conditions", list: await api.conditions() };
  }

  async function levelUp() {
    try {
      dialog = { kind: "levelup", step: await api.levelupStart() };
    } catch (e) {
      say(String(e));
    }
  }

  async function levelAnswer(a: api.Answer) {
    try {
      dialog = { kind: "levelup", step: await api.levelupAnswer(a) };
    } catch (e) {
      say(String(e));
    }
  }

  async function levelApply() {
    try {
      const done = await api.levelupApply();
      sheet = done.sheet;
      say(done.outcome.message);
      dialog = null;
      nextPending();
    } catch (e) {
      say(String(e));
    }
  }

  /// Ask the next missing class choice, if any.
  async function nextPending() {
    const p = await api.pending();
    dialog = p ? { kind: "pending", p } : null;
  }

  async function pendingAnswer(p: api.PendingChoice, a: api.Answer) {
    if (a.type !== "pick") return;
    if (await act({ command: "make_choice", level: p.level, key: p.key, values: a.value })) nextPending();
  }

  function rest(long: boolean) {
    dialog = {
      kind: "confirm",
      title: long ? "Long rest" : "Short rest",
      text: long ? "Take a long rest? Hit points and most resources come back." : "Take a short rest?",
      then: () => act({ command: long ? "long_rest" : "short_rest" }),
    };
  }

  function more() {
    ctx.menu(sheet.header.name, [
      ["Level up…", levelUp],
      ...(sheet.header.pending ? ([[`Make ${sheet.header.pending} missing choice(s)…`, nextPending]] as [string, () => void][]) : []),
      ["Short rest", () => rest(false)],
      ["Long rest", () => rest(true)],
      [
        "Remove the latest level",
        () =>
          (dialog = {
            kind: "confirm",
            title: "Undo a level",
            text: "Remove the latest level and its choices?",
            then: () => act({ command: "undo_level" }),
          }),
      ],
      ["Open the character's note", openNote],
      ["Edit the character file…", editFile],
      [logRolls ? "Stop writing rolls into the session note" : "Write rolls into the session note", () => setLogRolls(!logRolls)],
      ["Export as a printable page…", () => exportAs("html")],
      ["Export as Markdown…", () => exportAs("markdown")],
      ["Delete the character…", remove],
    ]);
  }

  function remove() {
    dialog = {
      kind: "confirm",
      title: `Delete ${sheet.header.name}`,
      text: "The character is gone for good; their notes stay.",
      danger: "Delete",
      then: async () => {
        try {
          say(await api.deleteCharacter());
          onclose();
        } catch (e) {
          say(String(e));
        }
      },
    };
  }

  // A new tab starts at its top.
  let body: HTMLElement | undefined = $state();
  $effect(() => {
    void tab;
    body?.scrollTo({ top: 0 });
  });

  // ---- swiping between tabs ----
  /// Where a touch started; swipes from the screen's edges are the system's.
  let touchFrom: { x: number; y: number; t: number } | null = null;
  const EDGE = 40;
  const SWIPED = TABS.filter((t) => t !== "Notes");

  function swipeStart(e: TouchEvent) {
    const p = e.touches[0];
    touchFrom = e.touches.length === 1 && p.clientX > EDGE && p.clientX < window.innerWidth - EDGE ? { x: p.clientX, y: p.clientY, t: Date.now() } : null;
  }

  function swipeEnd(e: TouchEvent) {
    const from = touchFrom;
    touchFrom = null;
    const p = e.changedTouches[0];
    if (!from || !p || Date.now() - from.t > 600) return;
    const dx = p.clientX - from.x;
    const dy = p.clientY - from.y;
    // A clear sideways swipe, not a scroll.
    if (Math.abs(dx) < 70 || Math.abs(dx) < 2 * Math.abs(dy)) return;
    const at = SWIPED.indexOf(tab as (typeof SWIPED)[number]);
    const next = SWIPED[at + (dx < 0 ? 1 : -1)];
    if (next) tab = next;
  }

  /// Spells to choose: go to the Spells tab and choose the first that are
  /// missing (a feature's grant, else a class short of its spells).
  function chooseSpells() {
    tab = "Spells";
    const add = (o: api.Options, ids: string[]) => ctx.act({ command: "add_spells", to: o.target, ids });
    const choice = sheet.spells.find((r) => r.type === "choice");
    if (choice?.type === "choice") {
      return ctx.pick({ type: "grant_spells", key: choice.key, filter: choice.filter, missing: choice.missing }, (ids, o) => add(o, ids));
    }
    const short = sheet.spells.find((r) => r.type === "header" && r.class_id && r.counts.some(([, have, max]) => have < max));
    if (short?.type === "header" && short.class_id) {
      ctx.pick({ type: "class_spells", class_id: short.class_id }, (ids, o) => add(o, ids));
    }
  }

  /// The character's file as text: abilities, species, background and
  /// anything else, as stored.
  async function editFile() {
    try {
      dialog = { kind: "file", text: await api.characterText(), error: "" };
    } catch (e) {
      say(String(e));
    }
  }

  async function saveFile(text: string) {
    try {
      sheet = await api.characterSetText(text);
      close();
      say("Saved the character");
    } catch (e) {
      if (dialog?.kind === "file") dialog = { ...dialog, error: String(e) };
    }
  }

  /// Save the sheet as a page to print (or save as PDF from a browser), or
  /// as Markdown.
  async function exportAs(format: "html" | "markdown") {
    const ext = format === "html" ? "html" : "md";
    const path = await save({
      defaultPath: `${sheet.header.name}.${ext}`,
      filters: [{ name: format === "html" ? "Web page" : "Markdown", extensions: [ext] }],
    });
    if (!path) return;
    try {
      say(await api.exportSheet(format, path));
    } catch (e) {
      say(String(e));
    }
  }

  async function openNote() {
    try {
      onnote(await api.noteForCharacter(sheet.header.name));
    } catch (e) {
      say(String(e));
    }
  }

  onMount(() => {
    if (!sheet.header.level) say("No class yet: open ⋯ and take your first level");
  });
</script>

<div class="sheet">
  <div class="head">
    <div class="who">
      <h1><span class="acc">◆</span> {sheet.header.name}</h1>
      <div class="muted id">{sheet.header.identity}</div>
    </div>
    <div class="actions">
      {#if sheet.header.warnings.length}
        <button class="ghost icon warnbtn" aria-label="Warnings" onclick={() => (dialog = { kind: "warnings" })}><Icon name="warning" /></button>
      {/if}
      <button class="ghost icon" onclick={more} aria-label="More">⋯</button>
    </div>
  </div>
  {#if sheet.header.pending || sheet.header.spells_to_choose || !sheet.header.level}
    <div class="flags">
      {#if !sheet.header.level}<button class="flag" onclick={levelUp}>Take the first level</button>{/if}
      {#if sheet.header.pending}<button class="flag" onclick={nextPending}>{sheet.header.pending} {sheet.header.pending === 1 ? "choice" : "choices"}<span class="wide"> to make</span></button>{/if}
      {#if sheet.header.spells_to_choose}<button class="flag" onclick={chooseSpells}>{sheet.header.spells_to_choose} {sheet.header.spells_to_choose === 1 ? "spell" : "spells"}<span class="wide"> to choose</span></button>{/if}
    </div>
  {/if}

  <div class="tabs no-bar">
    {#each TABS as t (t)}
      <button class:active={tab === t} onclick={() => (t === "Notes" ? openNote() : (tab = t))}><Icon name={TAB_ICONS[t]} /> {t}</button>
    {/each}
  </div>

  <!-- Swiping sideways changes the tab (the tabs above do the same). -->
  <div class="body" role="presentation" bind:this={body} ontouchstart={swipeStart} ontouchend={swipeEnd}>
    {#if tab === "Overview"}
      <OverviewTab o={sheet.overview} {ctx} onconditions={conditions} />
    {:else if tab === "Features"}
      <FeaturesTab groups={sheet.features} {ctx} />
    {:else if tab === "Spells"}
      <SpellsTab rows={sheet.spells} slots={sheet.overview.trackers.filter((t) => t.id.startsWith("slot:") || t.id === "pact")} {ctx} />
    {:else if tab === "Inventory"}
      <InventoryTab inv={sheet.inventory} {ctx} />
    {/if}
  </div>

  {#if message}
    <div class="status" class:roll={rolled}>
      {#if rolled}
        <span class="rtitle"><Icon name="d20" /> {rolled.title}</span>
        <b class:crit={rolled.crit === true} class:fumble={rolled.crit === false}>{rolled.result}</b>
        <span class="dim">{rolled.detail}</span>
        {#each rolled.extra as e, i (i)}<div>{e}</div>{/each}
      {:else}{message}{/if}
    </div>
  {/if}
</div>

{#if dialog?.kind === "warnings"}
  <Modal title="Warnings" onclose={close}>
    <ul class="warnings">
      {#each sheet.header.warnings as w (w)}<li><Icon name="warning" /> {w}</li>{/each}
    </ul>
  </Modal>
{/if}

{#if dialog?.kind === "rules"}
  <RulesEditor
    entity={dialog.entity}
    character={sheet.header.name}
    onclose={close}
    onsaved={(s, m) => {
      sheet = s;
      close();
      say(m);
    }}
  />
{/if}

{#if dialog?.kind === "file"}
  {@const d = dialog}
  <Modal title="{sheet.header.name} · the file" onclose={close} wide>
    <p class="dim">The character as stored (TOML): abilities, species, background, levels and choices, inventory, notes. It is checked before saving.</p>
    <textarea class="file" bind:value={d.text} spellcheck="false" autocapitalize="off" autocomplete="off" rows="18"></textarea>
    {#if d.error}<p class="file-err">{d.error}</p>{/if}
    <div class="file-buttons">
      <button class="ghost" onclick={close}>Cancel</button>
      <button class="primary" onclick={() => saveFile(d.text)}>Save</button>
    </div>
  </Modal>
{/if}

{#if dialog?.kind === "menu"}
  {@const d = dialog}
  <Modal title={d.title} onclose={close}>
    <div class="menu">
      {#each d.actions as [label, run] (label)}
        <button
          onclick={() => {
            close();
            run();
          }}>{label}</button
        >
      {/each}
    </div>
  </Modal>
{:else if dialog?.kind === "pick"}
  {@const d = dialog}
  <Modal title={d.options.title} onclose={close} footer="enter choose · esc">
    <Picker
      choices={d.options.choices}
      count={d.options.pick}
      initial={d.options.default}
      onpick={(v) => {
        const { then, options } = d;
        close();
        then(v, options);
      }}
    />
  </Modal>
{:else if dialog?.kind === "prompt"}
  {@const d = dialog}
  <Prompt
    title={d.title}
    number={d.number}
    value={d.value}
    hint={d.hint}
    onclose={close}
    onsubmit={(v) => {
      const then = d.then;
      close();
      then(v);
    }}
  />
{:else if dialog?.kind === "entry"}
  <EntryModal id={dialog.id} onclose={close} />
{:else if dialog?.kind === "breakdown"}
  {@const b = dialog.b}
  <Modal title={b.title} onclose={close}>
    <div class="bvalue">{b.value}</div>
    {#each b.parts as p, i (i)}
      <div class="part" class:off={!p.applied}>
        <span class="acc">{p.applied ? "✓" : "·"}</span>
        <span class="dim">{p.kind}</span>
        <b>{p.value}</b>
        <span>{p.origin}{p.note ? ` (${p.note})` : ""}</span>
      </div>
    {/each}
  </Modal>
{:else if dialog?.kind === "conditions"}
  {@const d = dialog}
  <Modal title="Conditions" onclose={close}>
    <div class="conds">
      {#each d.list as [c, on] (c.name)}
        <button class="cond" class:on onclick={() => setCondition(c.name, !on)}>
          <span class="tick">{on ? "◉" : "○"}</span>
          <span class="cname">{c.name}</span>
          <span class="dim">{c.summary}</span>
        </button>
      {/each}
      <div class="exh">
        <span class="cname">Exhaustion</span>
        <button class="ghost" onclick={() => exhaustion(-1)}>−</button>
        <b>{sheet.overview.exhaustion}</b>
        <button class="ghost" onclick={() => exhaustion(1)}>+</button>
        <span class="dim">{sheet.overview.exhaustion_summary}</span>
      </div>
    </div>
  </Modal>
{:else if dialog?.kind === "levelup"}
  {@const d = dialog}
  <Modal title={sheet.header.level ? "Level up" : "First level"} onclose={close} wide>
    {#if d.step.summary.length}
      <div class="summary">{#each d.step.summary as s (s)}<div><span class="acc">✓</span> <span class="muted">{s}</span></div>{/each}</div>
    {/if}
    <Ask question={d.step.question} onanswer={levelAnswer} onconfirm={levelApply} confirm="Apply the level" />
  </Modal>
{:else if dialog?.kind === "pending"}
  {@const d = dialog}
  <Modal title="{d.p.from}: {d.p.key}" onclose={close} footer="{d.p.left} left">
    <Ask question={d.p.question} onanswer={(a) => pendingAnswer(d.p, a)} />
  </Modal>
{:else if dialog?.kind === "confirm"}
  {@const d = dialog}
  <Modal title={d.title} onclose={close}>
    <p>{d.text}</p>
    <div class="confirm">
      <button class="ghost" onclick={close}>Cancel</button>
      <button
        class="primary"
        class:danger={!!d.danger}
        onclick={() => {
          const then = d.then;
          close();
          then();
        }}>{d.danger ?? "Yes"}</button
      >
    </div>
  </Modal>
{/if}

<style>
  textarea.file {
    width: 100%;
    box-sizing: border-box;
    min-height: 16em;
    background: var(--black);
    color: var(--fg);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 0.6em 1ch;
    font: inherit;
    resize: vertical;
  }
  .file-err {
    color: var(--red);
  }
  .file-buttons {
    display: flex;
    justify-content: flex-end;
    gap: 1.5ch;
    margin-top: 1em;
  }
  .sheet {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    position: relative;
  }
  .head {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 2ch;
  }
  .who {
    min-width: 0;
  }
  .icon {
    min-width: 2.6em;
    font-size: 1.1em;
  }
  .warnbtn {
    color: var(--yellow);
    border-color: rgb(224 175 104 / 0.5);
  }
  .flags {
    display: flex;
    flex-wrap: wrap;
    gap: 1ch;
    margin-top: 0.6em;
  }
  .warnings {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 0.6em;
    color: var(--yellow);
  }
  h1 {
    font-size: 1.1em;
    margin: 0;
    color: var(--accent);
  }
  .acc {
    color: var(--accent);
  }
  .id {
    margin-top: 0.1em;
  }
  .actions {
    display: flex;
    gap: 1ch;
    align-items: center;
    flex-shrink: 0;
  }
  .flag {
    background: rgb(125 207 255 / 0.08);
    color: var(--cyan);
    border: 1px solid rgb(125 207 255 / 0.55);
    border-radius: 3px;
    padding: 0.1em 1ch;
    cursor: pointer;
  }
  .tabs {
    display: flex;
    gap: 1ch;
    margin: 0.75em 0 0.25em;
    overflow-x: auto;
    flex-shrink: 0;
  }
  .tabs button {
    background: none;
    border: 0;
    color: var(--gray);
    padding: 0 1ch;
    cursor: pointer;
    white-space: nowrap;
  }
  .tabs button.active {
    background: var(--cyan);
    color: var(--black);
    font-weight: 700;
  }
  .body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 0.4em 1ch 1.5em 0;
  }
  .status {
    position: sticky;
    bottom: 0;
    margin-top: 0.4em;
    padding: 0.4em 1.5ch;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bar);
  }
  .status.roll {
    border-color: var(--accent);
  }
  .status b {
    color: var(--accent);
    font-size: 1.2em;
    margin: 0 1ch;
  }
  .status b.crit {
    color: var(--green);
  }
  .status b.fumble {
    color: var(--red);
  }
  .menu {
    display: flex;
    flex-direction: column;
  }
  .menu button {
    background: none;
    border: 0;
    text-align: left;
    padding: 0.3em 1ch;
    cursor: pointer;
  }
  .menu button:hover {
    background: var(--selected);
  }
  .bvalue {
    font-size: 1.4em;
    font-weight: 700;
    color: var(--accent);
    margin-bottom: 0.5em;
  }
  .part {
    display: grid;
    grid-template-columns: 2ch 9ch 8ch 1fr;
    gap: 1ch;
  }
  .part.off {
    opacity: 0.55;
  }
  .conds {
    display: flex;
    flex-direction: column;
  }
  .cond {
    display: grid;
    grid-template-columns: 2ch 15ch 1fr;
    gap: 1ch;
    background: none;
    border: 0;
    text-align: left;
    padding: 0.15em 0;
    cursor: pointer;
  }
  .cond .tick {
    color: var(--dim);
  }
  .cond.on .tick,
  .cond.on .cname {
    color: var(--magenta);
    font-weight: 700;
  }
  .exh {
    display: grid;
    grid-template-columns: 15ch 4ch 3ch 4ch 1fr;
    gap: 1ch;
    align-items: center;
    margin-top: 0.75em;
    padding-left: 3ch;
  }
  .exh b {
    text-align: center;
  }
  .summary {
    margin-bottom: 0.75em;
  }
  .confirm {
    display: flex;
    justify-content: flex-end;
    gap: 1.5ch;
  }
  .confirm .danger {
    background: var(--red);
    border-color: var(--red);
  }
  @media (max-width: 760px) {
    .icon {
      min-height: 44px;
      min-width: 44px;
    }
    .flag .wide {
      display: none;
    }
    /* The top bar has the name. */
    .who h1 {
      display: none;
    }
    .who {
      align-self: center;
    }
    .flag {
      min-height: 36px;
      padding: 0 1.5ch;
      border-radius: 18px;
    }
    .tabs button {
      height: 36px;
      border: 1px solid var(--border);
      border-radius: 18px;
      padding: 0 14px;
    }
    .tabs button.active {
      border-color: var(--cyan);
    }
    .menu button {
      min-height: 48px;
      border-top: 1px solid rgb(65 72 104 / 0.3);
    }
    .cond {
      grid-template-columns: 2ch 1fr;
      min-height: 48px;
      align-content: center;
    }
    .cond .dim {
      grid-column: 2;
      font-size: 0.85em;
    }
    .exh {
      grid-template-columns: 1fr 44px 4ch 44px;
      padding-left: 0;
    }
    .exh .dim {
      grid-column: 1 / -1;
    }
    .confirm button {
      flex: 1;
    }
  }
</style>
