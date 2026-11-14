<!-- What the character carries, worn and held, and coins. Tap a line for
     what you can do with it. -->
<script lang="ts">
  import * as api from "./api";
  import type { Inventory, Line } from "./api";
  import BrewEditor from "./BrewEditor.svelte";
  import Pane from "./Pane.svelte";
  import type { SheetCtx } from "./sheetctx";
  import { desktop } from "./screen";

  let { inv, ctx }: { inv: Inventory; ctx: SheetCtx } = $props();

  /// `into`: the container line to put it in (null: carried loose).
  function add(into: number | null = null) {
    ctx.pick({ type: "items" }, ([id]) => ctx.act({ command: "add_item", id, inside: null, into }));
  }

  function custom(into: number | null = null) {
    ctx.prompt("A custom item", (name) => name.trim() && ctx.act({ command: "add_custom_item", name: name.trim(), inside: null, into }));
  }

  /// A new campaign item from a template, added here, then edited.
  let editing: api.Editing | null = $state(null);
  async function homebrew() {
    try {
      const templates = await api.brewItemTemplates();
      ctx.menu(
        "A new homebrew item",
        templates.map(([i, label]) => [
          label,
          () =>
            ctx.prompt(`New ${label.toLowerCase()}`, async (name) => {
              if (!name.trim()) return;
              try {
                editing = await api.brewCreateItem(i, name.trim());
                await ctx.refresh();
              } catch (e) {
                ctx.say(String(e));
              }
            }, { hint: "Its name; you fill in the rest next (it goes into the campaign's homebrew)" }),
        ]),
      );
    } catch (e) {
      ctx.say(String(e));
    }
  }

  // ---- folding containers ----
  /// Folded container lines, by index and name (so a shifted index does
  /// not fold another line).
  let folded = $state(new Set<string>());
  const foldKey = (l: Line) => `${l.index}:${l.name}`;
  const hasContents = (i: number) => inv.lines[i + 1]?.depth > inv.lines[i].depth;
  /// The lines not hidden in a folded container.
  const shown = $derived.by(() => {
    const out: Line[] = [];
    let hideDeeper = Infinity;
    for (const l of inv.lines) {
      if (l.depth > hideDeeper) continue;
      hideDeeper = folded.has(foldKey(l)) ? l.depth : Infinity;
      out.push(l);
    }
    return out;
  });
  function toggleFold(l: Line) {
    const k = foldKey(l);
    const next = new Set(folded);
    if (!next.delete(k)) next.add(k);
    folded = next;
  }

  function coins() {
    ctx.prompt("Coins", (change) => change.trim() && ctx.act({ command: "change_coins", change }), {
      hint: "+15gp, -3sp, or a number to set gold",
    });
  }

  function menu(l: Line) {
    ctx.menu(l.name, itemActions(l));
  }

  /// The item chosen on a computer: its actions in sight under it.
  let chosen = $state<number | null>(null);
  /// A tap opens the menu; on a computer a click shows the actions instead.
  const open = (l: Line) => (desktop.current ? (chosen = l.index) : menu(l));

  /// Where an equipped item is, for its pill: worn in a slot, or held.
  function equipped(l: Line): string {
    if (l.held) return inv.main_hand?.[0] === l.index ? "Held · main hand" : inv.off_hand?.[0] === l.index ? "Held · off hand" : "Held";
    if (l.worn) {
      const slot = inv.worn.find((w) => w.items.some(([i]) => i === l.index));
      return slot ? `Worn · ${slot.label.toLowerCase()}` : "Worn";
    }
    return "Equip";
  }

  function itemActions(l: Line): [string, () => void][] {
    const line = l.index;
    const actions: [string, () => void][] = [];
    if (l.entity !== null) actions.push(["Read it", () => ctx.show(l.entity!)]);
    actions.push([l.worn ? "Take it off" : l.held ? "Put it away" : "Equip (wear or hold)", () => ctx.act({ command: "equip", line })]);
    actions.push(["Wear it in…", () => ctx.pick({ type: "wear", line }, ([slot]) => ctx.act({ command: "wear", line, slot }))]);
    actions.push(["Hold in the main hand", () => ctx.act({ command: "hold", line, hand: "main" })]);
    actions.push(["Hold in the off hand", () => ctx.act({ command: "hold", line, hand: "off" })]);
    if (l.attuned || l.needs_attunement) actions.push([l.attuned ? "End attunement" : "Attune", () => ctx.act({ command: "toggle_attuned", line })]);
    actions.push(["Put it in…", () => ctx.pick({ type: "containers", line }, ([v]) => ctx.act({ command: "put_in", line, into: v === "loose" ? null : Number(v) }))]);
    if (l.pack) actions.push(["Unpack", () => ctx.act({ command: "unpack", line })]);
    if (l.fill !== null) {
      actions.push(["Add an item into it…", () => add(line)]);
      actions.push(["Add a custom item into it…", () => custom(line)]);
    }
    actions.push([
      l.fill !== null ? "Container settings…" : "Make it a container…",
      () =>
        ctx.prompt(`${l.name}: container`, (settings) => ctx.act({ command: "set_container", line, settings }), {
          value: l.container,
          hint: "30 (pounds it holds) · no limit · weightless (a Bag of Holding) · off · empty: as the item says",
        }),
    ]);
    if (l.monk_weapon !== null) {
      actions.push([l.monk_weapon ? "Not a monk weapon" : "Count as a monk weapon", () => ctx.act({ command: "toggle_monk_weapon", line })]);
    }
    actions.push(["One more", () => ctx.act({ command: "change_quantity", line, delta: 1 })]);
    if (l.quantity > 1) actions.push(["One fewer", () => ctx.act({ command: "change_quantity", line, delta: -1 })]);
    actions.push(["Remove", () => ctx.act({ command: "remove_line", line })]);
    return actions;
  }

  const mark = (l: Line) => (l.attuned ? "★" : l.needs_attunement ? "☆" : "");
</script>

<div class="inventory">
  <Pane title="Inventory" class="items" fold="sheet.inventory" summary="{inv.lines.length} item{inv.lines.length === 1 ? '' : 's'} · {inv.carried.toFixed(1)} lb">
    <div class="buttons">
      <button class="primary" onclick={() => add()}>+ Add item</button>
      <button class="ghost" onclick={() => custom()}>+ Custom</button>
      <button class="ghost" onclick={homebrew}>+ Homebrew</button>
    </div>
    {#if !inv.lines.length}<p class="dim">Empty.</p>{/if}
    {#each shown as l (l.index)}
      {@const i = inv.lines.indexOf(l)}
      <div class="lrow" style:padding-left="{l.depth * 2}ch">
        <!-- A container's fold arrow; equip it (wear or hold) or not, right here. -->
        {#if hasContents(i)}
          <button class="foldbtn" aria-label={folded.has(foldKey(l)) ? "Unfold" : "Fold"} onclick={() => toggleFold(l)}>{folded.has(foldKey(l)) ? "▸" : "▾"}</button>
        {/if}
        {#if l.equippable}
          <button
            class="equip"
            role="checkbox"
            aria-checked={l.worn || l.held}
            aria-label="Equipped: {l.name}"
            class:on={l.worn || l.held}
            onclick={() => ctx.act({ command: "equip", line: l.index })}>{#if desktop.current}{equipped(l)}{:else}{l.worn ? "◉" : l.held ? "◈" : ""}{/if}</button
          >
        {:else if !hasContents(i)}
          <span class="equip none"></span>
        {/if}
        <button class="line" class:chosen={chosen === l.index} aria-expanded={desktop.current ? chosen === l.index : undefined} onclick={() => open(l)} oncontextmenu={(e) => (e.preventDefault(), menu(l))}>
          <span class="name" class:container={l.fill !== null}>{l.name}{#if mark(l)} <span class="mark" class:dim={!l.attuned}>{mark(l)}</span>{/if}</span>
          <span class="kind dim">{l.kind}{#if l.fill} <span class:over={l.over} class="fill">{l.fill}</span>{/if}{l.weightless ? " weightless" : ""}</span>
          <span class="qty">{l.quantity > 1 ? `×${l.quantity}` : ""}</span>
          <span class="lb dim">{l.weight > 0 ? `${l.weight.toFixed(1)} lb` : ""}</span>
        </button>
        {#if desktop.current}<button class="more" aria-label="More" title="The menu" onclick={() => menu(l)}>⋯</button>{/if}
      </div>
      {#if desktop.current && chosen === l.index}
        <!-- Everything the item's menu does, in sight. -->
        <div class="acts" role="dialog" aria-label={l.name} style:padding-left="calc({l.depth * 2}ch + 8.2rem)">
          {#each itemActions(l) as [label, run] (label)}<button class="ghost" class:bad={label === "Remove"} onclick={run}>{label}</button>{/each}
        </div>
      {/if}
    {/each}
    <div class="totals">
      <span class="box"><span><b class:over={inv.carried > inv.capacity}>{inv.carried.toFixed(1)}</b> <span class="muted">/ {inv.capacity} lb</span></span><span class="dim">carried</span></span>
      <span class="box"><span><b class:over={inv.attuned > 3}>{inv.attuned}</b> <span class="muted">/ 3</span></span><span class="dim">attuned</span></span>
    </div>
    <div class="money">
      <button class="coins" onclick={coins}>
        <span class="dim">Coins</span>
        {#each inv.coins as [c, n] (c)}<span><b class="c-{c}">{n}</b> <span class="dim">{c}</span></span>{/each}
      </button>
    </div>
    <p class="legend dim">◉ worn · ◈ in hand · ★ attuned · ☆ needs attunement</p>
  </Pane>

  <Pane title="Equipment" class="equipment" fold="sheet.equipment">
    <div class="eq-title">Worn</div>
    {#each inv.worn as w (w.slot)}
      <div class="slot"><span class="dim">{w.label}</span><span>{w.items.length ? w.items.map(([, n]) => n).join(", ") : "—"}</span></div>
    {/each}
    <div class="eq-title">In hand</div>
    <div class="slot"><span class="dim">Main hand</span><span>{inv.main_hand?.[1] ?? "—"}</span></div>
    <div class="slot"><span class="dim">Off hand</span><span>{inv.off_hand?.[1] ?? "—"}</span></div>
  </Pane>
</div>

{#if editing}
  <BrewEditor
    bind:editing
    onchanged={() => ctx.refresh()}
    onclose={async () => {
      editing = null;
      await api.brewClose().catch(() => {});
      await ctx.refresh();
    }}
  />
{/if}

<style>
  .inventory {
    display: grid;
    grid-template-columns: minmax(0, 1.4fr) minmax(30ch, 1fr);
    gap: 1.4em 2ch;
    align-items: start;
    padding-top: 0.6em;
  }
  :where(button:not(.ghost):not(.primary)) {
    background: none;
    border: 0;
    text-align: left;
    cursor: pointer;
  }
  .buttons {
    display: flex;
    gap: 1ch;
    margin-bottom: 0.75em;
  }
  .buttons button {
    text-align: center;
    white-space: nowrap;
  }
  .lrow {
    display: flex;
    align-items: stretch;
  }
  /* In the checkbox's place, so rows keep their names in line. */
  .foldbtn {
    flex: 0 0 auto;
    width: 2.4ch;
    align-self: center;
    margin-right: 1ch;
    color: var(--accent);
    text-align: center !important;
    padding: 0;
  }
  .line {
    flex: 1;
    min-width: 0;
    width: 100%;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto 4ch 8ch;
    gap: 1ch;
    padding: 0.1em 0.5ch;
  }
  .line:hover {
    background: var(--selected);
  }
  .mark {
    color: var(--accent);
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .name.container {
    font-weight: 700;
  }
  .kind {
    white-space: nowrap;
  }
  .fill {
    color: var(--accent);
  }
  .over {
    color: var(--red) !important;
  }
  .qty,
  .lb {
    text-align: right;
  }
  .equip {
    flex: 0 0 auto;
    width: 2.4ch;
    height: 2.4ch;
    align-self: center;
    display: inline-flex !important;
    align-items: center;
    justify-content: center;
    margin-right: 1ch;
    padding: 0 !important;
    border: 1px solid var(--border) !important;
    border-radius: 3px;
    color: var(--accent);
  }
  .equip[aria-checked="true"] {
    border-color: var(--accent) !important;
  }
  .equip.none {
    border: 0 !important;
  }
  .totals {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 1ch;
    margin-top: 0.75em;
  }
  .box {
    display: flex;
    flex-direction: column;
    align-items: center;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 0.4em 0.5ch;
  }
  .box b {
    color: var(--accent);
    font-size: 1.2em;
  }
  .box > .dim {
    font-size: 0.8em;
  }
  .money {
    margin-top: 0.6em;
  }
  .coins {
    display: flex;
    gap: 2ch;
    flex-wrap: wrap;
    padding: 0;
  }
  .c-gp {
    color: var(--yellow);
  }
  .c-cp {
    color: var(--red);
  }
  .legend {
    margin: 0.75em 0 0;
    font-size: 0.9em;
  }
  .eq-title {
    color: var(--accent);
    font-weight: 700;
    margin: 0.5em 0 0.2em;
  }
  .eq-title:first-child {
    margin-top: 0;
  }
  .slot {
    display: grid;
    grid-template-columns: 11ch 1fr;
    gap: 1ch;
  }
  @media (max-width: 1000px) {
    .inventory {
      grid-template-columns: minmax(0, 1fr);
    }
  }
  /* A computer: the equip box says what it does (Equip, Worn · slot,
     Held · hand), and a click shows an item's actions under it (⋯ and a
     right click still open its menu). Arrows and boxes share one width so
     names stay in line. */
  @media (pointer: fine) and (min-width: 761px) {
    .equip,
    .foldbtn {
      width: 7.6rem;
      margin-right: 0.6rem;
    }
    .equip {
      height: 1.9rem;
      justify-content: center;
      border-radius: 0.95rem !important;
      font: inherit;
      font-size: 0.82em;
      font-weight: 500;
      color: var(--gray);
      white-space: nowrap;
    }
    .equip.on {
      background: rgb(158 206 106 / 0.14);
      border-color: var(--green) !important;
      color: var(--green);
    }
    .equip.none {
      visibility: hidden;
    }
    .foldbtn {
      text-align: right;
      padding-right: 0.8rem;
    }
    .line.chosen .name {
      color: var(--accent);
    }
    .more {
      flex: 0 0 auto;
      background: none;
      border: 0;
      color: var(--gray);
      cursor: pointer;
      font: inherit;
      padding: 0 0.5rem;
    }
    .acts {
      display: flex;
      flex-wrap: wrap;
      gap: 0.4rem;
      margin: 0.2rem 0 0.6rem;
    }
    .acts .ghost {
      min-height: 2rem;
      padding: 0 0.75rem;
      font-size: 0.9em;
    }
    .acts .bad {
      color: var(--red);
    }
  }
  @media (max-width: 760px) {
    .line {
      grid-template-columns: minmax(0, 1fr) auto;
      min-height: 48px;
      align-items: center;
      border-top: 1px solid rgb(65 72 104 / 0.3);
    }
    .kind {
      grid-column: 1;
      grid-row: 2;
      font-size: 0.85em;
    }
    .lb {
      display: none;
    }
    /* Shared by their labels' widths: equal thirds wrap "+ Homebrew". */
    .buttons button {
      flex: 1 1 auto;
      padding-inline: 1ch;
    }
    .foldbtn {
      width: 32px;
      font-size: 1.3em;
      /* The equip box's 1ch, at this font size. */
      margin-right: calc(1ch / 1.3);
    }
    .equip {
      width: 32px;
      height: 32px;
    }
    .coins {
      min-height: 44px;
      align-items: center;
    }
  }
</style>
