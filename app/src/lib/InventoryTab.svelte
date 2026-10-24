<!-- What the character carries, worn and held, and coins. Tap a line for
     what you can do with it. -->
<script lang="ts">
  import type { Inventory, Line } from "./api";
  import Pane from "./Pane.svelte";
  import type { SheetCtx } from "./sheetctx";

  let { inv, ctx }: { inv: Inventory; ctx: SheetCtx } = $props();

  function add() {
    ctx.pick({ type: "items" }, ([id]) => ctx.act({ command: "add_item", id, inside: null }));
  }

  function custom() {
    ctx.prompt("A custom item", (name) => name.trim() && ctx.act({ command: "add_custom_item", name: name.trim(), inside: null }));
  }

  function coins() {
    ctx.prompt("Coins", (change) => change.trim() && ctx.act({ command: "change_coins", change }), {
      hint: "+15gp, -3sp, or a number to set gold",
    });
  }

  function menu(l: Line) {
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
    actions.push(["One more", () => ctx.act({ command: "change_quantity", line, delta: 1 })]);
    if (l.quantity > 1) actions.push(["One fewer", () => ctx.act({ command: "change_quantity", line, delta: -1 })]);
    actions.push(["Remove", () => ctx.act({ command: "remove_line", line })]);
    ctx.menu(l.name, actions);
  }

  const mark = (l: Line) => (l.attuned ? "★" : l.worn ? "◉" : l.held ? "◈" : l.needs_attunement ? "☆" : "");
</script>

<div class="inventory">
  <Pane title="Inventory" class="items" fold="sheet.inventory" summary="{inv.lines.length} item{inv.lines.length === 1 ? '' : 's'} · {inv.carried.toFixed(1)} lb">
    <div class="buttons">
      <button class="primary" onclick={add}>+ Add item</button>
      <button class="ghost" onclick={custom}>+ Custom</button>
    </div>
    {#if !inv.lines.length}<p class="dim">Empty.</p>{/if}
    {#each inv.lines as l (l.index)}
      <button class="line" onclick={() => menu(l)} style:padding-left="{l.depth * 2 + 0.5}ch">
        <span class="mark" class:dim={!l.worn && !l.held && !l.attuned}>{mark(l)}</span>
        <span class="name" class:box={l.fill !== null}>{l.name}</span>
        <span class="kind dim">{l.kind}{#if l.fill} <span class:over={l.over} class="fill">{l.fill}</span>{/if}{l.weightless ? " weightless" : ""}</span>
        <span class="qty">{l.quantity > 1 ? `×${l.quantity}` : ""}</span>
        <span class="lb dim">{l.weight > 0 ? `${l.weight.toFixed(1)} lb` : ""}</span>
      </button>
    {/each}
    <div class="totals">
      <div><span class="dim">Carried</span> <b class:over={inv.carried > inv.capacity}>{inv.carried.toFixed(1)}</b> <span class="muted">/ {inv.capacity} lb</span></div>
      <div><span class="dim">Attuned</span> <b class:over={inv.attuned > 3}>{inv.attuned}</b> <span class="muted">/ 3</span></div>
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
  }
  .line {
    width: 100%;
    display: grid;
    grid-template-columns: 2ch minmax(0, 1fr) auto 4ch 8ch;
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
  .name.box {
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
  .totals {
    border-top: 1px solid var(--border);
    margin-top: 0.75em;
    padding-top: 0.5em;
    display: flex;
    flex-direction: column;
    gap: 0.2em;
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
  @media (max-width: 760px) {
    .line {
      grid-template-columns: 2ch minmax(0, 1fr) auto;
      min-height: 48px;
      align-items: center;
      border-top: 1px solid rgb(65 72 104 / 0.3);
    }
    .kind {
      grid-column: 2;
      grid-row: 2;
      font-size: 0.85em;
    }
    .lb {
      display: none;
    }
    .buttons button {
      flex: 1;
    }
    .coins {
      min-height: 44px;
      align-items: center;
    }
  }
</style>
