<!-- The overview: hit points and the numbers, conditions, attacks and
     what is tracked (abilities and skills are on their own tab). Tap a
     number to roll it or see how it is made up. -->
<script lang="ts">
  import { killsOutright, type Overview, type Tracker } from "./api";
  import Pane from "./Pane.svelte";
  import Icon from "./Icon.svelte";
  import { pool, type SheetCtx } from "./sheetctx";

  let { o, ctx, onconditions }: { o: Overview; ctx: SheetCtx; onconditions: () => void } = $props();

  const pct = $derived(o.hp.max ? Math.round((o.hp.current / o.hp.max) * 100) : 0);
  const hpColor = $derived(pct <= 25 ? "var(--red)" : pct <= 50 ? "var(--yellow)" : "var(--green)");
  /// Tile labels on a phone, where five fit a row.
  const SHORT: Record<string, string> = {
    "Armor Class": "AC",
    Initiative: "Init.",
    Proficiency: "Prof.",
    "Passive Perception": "Passive",
  };

  function amount(title: string, command: string) {
    ctx.prompt(
      title,
      (v) => {
        const n = parseInt(v, 10);
        if (!Number.isNaN(n)) ctx.act({ command, amount: n });
      },
      { number: true },
    );
  }

  /// Damage; when it kills outright, ask first (dead, or left dying at 0).
  function damage() {
    ctx.prompt(
      "Damage",
      async (v) => {
        const n = parseInt(v, 10);
        if (Number.isNaN(n)) return;
        if (await killsOutright(n).catch(() => false)) {
          ctx.menu(`${n} damage kills outright`, [
            ["Dead", async () => (await ctx.act({ command: "damage", amount: n })) && ctx.act({ command: "set_dead", dead: true })],
            ["Leave at 0 hit points, dying", () => ctx.act({ command: "damage", amount: n })],
          ]);
          return;
        }
        ctx.act({ command: "damage", amount: n });
      },
      { number: true },
    );
  }

  /// Your own tracker: a magic item's charges, a boon, …
  function addTracker() {
    ctx.prompt(
      "New tracker",
      (name) => {
        if (!name.trim()) return;
        ctx.prompt(
          `${name}: how many uses?`,
          (v) => {
            const max = parseInt(v, 10);
            if (Number.isNaN(max) || max < 1) return;
            const add = (recharge: string) => ctx.act({ command: "add_tracker", name, max, recharge });
            ctx.menu(`${name}: the uses come back`, [
              ["After a long rest", () => add("long_rest")],
              ["After a short or long rest", () => add("short_rest")],
              ["At dawn", () => add("dawn")],
              ["Never by themselves", () => add("manual")],
            ]);
          },
          { number: true, value: "1" },
        );
      },
      { hint: "e.g. Ring of Spell Storing, a boon, a potion's charges" },
    );
  }

  /// Spend a hit die: rolled, with Constitution, and healed.
  /// The hit dice show with the hit points; the other trackers below.
  const hitDice = $derived(o.trackers.filter((t) => t.id.startsWith("hit_die:")));
  // Spell slots are on the Spells tab.
  const tracked = $derived(o.trackers.filter((t) => !t.id.startsWith("hit_die:") && !t.id.startsWith("slot:") && t.id !== "pact"));
  const rollHitDie = (id: string) => ctx.act({ command: "roll_hit_die", faces: Number(id.slice("hit_die:".length)) });

  /// What can be done with a tracker: your own can be removed; a
  /// feature's is kept by its rules, which can be changed or removed.
  function trackerMenu(t: Tracker) {
    const explain: [string, () => void] = ["How the uses are made up", () => ctx.explain(t.stat)];
    const key = t.id.startsWith("resource:") ? t.id.slice("resource:".length) : "";
    if (t.id.startsWith("hit_die:")) {
      return ctx.menu(t.name, [["Roll one and heal", () => rollHitDie(t.id)], ["How the dice are made up", () => ctx.explain(t.stat)]]);
    }
    if (key.startsWith("custom_")) {
      const remove = () => ctx.confirm(`Remove ${t.name}`, "The tracker and its count are gone.", () => ctx.act({ command: "remove_tracker", key }), "Remove");
      return ctx.menu(t.name, [["Remove the tracker…", remove], explain]);
    }
    if (t.feature !== null) {
      const feature = t.feature;
      return ctx.menu(t.name, [
        explain,
        ["Show the feature", () => ctx.show(feature)],
        ["Change or remove its rules (and this tracker)", () => ctx.rules(feature)],
      ]);
    }
    ctx.explain(t.stat);
  }

  /// Holding a number (or a right click) shows how it is made up; a tap
  /// rolls it.
  const hold = (key: string) => (e: Event) => {
    e.preventDefault();
    ctx.explain(key);
  };

  function attackMenu(slug: string, name: string, line: number | null) {
    const actions: [string, () => void][] = [["Roll the attack", () => ctx.roll({ type: "attack", slug })]];
    if (line !== null) {
      actions.push(["Hold in the main hand", () => ctx.act({ command: "hold", line, hand: "main" })]);
      actions.push(["Hold in the off hand", () => ctx.act({ command: "hold", line, hand: "off" })]);
      actions.push(["Hold with both hands", () => ctx.act({ command: "hold", line, hand: "both" })]);
    }
    actions.push(["How the bonus is made up", () => ctx.explain(`attack.${slug}.bonus`)]);
    actions.push(["How the damage is made up", () => ctx.explain(`attack.${slug}.damage`)]);
    ctx.menu(name, actions);
  }

</script>

<div class="overview">
 <div class="mid">
  <!-- Hit points first: what changes most in play. -->
  <Pane title="Hit points" class="hp" fold="sheet.hp" summary="{o.hp.current}/{o.hp.max}{o.hp.temp ? ` +${o.hp.temp}` : ''}">
    <div class="hprow">
      <div class="bar"><div class="fill" style:width="{pct}%" style:background={hpColor}></div></div>
      <div class="hpnum"><span class="hearticon" class:down={o.hp.current === 0}><Icon name="heart" /></span> <b>{o.hp.current}</b><span class="muted">/{o.hp.max}</span>{#if o.hp.temp}<span class="temp">+{o.hp.temp} temp</span>{/if}</div>
    </div>
    <!-- The hit dice, right under: tap the bar to roll one and heal. -->
    {#each hitDice as t (t.id)}
      <!-- As long as the hit points' bar, its count at the end; the die and
           −/+ (spend or restore one without rolling) under it. -->
      <div class="tracker hitdie">
        <button class="bar hdbar" aria-label="Roll d{t.id.slice(8)} + Con, heal" disabled={t.left <= 0} onclick={() => rollHitDie(t.id)}>
          <div class="fill" style:width="{t.max ? Math.round((t.left / t.max) * 100) : 0}%"></div>
        </button>
        <span class="count">{t.left}/{t.max}</span>
        <button class="tname" onclick={() => trackerMenu(t)}>Hit dice <b>d{t.id.slice(8)}</b></button>
        <span class="steps">
          <button class="step" aria-label="Use" onclick={() => ctx.act({ command: "spend", pool: pool(t.id), amount: 1 })}>−</button>
          <button class="step" aria-label="Restore" onclick={() => ctx.act({ command: "spend", pool: pool(t.id), amount: -1 })}>+</button>
        </span>
      </div>
    {/each}
    {#if o.hp.dead}
      <div class="death dead">
        <span class="bad"><Icon name="dead" size="1.4em" /> Dead</span>
        <button class="ghost" onclick={() => ctx.act({ command: "set_dead", dead: false })}>Not dead</button>
      </div>
    {:else if o.hp.current === 0}
      {@const s = o.hp.death_successes}
      {@const f = o.hp.death_failures}
      <div class="death">
        <span class="dim">Death saves</span>
        <span class="ok">{"●".repeat(s)}<span class="dim">{"○".repeat(Math.max(0, 3 - s))}</span></span>
        <span class="bad">{"●".repeat(f)}<span class="dim">{"○".repeat(Math.max(0, 3 - f))}</span></span>
        {#if s >= 3}<span class="dim">stable</span>{:else}<button class="primary" onclick={() => ctx.act({ command: "death_save" })}><Icon name="d20" /> Roll</button>{/if}
      </div>
    {/if}
    <div class="buttons">
      <button class="ghost" onclick={damage}>Damage</button>
      <button class="ghost" onclick={() => amount("Heal", "heal")}>Heal</button>
      <button class="ghost" onclick={() => amount("Temporary hit points", "set_temp_hp")}>Temp</button>
    </div>
  </Pane>

  <div class="tiles">
    {#each o.tiles as t (t.key)}
      <button
        class="tile"
        onclick={() => (t.key === "initiative" ? ctx.roll({ type: "initiative" }) : ctx.explain(t.key))}
        oncontextmenu={hold(t.key)}
      >
        <b>{t.value}</b><span class="dim long">{t.label}</span><span class="dim short">{SHORT[t.label] ?? t.label}</span>
      </button>
    {/each}
  </div>

  <div class="state">
    <button class="chip" class:on={o.conditions.length || o.exhaustion} onclick={onconditions}>
      {o.conditions.length || o.exhaustion ? [...o.conditions.map((c) => c.name), ...(o.exhaustion ? [`Exhaustion ${o.exhaustion}`] : [])].join(" · ") : "No conditions"}
    </button>
    {#if o.concentration}
      <button class="chip conc" onclick={() => ctx.act({ command: "end_concentration" })}>◎ {o.concentration} ✕</button>
    {/if}
    <button class="chip mode" class:on={o.next_roll !== "normal"} onclick={() => ctx.cycle()}>
      next roll: {o.next_roll}
    </button>
  </div>
  {#each o.conditions as c (c.name)}<p class="dim small">{c.name}: {c.summary}</p>{/each}
  {#if o.exhaustion}<p class="dim small">Exhaustion: {o.exhaustion_summary}</p>{/if}

  {#if o.rolls.length}
    <!-- The latest rolls, folded away: the status line shows each roll. -->
    <details class="rolled">
      <summary>
        <Icon name="d20" />
        <span class="dim">Rolled</span>
        <span class="last">{o.rolls[0].title} <b class:crit={o.rolls[0].crit === true} class:fumble={o.rolls[0].crit === false}>{o.rolls[0].result}</b></span>
        <span class="dim count">{o.rolls.length}</span>
      </summary>
      {#each o.rolls.slice(0, 10) as r, i (i)}
        <div class="roll" class:old={i > 0}>
          <span class="rtitle">{r.title}</span>
          <b class:crit={r.crit === true} class:fumble={r.crit === false}>{r.result}</b>
          <span class="dim">{r.detail}</span>
          {#each r.extra as e, j (j)}<div class="extra">{e}</div>{/each}
        </div>
      {/each}
    </details>
  {/if}

  <Pane title="Attacks" class="attacks" fold="sheet.attacks" summary={o.held.length ? o.held.join(" · ") : `${o.attacks.length} attack${o.attacks.length === 1 ? "" : "s"}`}>
    <div class="held"><span class="dim">In hand</span> {o.held.length ? o.held.join(" · ") : "nothing"}</div>
    {#each o.attacks as a (a.slug)}
      <div class="attack">
        <button class="arow" onclick={() => ctx.roll({ type: "attack", slug: a.slug })}>
          <span class="hand">{a.hand}</span>
          <span class="aname" class:inhand={a.hand}>{a.name}</span>
          <b class="hit rollbox" class:bad={!a.proficient}>{a.hit}</b>
          <span class="dmg">{a.damage}</span>
        </button>
        <button class="more" aria-label="More" onclick={() => attackMenu(a.slug, a.name, a.line)}>⋯</button>
        <div class="anotes">
          <span class="dim">{a.range}</span>
          {#each a.notes as n (n)}<span class="note">{n}</span>{/each}
          {#each a.problems as n (n)}<span class="bad">{n}</span>{/each}
          {#if a.properties.length}<span class="dim">{a.properties.join(", ")}</span>{/if}
        </div>
      </div>
    {/each}
    {#if o.riders.length}
      <div class="dim riders-title">On a hit</div>
      {#each o.riders as r (r.name)}
        <div class="rider"><b class="acc">{r.value}</b> {r.name} <span class="dim">{r.when}</span></div>
      {/each}
    {/if}
  </Pane>

 </div>
 <div class="side">
  {@render tracking()}
 </div>
</div>

{#snippet tracking()}
    <Pane title="Tracking" class="tracking" fold="sheet.tracking" summary="{tracked.length} tracked">
      {#each tracked as t (t.id)}
        <div class="tracker">
          <button class="tname" onclick={() => trackerMenu(t)}>{t.name}</button>
          <span class="pips">{#if t.max <= 12}<span class="acc">{"●".repeat(Math.max(t.left, 0))}</span><span class="dim">{"○".repeat(Math.max(t.max - t.left, 0))}</span>{:else}<b class="acc">{t.left}</b>{/if}</span>
          <span class="count">{t.left}/{t.max}</span>
          <button class="step" aria-label="Use" onclick={() => ctx.act({ command: "spend", pool: pool(t.id), amount: 1 })}>−</button>
          <button class="step" aria-label="Restore" onclick={() => ctx.act({ command: "spend", pool: pool(t.id), amount: -1 })}>+</button>
          <span class="dim recharge">{t.recharge}</span>
        </div>
      {/each}
      <button class="ghost addtracker" onclick={addTracker}>+ Tracker</button>
      {#each o.toggles as t (t.key)}
        <button class="toggle" onclick={() => ctx.act({ command: "set_toggle", key: t.key, on: !t.on })}>
          <span class:acc={t.on} class:dim={!t.on}>{t.on ? "◉" : "○"}</span>
          {t.name}
        </button>
      {/each}
    </Pane>
{/snippet}

<style>
  .overview {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(30ch, 40ch);
    gap: 1.4em 2ch;
    align-items: start;
    padding-top: 0.6em;
  }
  .side, .mid { display: flex; flex-direction: column; gap: 1.4em; min-width: 0; }
  .mid > p { margin: -1em 0 0; }

  :where(button:not(.ghost):not(.primary)) {
    background: none;
    border: 0;
    padding: 0;
    text-align: left;
    cursor: pointer;
  }
  .acc { color: var(--accent); }
  .bad { color: var(--red); }
  .ok { color: var(--green); }
  .small { font-size: 0.9em; }

  .hprow { display: flex; align-items: center; gap: 2ch; }
  .bar { flex: 1; height: 10px; background: var(--dark); border-radius: 2px; overflow: hidden; }
  .fill { height: 100%; }
  .hpnum b { font-size: 1.3em; }
  .temp { color: var(--cyan); margin-left: 1ch; }
  .rolled {
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 0.4em 1.5ch;
  }
  .rolled summary {
    display: flex;
    align-items: center;
    gap: 1.2ch;
    cursor: pointer;
    list-style: none;
    min-height: 2.2em;
  }
  .rolled summary::-webkit-details-marker { display: none; }
  .rolled summary::after { content: "▸"; color: var(--dim); }
  .rolled[open] summary::after { content: "▾"; }
  .rolled .last { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .rolled[open] .last { visibility: hidden; }
  .rolled .roll { padding: 0.3em 0; border-top: 1px solid rgb(65 72 104 / 0.35); }
  .hearticon { color: var(--red); }
  .hearticon.down { color: var(--dim); }
  .death { margin-top: 0.6em; display: flex; align-items: center; gap: 1.5ch; flex-wrap: wrap; }
  .death .ok, .death .bad { letter-spacing: 0.2ch; }
  .death button { margin-left: auto; }
  .buttons { display: flex; gap: 1ch; margin-top: 0.75em; }
  .buttons button { flex: 1; }

  .tiles { display: grid; grid-template-columns: repeat(5, 1fr); gap: 1ch; }
  .tile {
    display: flex;
    flex-direction: column;
    align-items: center;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 0.5em 0.5ch;
  }
  .tile b { color: var(--accent); font-size: 1.2em; }
  .tile span { font-size: 0.8em; text-align: center; }
  .tile .short { display: none; }
  .tile:hover { border-color: var(--accent); }

  .state { display: flex; gap: 1ch; flex-wrap: wrap; }
  .chip {
    border: 1px solid var(--border);
    border-radius: 14px;
    padding: 0.1em 1.5ch;
    color: var(--gray);
  }
  .chip.on { border-color: var(--magenta); color: var(--magenta); font-weight: 700; }
  .chip.mode.on { border-color: var(--cyan); color: var(--black); background: var(--cyan); }
  .conc { border-color: var(--magenta); color: var(--magenta); }

  .roll { margin-bottom: 0.4em; }
  .roll.old { opacity: 0.6; }
  .rtitle { margin-right: 1ch; }
  .roll b { color: var(--accent); margin-right: 1ch; }
  .roll b.crit { color: var(--green); }
  .roll b.fumble { color: var(--red); }
  .extra { padding-left: 2ch; }

  .held { margin-bottom: 0.5em; }
  .attack {
    display: grid;
    grid-template-columns: 1fr auto;
    padding: 0.25em 0;
    border-top: 1px solid rgb(65 72 104 / 0.35);
  }
  .arow { display: grid; grid-template-columns: 5ch minmax(0, 1fr) 6ch minmax(0, 24ch); gap: 1ch; align-items: center; }
  .arow:hover .aname { color: var(--accent); }
  .hand { color: var(--accent); }
  .aname { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .aname:not(.inhand) { color: var(--gray); }
  .aname.inhand { font-weight: 700; }
  .hit { justify-self: end; min-width: 5ch; }
  .hit.bad { color: var(--red); }
  .more { color: var(--gray); padding: 0 1ch; }
  .anotes { grid-column: 1 / -1; display: flex; flex-wrap: wrap; gap: 0 2ch; padding-left: 6ch; font-size: 0.9em; }
  .note { color: var(--accent); }
  .riders-title { margin-top: 0.75em; }

  /* Holding shows a breakdown: no text selection or callout on the way. */
  .tile { -webkit-user-select: none; user-select: none; -webkit-touch-callout: none; }

  .addtracker { margin: 0.5em 0 0.2em; }
  .tracker {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto 5ch 3ch 3ch;
    gap: 1ch;
    align-items: center;
    padding: 0.15em 0;
  }
  .tname { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .count { text-align: right; }
  .step { border: 1px solid var(--border); border-radius: 3px; text-align: center; color: var(--accent); }
  .recharge { grid-column: 1 / -1; font-size: 0.8em; margin-top: -0.3em; }
  .toggle { display: flex; gap: 1ch; padding: 0.15em 0; }
  .tracker.hitdie { grid-template-columns: minmax(0, 1fr) auto; column-gap: 2ch; row-gap: 0.3em; margin-top: 0.8em; }
  .tracker.hitdie .count { font-weight: 700; }
  .steps { display: flex; gap: 1ch; }
  .steps .step { width: 3ch; }
  .tracker.hitdie .tname { color: var(--gray); }
  .tracker.hitdie .tname b { color: var(--fg); }
  .hdbar { display: block; width: 100%; padding: 0; border: 0; cursor: pointer; }
  .hdbar .fill { background: var(--blue); }
  .hdbar:disabled { cursor: default; opacity: 0.6; }

  /* Tablets and narrow windows: the middle on top, then two columns. */
  @media (max-width: 1100px) {
    .overview { grid-template-columns: minmax(0, 1fr); }
  }
  /* Phones: one column, big targets. */
  @media (max-width: 760px) {
    .overview { grid-template-columns: minmax(0, 1fr); gap: 1.6em; }
    .side, .mid { gap: 1.6em; }
    /* One row: the numbers, with short labels. */
    .tiles { grid-template-columns: repeat(5, minmax(0, 1fr)); gap: 0.8ch; }
    .tile { padding: 0.45em 0; }
    .tile .long { display: none; }
    .tile .short { display: block; }
    .buttons button { min-height: 44px; }
    .chip { min-height: 36px; }
    .arow { grid-template-columns: 4ch minmax(0, 1fr) 6ch; min-height: 44px; }
    .dmg { grid-column: 2 / -1; font-size: 0.9em; color: var(--gray); }
    .anotes { padding-left: 5ch; }
    .tracker { min-height: 44px; grid-template-columns: minmax(0, 1fr) auto 5ch 44px 44px; }
    .step { height: 40px; font-size: 1.2em; }
    .toggle { min-height: 44px; align-items: center; }
    .tracker.hitdie { grid-template-columns: minmax(0, 1fr) auto; }
    .steps .step { width: 44px; }
  }
</style>
