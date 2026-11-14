<!-- A character sheet: the header, the tabs, and the dialogs they use. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { fade } from "svelte/transition";
  import { saveAs } from "./files";
  import * as api from "./api";
  import Ask from "./Ask.svelte";
  import EntryModal from "./EntryModal.svelte";
  import FeaturesTab from "./FeaturesTab.svelte";
  import InventoryTab from "./InventoryTab.svelte";
  import Modal from "./Modal.svelte";
  import OverviewTab from "./OverviewTab.svelte";
  import Picker from "./Picker.svelte";
  import Prompt from "./Prompt.svelte";
  import DiceRoll from "./DiceRoll.svelte";
  import RulesEditor from "./RulesEditor.svelte";
  import Icon from "./Icon.svelte";
  import type { IconName } from "./icons";
  import SpellsTab from "./SpellsTab.svelte";
  import { SHEET_TABS, type SheetCtx, type SheetTab } from "./sheetctx";
  import { Swipe } from "./swipe.svelte";
  import SkillsTab from "./SkillsTab.svelte";
  import { desktop, narrow, touch } from "./screen";
  import Tip from "./Tip.svelte";
  import { tips } from "./tips.svelte";

  let {
    sheet = $bindable(),
    tab = $bindable("Overview"),
    actions = $bindable([]),
    onclose,
    onnote,
  }: {
    sheet: api.Sheet;
    tab?: SheetTab;
    /// The sheet's own actions (warnings, the ⋯ menu), for the app bar on a phone.
    actions?: { label: string; name: string; onclick: () => void }[];
    onclose: () => void;
    onnote: (path: string) => void;
  } = $props();

  const TAB_ICONS: Record<SheetTab, IconName> = {
    Overview: "characters",
    Skills: "d20",
    Features: "features",
    Spells: "spells",
    Inventory: "inventory",
    Notes: "notes",
  };
  type Tab = SheetTab;
  // Notes is the character's note, in the Notes section: picking it (here
  // or from the menu) opens the note and leaves the sheet on its tab.
  let shown: Exclude<Tab, "Notes"> = $state("Overview");
  $effect(() => {
    if (tab === "Notes") {
      tab = shown;
      openNote();
    } else shown = tab;
  });

  // ---- status line ----
  let message = $state("");
  let rolled: api.Rolled | null = $state(null);
  let timer: ReturnType<typeof setTimeout> | undefined;
  /// A roll's line as its number, what it is and its dice: `7 piercing
  /// damage (1d6 + 2)`, `17 to hit`, `off hand 5 (1d6)`.
  function numbered(line: string) {
    const m = line.match(/^(.*?)([+-]?\d+)(.*?)(?:\s*\((.*)\))?$/);
    return m ? { num: m[2], label: `${m[1]} ${m[3]}`.trim(), dice: m[4] ?? "" } : null;
  }

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
    | { kind: "entry"; id: number; part?: string | null }
    | { kind: "breakdown"; b: api.Breakdown }
    | { kind: "conditions"; list: [api.Condition, boolean][] }
    | { kind: "levelup"; step: api.Step }
    | { kind: "pending"; p: api.PendingChoice }
    | { kind: "confirm"; title: string; text: string; then: () => void; danger?: string }
    | { kind: "file"; text: string; error: string }
    | { kind: "rules"; entity: number }
    | { kind: "warnings" }
    | { kind: "dice"; text: string; error: string };
  let dialog: Dialog | null = $state(null);
  /// How a number adds up, in a pop-up beside it (a computer).
  let pop: { b: api.Breakdown; key: string; x: number; y: number } | null = $state(null);

  /// The roll a breakdown's number makes, when it is one that rolls.
  function rollOf(key: string): api.Roll | null {
    const [kind, a, b] = key.split(".");
    if (kind === "mod") return { type: "ability", key: a };
    if (kind === "save") return { type: "save", key: a };
    if (kind === "skill") return { type: "skill", key: a };
    if (key === "initiative") return { type: "initiative" };
    if (kind === "attack" && b === "bonus") return { type: "attack", slug: a };
    return null;
  }

  /// A modifier is shown with its sign (+3, −1); a score or a count is not.
  const SIGNED = /^(skill|save|mod)\.|^initiative$|^prof$|^attack\..*\.bonus$/;
  const sign = (n: number) => (n >= 0 ? `+${n}` : `−${-n}`);
  const num = (s: string) => parseInt(s.replace("−", "-"), 10);
  const signed = (key: string, v: string) => (SIGNED.test(key) && /^[-−+]?\d+$/.test(v) ? sign(num(v)) : v);

  type Line = { kind: string; value: string; origin: string; applied: boolean };
  /// A breakdown's lines. A skill or a save is one part in the rules (its
  /// formula, "Intelligence + proficiency"): shown as its components, the
  /// ability modifier and the proficiency, from the sheet's own numbers;
  /// whatever else the rules add (items, features) follows as it is.
  function lines(key: string, b: api.Breakdown): Line[] {
    const as = (p: api.Part): Line => ({ kind: p.kind, value: p.value, origin: p.origin + (p.note ? ` (${p.note})` : ""), applied: p.applied });
    const o = sheet.overview;
    let ability: api.Ability | undefined;
    let prof: api.Skill["proficiency"] = "none";
    if (key.startsWith("skill.")) {
      const s = o.skills.find((s) => `skill.${s.key}` === key);
      ability = s && o.abilities.find((a) => a.key === s.ability);
      prof = s?.proficiency ?? "none";
    } else if (key.startsWith("save.")) {
      ability = o.abilities.find((a) => `save.${a.key}` === key);
      prof = ability?.proficient ? "proficient" : "none";
    }
    const [base, ...rest] = b.parts;
    const total = base ? num(base.value) : NaN;
    const mod = ability ? num(ability.modifier) : NaN;
    if (!ability || base?.kind !== "base" || Number.isNaN(total) || Number.isNaN(mod)) return b.parts.map(as);
    const out: Line[] = [{ kind: "ability", value: sign(mod), origin: `${ability.name} modifier (score ${ability.score})`, applied: true }];
    if (total !== mod) {
      const what = prof === "expertise" ? "Proficiency bonus, doubled (expertise)" : prof === "half" ? "Half the proficiency bonus" : "Proficiency bonus";
      out.push({ kind: "proficiency", value: sign(total - mod), origin: what, applied: true });
    }
    return [...out, ...rest.map(as)];
  }

  /// Roll with advantage, disadvantage or neither: the next roll's mode set
  /// first (as its chip cycles it), then the roll.
  async function rollWith(what: api.Roll, mode: string) {
    pop = null;
    for (let i = 0; i < 3 && sheet.overview.next_roll !== mode; i++) await ctx.cycle();
    await ctx.roll(what);
  }
  /// The last roll typed by hand, to roll again.
  let lastDice = "";

  $effect(() => {
    actions = [
      ...(sheet.header.warnings.length ? [{ label: "⚠", name: "Warnings", onclick: () => (dialog = { kind: "warnings" }) }] : []),
      { label: "⋯", name: "More", onclick: more },
    ];
  });
  /// Counts closings. A dialog opened while the last still fades out is
  /// that one again, so what holds a state in it (a filter, a draft) is
  /// keyed on this, and starts afresh.
  let closings = $state(0);
  const close = () => {
    dialog = null;
    closings++;
  };

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
    confirm(title, text, then, danger) {
      dialog = { kind: "confirm", title, text, then, danger };
    },
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
    dice: () => (dialog = { kind: "dice", text: lastDice, error: "" }),
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
    show: (id, part) => (dialog = { kind: "entry", id, part }),
    rules: (entity) => (dialog = { kind: "rules", entity }),
    async explain(key, near) {
      tips.done("sheet.hold");
      const b = await api.breakdown(key);
      if (!b) return;
      if (!desktop.current || !near) return void (dialog = { kind: "breakdown", b });
      // Beside what it explains: to its right, or its left near the edge.
      const r = near.getBoundingClientRect();
      const rem = parseFloat(getComputedStyle(document.documentElement).fontSize);
      const width = 26 * rem;
      const x = r.right + 8 + width < innerWidth - 8 ? r.right + 8 : Math.max(8, r.left - width - 8);
      const y = Math.max(8, Math.min(r.top, innerHeight - 20 * rem));
      pop = { b, key, x, y };
    },
    async cycle() {
      sheet = await api.cycleAdvantage();
    },
    say,
    async refresh() {
      sheet = await api.sheet();
    },
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
      close();
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
    ctx.menu(sheet.header.name, sheetActions());
  }

  /// What can be done with the whole character: the ⋯ menu, and on a
  /// computer a row of buttons as well.
  function sheetActions(): [string, () => void][] {
    return [
      ["Level up…", levelUp],
      ...(sheet.header.pending ? ([[`Make ${sheet.header.pending} missing choice(s)…`, nextPending]] as [string, () => void][]) : []),
      ["Short rest", () => rest(false)],
      ["Long rest", () => rest(true)],
      ["Remove the latest level", undoLevel],
      ["Open the character's note", openNote],
      ["Edit the character file…", editFile],
      [logRolls ? "Stop writing rolls into the session note" : "Write rolls into the session note", () => setLogRolls(!logRolls)],
      ["Export as a printable page…", () => exportAs("html")],
      ["Export as Markdown…", () => exportAs("markdown")],
      ["Export for dnt (to import elsewhere)…", exportForDnt],
      ["Delete the character…", remove],
      ...(touch.current ? ([["Show the tips again", () => tips.reset()]] as [string, () => void][]) : []),
    ];
  }

  function undoLevel() {
    dialog = {
      kind: "confirm",
      title: "Undo a level",
      text: "Remove the latest level and its choices?",
      then: () => act({ command: "undo_level" }),
    };
  }

  /// The same, on a computer: one toolbar of groups, each button a short
  /// name (its full name as its title), the deletion apart at the end.
  type Tool = { label: string; title: string; run: () => void };
  function toolGroups(): { name: string; tools: Tool[] }[] {
    const t = (label: string, title: string, run: () => void): Tool => ({ label, title, run });
    const n = sheet.header.pending;
    return [
      {
        name: "Play",
        tools: [
          t("Short rest", "Short rest", () => rest(false)),
          t("Long rest", "Long rest", () => rest(true)),
          logRolls
            ? t("Stop rolls to the session note", "Stop writing rolls into the session note", () => setLogRolls(false))
            : t("Rolls to the session note", "Write rolls into the session note", () => setLogRolls(true)),
        ],
      },
      {
        name: "Character",
        tools: [
          t("Level up…", "Level up…", levelUp),
          ...(n ? [t(`${n} missing ${n === 1 ? "choice" : "choices"}…`, `Make ${n} missing choice(s)…`, nextPending)] : []),
          t("Undo the last level", "Remove the latest level", undoLevel),
          t("Note", "Open the character's note", openNote),
          t("File…", "Edit the character file…", editFile),
        ],
      },
      {
        name: "Export",
        tools: [
          t("Printable page…", "Export as a printable page…", () => exportAs("html")),
          t("Markdown…", "Export as Markdown…", () => exportAs("markdown")),
          t("For dnt…", "Export for dnt (to import elsewhere)…", exportForDnt),
        ],
      },
    ];
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

  // ---- swiping between tabs (Notes is elsewhere: not swiped to) ----
  const swipe = new Swipe<Tab>({
    items: () => SHEET_TABS.filter((t) => t !== "Notes"),
    current: () => tab,
    open: (t) => {
      tab = t;
      tips.done("sheet.swipe");
    },
    body: () => body,
  });

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
    const path = await saveAs(`${sheet.header.name}.${ext}`, format === "html" ? "Web page" : "Markdown", ext);
    if (!path) return;
    try {
      say(await api.exportSheet(format, path));
    } catch (e) {
      say(String(e));
    }
  }

  /// The character with the homebrew and rules they use, to import on
  /// another device or into another campaign (Data › Import).
  async function exportForDnt() {
    const path = await saveAs(`${sheet.header.name}.dnt.zip`, "dnt export", "zip");
    if (!path) return;
    try {
      say(await api.exportCharacter(path));
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

{#snippet view(t: Tab)}
  {#if t === "Overview"}
    <OverviewTab o={sheet.overview} {ctx} onconditions={conditions} />
  {:else if t === "Skills"}
    <SkillsTab o={sheet.overview} {ctx} />
  {:else if t === "Features"}
    <FeaturesTab groups={sheet.features} {ctx} />
  {:else if t === "Spells"}
    <SpellsTab rows={sheet.spells} slots={sheet.overview.trackers.filter((t) => t.id.startsWith("slot:") || t.id === "pact")} {ctx} />
  {:else if t === "Inventory"}
    <InventoryTab inv={sheet.inventory} {ctx} />
  {/if}
{/snippet}

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
      {#if sheet.header.pending}<button class="flag" onclick={nextPending}>{sheet.header.pending} {sheet.header.pending === 1 ? "choice" : "choices"}<span class="wide">&nbsp;to make</span></button>{/if}
      {#if sheet.header.spells_to_choose}<button class="flag" onclick={chooseSpells}>{sheet.header.spells_to_choose} {sheet.header.spells_to_choose === 1 ? "spell" : "spells"}<span class="wide">&nbsp;to choose</span></button>{/if}
    </div>
  {/if}

  {#if desktop.current}
    <!-- A computer: everything the ⋯ menu does, in sight. -->
    <div class="charbar" role="toolbar" aria-label="Character actions">
      {#each toolGroups() as g (g.name)}
        <div class="tgroup" role="group" aria-label={g.name}>
          <span class="glabel" aria-hidden="true">{g.name}</span>
          {#each g.tools as tool (tool.title)}<button class="ghost" title={tool.title} onclick={tool.run}>{tool.label}</button>{/each}
        </div>
      {/each}
      <div class="tgroup end">
        <button class="ghost danger" title="Delete the character…" onclick={remove}>Delete the character…</button>
      </div>
    </div>
  {/if}

  {#if narrow.current || touch.current}
    <!-- A phone or a tablet: every tab as an icon, in one row. -->
    <div class="icontabs" role="tablist">
      {#each SHEET_TABS as t (t)}
        <button role="tab" aria-label={t} aria-selected={tab === t} class:active={tab === t} onclick={() => (tab = t)}><Icon name={TAB_ICONS[t]} size="1.5em" /></button>
      {/each}
    </div>
  {:else}
    <!-- A computer: every tab by its name, in one row. -->
    <div class="tabrow" role="tablist" aria-label="Sheet">
      {#each SHEET_TABS as t (t)}
        <button role="tab" aria-selected={tab === t} class:active={tab === t} onclick={() => (tab = t)}>
          {t}{#if t === "Spells" && sheet.header.spells_to_choose}<span class="badge">{sheet.header.spells_to_choose}</span>{/if}
        </button>
      {/each}
    </div>
  {/if}

  {#if tips.due("sheet.swipe")}
    <Tip id="sheet.swipe" text="swipe sideways for the next tab" />
  {:else if tab === "Overview" || tab === "Skills"}
    <Tip id="sheet.hold" text="hold a number to see how it adds up" />
  {/if}

  <!-- Swiping sideways changes the tab (the tabs above do the same), the
       next one coming in beside it. -->
  <div class="body" role="presentation" bind:this={body} ontouchstart={swipe.start} ontouchmove={swipe.move} ontouchend={swipe.end} ontouchcancel={swipe.end}>
    <div class="slide" class:settling={swipe.settling} style:transform={swipe.drag ? `translateX(${swipe.drag}px)` : null} ontransitionend={swipe.settled}>
      {@render view(tab)}
      {#if swipe.beside}
        <div class="beside" class:ready={swipe.ready} inert data-tab={swipe.beside} style:top="{swipe.top}px" style:height="{swipe.height}px" style:left={swipe.drag < 0 ? "calc(100% + var(--gap))" : "calc(-100% - var(--gap))"}>
          {@render view(swipe.beside)}
        </div>
      {/if}
    </div>
  </div>

  <!-- What just happened: on a computer a toast in the corner that fades,
       elsewhere a line at the foot of the sheet. Read out politely. -->
  <div class="toast" role="status" aria-live="polite">
  {#if message}
   <div class="toastin" out:fade={{ duration: 400 }}>
    {#if rolled}
      <!-- A roll: big, its number first; a tap puts it away. -->
      <button class="status roll" onclick={() => ((message = ""), (rolled = null))}>
        <span class="rtitle"><Icon name="d20" /> {rolled.title}</span>
        <!-- The result and each damage side by side, their numbers big. -->
        <span class="nums" class:many={rolled.extra.length > 0}>
          {#each [rolled.result, ...rolled.extra] as line, i (i)}
            {@const p = numbered(line)}
            <span class="n" class:dmg={i > 0}>
              <b class:word={!p} class:crit={i === 0 && rolled.crit === true} class:fumble={i === 0 && rolled.crit === false}>{p ? p.num : line}</b>
              {#if p?.label}<span class="label">{p.label}</span>{/if}
              <!-- The result's own dice under it, each damage's under that. -->
              {#if i === 0}<span class="dim detail">{rolled.detail}</span>{:else if p?.dice}<span class="dim">{p.dice}</span>{/if}
            </span>
          {/each}
        </span>
      </button>
    {:else}
      <div class="status">{message}</div>
    {/if}
   </div>
  {/if}
  </div>
</div>

{#if dialog?.kind === "warnings"}
  <Modal title="Warnings" onclose={close}>
    <ul class="warnings">
      {#each sheet.header.warnings as w (w)}<li><Icon name="warning" /> {w}</li>{/each}
    </ul>
  </Modal>
{/if}

{#if dialog?.kind === "rules"}
  {#key closings}
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
  {/key}
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
    {#key closings}
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
    {/key}
  </Modal>
{:else if dialog?.kind === "dice"}
  {@const d = dialog}
  {#key closings}
    <DiceRoll
      vars={sheet.overview.roll_vars}
      value={d.text}
      error={d.error}
      onclose={close}
      onroll={async (text) => {
        try {
          const r = await api.roll({ type: "custom", text });
          lastDice = text;
          close();
          sheet = r.sheet;
          say(r.line, r.rolled);
          log(r.line);
        } catch (e) {
          dialog = { kind: "dice", text, error: String(e) };
        }
      }}
    />
  {/key}
{:else if dialog?.kind === "prompt"}
  {@const d = dialog}
  {#key closings}
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
  {/key}
{:else if dialog?.kind === "entry"}
  <EntryModal id={dialog.id} part={dialog.part} onclose={close} />
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
{/if}

<svelte:window onkeydown={(e) => pop && e.key === "Escape" && ((pop = null), e.preventDefault())} />

{#if pop}
  {@const p = pop}
  {@const r = rollOf(p.key)}
  <!-- How a number adds up, beside it: where each part comes from, and its
       roll with or without advantage. -->
  <div class="popback" role="presentation" onclick={() => (pop = null)}></div>
  <div class="pop" role="dialog" aria-label={p.b.title} style:left="{p.x}px" style:top="{p.y}px">
    <div class="phead">
      <span class="ptitle">{p.b.title}</span>
      <b class="pvalue">{signed(p.key, p.b.value)}</b>
      <button class="pclose" aria-label="Close" onclick={() => (pop = null)}>✕</button>
    </div>
    {#each lines(p.key, p.b) as part, i (i)}
      <div class="pline" class:off={!part.applied}>
        <b class="mono">{part.value}</b>
        <span>{part.origin}</span>
        <span class="dim pkind">{part.kind}</span>
      </div>
    {/each}
    {#if r}
      <div class="pbtns">
        <button class="primary" onclick={() => rollWith(r, "normal")}>Roll</button>
        <button class="ghost" onclick={() => rollWith(r, "advantage")}>Advantage</button>
        <button class="ghost" onclick={() => rollWith(r, "disadvantage")}>Disadvantage</button>
      </div>
    {/if}
  </div>
{/if}

{#if dialog?.kind === "conditions"}
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
    {#key closings}<Ask question={d.step.question} onanswer={levelAnswer} onconfirm={levelApply} confirm="Apply the level" />{/key}
  </Modal>
{:else if dialog?.kind === "pending"}
  {@const d = dialog}
  <Modal title="{d.p.from}: {d.p.key}" onclose={close} footer="{d.p.left} left">
    {#key closings}<Ask question={d.p.question} onanswer={(a) => pendingAnswer(d.p, a)} />{/key}
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
  /* A computer: the tabs by name, one row, the chosen one raised. */
  .tabrow {
    display: inline-flex;
    align-self: flex-start;
    flex-wrap: wrap;
    gap: 0.25rem;
    margin: 0.9rem 0 0.6rem;
    padding: 0.25rem;
    background: var(--card);
    border: 1px solid var(--edge);
    border-radius: var(--radius);
    flex-shrink: 0;
  }
  .tabrow button {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    height: 2.3rem;
    padding: 0 1rem;
    border: 0;
    border-radius: 0.5rem;
    background: none;
    color: var(--gray);
    font: inherit;
    font-weight: 500;
    white-space: nowrap;
    cursor: pointer;
  }
  .tabrow button:hover {
    background: var(--raised);
    color: var(--fg);
  }
  .tabrow button.active {
    background: var(--dark);
    color: var(--fg);
  }
  .badge {
    background: var(--cyan);
    color: var(--black);
    border-radius: 0.6rem;
    padding: 0 0.4rem;
    font-size: 0.75em;
    font-weight: 700;
  }
  @media (pointer: fine) and (min-width: 761px) {
    h1 {
      font-size: 1.6em;
      color: var(--fg);
    }
    h1 .acc {
      font-size: 0.7em;
      vertical-align: 0.15em;
    }
    .id {
      color: var(--gray);
    }
    .head {
      align-items: center;
    }
    .flag {
      border-radius: 0.5rem;
      padding: 0.35rem 0.8rem;
      white-space: nowrap;
    }
    /* What happened (a roll, a change): a toast in the bottom right corner
       that fades; the sheet keeps room under its last row to scroll it
       clear of the toast. */
    .toast {
      position: fixed;
      right: 1.5rem;
      bottom: 1.5rem;
      z-index: 30;
      width: min(24rem, calc(100vw - 3rem));
    }
    .status {
      position: static;
      margin: 0;
      background: var(--card);
      border-color: var(--accent);
      box-shadow: 0 0.8rem 2rem rgb(0 0 0 / 0.5);
      animation: toastin 0.2s ease-out;
    }
    .sheet > .body {
      padding-bottom: 8rem;
    }
  }
  @keyframes toastin {
    from {
      opacity: 0;
      transform: translateY(0.5rem);
    }
  }
  .toast,
  .toastin {
    display: contents;
  }
  @media (pointer: fine) and (min-width: 761px) {
    .toast,
    .toastin {
      display: block;
    }
  }
  /* The sheet's actions on a computer: one toolbar, groups apart, a group
     wrapping as a whole. */
  .charbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.4rem 1.1rem;
    margin-top: 0.7rem;
    flex-shrink: 0;
  }
  .tgroup {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    white-space: nowrap;
  }
  .tgroup + .tgroup {
    padding-left: 1.1rem;
    border-left: 1px solid var(--edge);
  }
  .tgroup.end {
    margin-left: auto;
  }
  .glabel {
    font-size: 0.78em;
    font-weight: 600;
    color: var(--dim);
    margin-right: 0.2rem;
  }
  .charbar .ghost {
    min-height: 1.95rem;
    padding: 0 0.6rem;
    font-size: 0.85em;
  }
  .charbar .danger {
    color: var(--red);
  }
  .popback {
    position: fixed;
    inset: 0;
    z-index: 40;
  }
  .pop {
    position: fixed;
    z-index: 41;
    width: 26rem;
    max-height: calc(100vh - 2rem);
    overflow-y: auto;
    padding: 0.9rem 1.1rem 1rem;
    background: var(--card);
    border: 1px solid var(--accent);
    border-radius: var(--radius);
    box-shadow: 0 1rem 2.5rem rgb(0 0 0 / 0.5);
  }
  .phead {
    display: flex;
    align-items: baseline;
    gap: 0.6rem;
    margin-bottom: 0.6rem;
  }
  .ptitle {
    font-weight: 600;
  }
  .pvalue {
    font-family: var(--mono);
    font-size: 1.3em;
    color: var(--accent);
  }
  .pclose {
    margin-left: auto;
    background: none;
    border: 0;
    color: var(--gray);
    cursor: pointer;
    font: inherit;
  }
  .pline {
    display: grid;
    grid-template-columns: 3rem minmax(0, 1fr) auto;
    gap: 0.6rem;
    align-items: baseline;
    padding: 0.3rem 0;
    border-top: 1px solid var(--edge);
  }
  .pline b {
    text-align: right;
    color: var(--fg);
  }
  .pline.off {
    opacity: 0.55;
  }
  .pkind {
    font-size: 0.8em;
  }
  .pbtns {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
    margin-top: 0.8rem;
  }
  .body {
    position: relative;
    flex: 1;
    min-height: 0;
    overflow-x: hidden;
    overflow-y: auto;
    /* Sideways drags are the tabs' (see touchMove); scrolling stays native. */
    touch-action: pan-y;
    padding: 0.4em 1ch 1.5em 0;
  }
  .slide {
    position: relative;
  }
  .slide.settling {
    transition: transform 0.2s ease-out;
  }
  /* The tab a swipe goes to, its top in sight. */
  .beside {
    position: absolute;
    width: 100%;
    overflow: hidden;
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
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.2em;
    width: 100%;
    padding: 0.8em 2ch 1em;
    border: 2px solid var(--accent);
    font: inherit;
    color: var(--fg);
    cursor: pointer;
    box-shadow: 0 -0.5em 1.5em rgb(0 0 0 / 0.5);
  }
  .status.roll .rtitle {
    display: flex;
    align-items: center;
    gap: 1ch;
    font-weight: 700;
  }
  .nums {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 0.4em 4ch;
  }
  .n {
    display: flex;
    flex-direction: column;
    align-items: center;
  }
  .status b {
    color: var(--accent);
    font-size: 3.2em;
    line-height: 1.1;
  }
  .nums.many b {
    font-size: 2.6em;
  }
  .n.dmg b {
    color: var(--red);
  }
  /* A result in words ("critical hit", "fails"): big, but leaving room. */
  .status b.word,
  .nums.many b.word {
    font-size: 1.8em;
    line-height: 1.6;
  }
  .n .label {
    font-weight: 700;
  }
  .status .detail {
    text-align: center;
  }
  .icontabs {
    display: flex;
    margin: 0.4em 0 0.2em;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }
  .icontabs button {
    flex: 1;
    min-height: 48px;
    background: none;
    border: 0;
    border-bottom: 3px solid transparent;
    color: var(--dim);
    cursor: pointer;
  }
  .icontabs button.active {
    color: var(--accent);
    border-bottom-color: var(--accent);
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
  /* A phone's or a tablet's app bar has the name, the level and the
     sheet's actions. */
  :global(.touch) .head {
    display: none;
  }
  @media (max-width: 760px) {
    .icon {
      min-height: 44px;
      min-width: 44px;
    }
    .flag .wide {
      display: none;
    }
    .flags {
      margin: 0.4em 0 0;
    }
    .flag {
      min-height: 36px;
      padding: 0 1.5ch;
      border-radius: 18px;
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
