<!-- The overview: hit points and the numbers, abilities, skills, attacks
     and what is tracked. Tap a number to roll it or see how it is made up. -->
<script lang="ts">
  import type { Overview } from "./api";
  import Pane from "./Pane.svelte";
  import { pool, type SheetCtx } from "./sheetctx";

  let { o, ctx, onconditions }: { o: Overview; ctx: SheetCtx; onconditions: () => void } = $props();

  const pct = $derived(o.hp.max ? Math.round((o.hp.current / o.hp.max) * 100) : 0);
  const hpColor = $derived(pct <= 25 ? "var(--red)" : pct <= 50 ? "var(--yellow)" : "var(--green)");
  const MARK = { none: "○", half: "◐", proficient: "●", expertise: "◉" };

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
  <Pane title="Hit points" class="hp">
    <div class="hprow">
      <div class="bar"><div class="fill" style:width="{pct}%" style:background={hpColor}></div></div>
      <div class="hpnum"><b>{o.hp.current}</b><span class="muted">/{o.hp.max}</span>{#if o.hp.temp}<span class="temp">+{o.hp.temp} temp</span>{/if}</div>
    </div>
    {#if o.hp.current === 0}
      <div class="death">Death saves <span class="ok">{"✓".repeat(o.hp.death_successes)}</span> <span class="bad">{"✗".repeat(o.hp.death_failures)}</span></div>
    {/if}
    <div class="buttons">
      <button class="ghost" onclick={() => amount("Damage", "damage")}>Damage</button>
      <button class="ghost" onclick={() => amount("Heal", "heal")}>Heal</button>
      <button class="ghost" onclick={() => amount("Temporary hit points", "set_temp_hp")}>Temp</button>
    </div>
  </Pane>

  <div class="tiles">
    {#each o.tiles as t (t.key)}
      <button class="tile" onclick={() => (t.key === "initiative" ? ctx.roll({ type: "initiative" }) : ctx.explain(t.key))}>
        <b>{t.value}</b><span class="dim">{t.label}</span>
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
    <Pane title="Rolled" class="rolls">
      {#each o.rolls.slice(0, 3) as r, i (i)}
        <div class="roll" class:old={i > 0}>
          <span class="rtitle">{r.title}</span>
          <b class:crit={r.crit === true} class:fumble={r.crit === false}>{r.result}</b>
          <span class="dim">{r.detail}</span>
          {#each r.extra as e, j (j)}<div class="extra">{e}</div>{/each}
        </div>
      {/each}
    </Pane>
  {/if}

  <Pane title="Attacks" class="attacks">
    <div class="held"><span class="dim">In hand</span> {o.held.length ? o.held.join(" · ") : "nothing"}</div>
    {#each o.attacks as a (a.slug)}
      <div class="attack">
        <button class="arow" onclick={() => ctx.roll({ type: "attack", slug: a.slug })}>
          <span class="hand">{a.hand}</span>
          <span class="aname" class:inhand={a.hand}>{a.name}</span>
          <b class="hit" class:bad={!a.proficient}>{a.hit}</b>
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
 <div class="left">
  <Pane title="Abilities" class="abilities">
    <div class="grid abil">
      <span></span><span class="dim c">score</span><span class="dim c">check</span><span class="dim c">save</span>
      {#each o.abilities as a (a.key)}
        <span>{a.name}</span>
        <span class="c">{a.score}</span>
        <button class="num" onclick={() => ctx.roll({ type: "ability", key: a.key })}>{a.modifier}</button>
        <button class="num save" class:prof={a.proficient} onclick={() => ctx.roll({ type: "save", key: a.key })}>{a.save}{a.proficient ? " ●" : ""}</button>
      {/each}
    </div>
    {#each o.casting as c (c.name)}
      <div class="cast"><span class="muted">{c.name}</span> <span class="dim">DC</span> <b>{c.dc}</b> <span class="dim">attack</span> <b>{c.attack}</b></div>
    {/each}
  </Pane>

  {@render tracking()}
 </div>
 <div class="right">
  <Pane title="Skills" class="skills">
    {#each o.skills as s (s.key)}
      <button class="skill" onclick={() => ctx.roll({ type: "skill", key: s.key })}>
        <span class="mark" class:on={s.proficiency !== "none"}>{MARK[s.proficiency]}</span>
        <span class="sname">{s.name} <span class="dim">{s.ability.toUpperCase()}</span></span>
        <b>{s.bonus}</b>
      </button>
    {/each}
  </Pane>

 </div>
</div>

{#snippet tracking()}
  {#if o.trackers.length || o.toggles.length}
    <Pane title="Tracking" class="tracking">
      {#each o.trackers as t (t.id)}
        <div class="tracker">
          <button class="tname" onclick={() => ctx.explain(t.stat)}>{t.name}</button>
          <span class="pips">{#if t.max <= 12}<span class="acc">{"●".repeat(Math.max(t.left, 0))}</span><span class="dim">{"○".repeat(Math.max(t.max - t.left, 0))}</span>{:else}<b class="acc">{t.left}</b>{/if}</span>
          <span class="count">{t.left}/{t.max}</span>
          <button class="step" aria-label="Use" onclick={() => ctx.act({ command: "spend", pool: pool(t.id), amount: 1 })}>−</button>
          <button class="step" aria-label="Restore" onclick={() => ctx.act({ command: "spend", pool: pool(t.id), amount: -1 })}>+</button>
          <span class="dim recharge">{t.recharge}</span>
        </div>
      {/each}
      {#each o.toggles as t (t.key)}
        <button class="toggle" onclick={() => ctx.act({ command: "set_toggle", key: t.key, on: !t.on })}>
          <span class:acc={t.on} class:dim={!t.on}>{t.on ? "◉" : "○"}</span>
          {t.name}
        </button>
      {/each}
    </Pane>
  {/if}
{/snippet}

<style>
  .overview {
    display: grid;
    grid-template-columns: minmax(30ch, 36ch) minmax(0, 1fr) minmax(30ch, 36ch);
    gap: 1.4em 2ch;
    align-items: start;
    padding-top: 0.6em;
  }
  .left, .mid, .right { display: flex; flex-direction: column; gap: 1.4em; min-width: 0; }
  .left { order: 1; }
  .mid { order: 2; }
  .right { order: 3; }
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
  .death { margin-top: 0.4em; }
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
  .arow { display: grid; grid-template-columns: 5ch minmax(0, 1fr) 5ch minmax(0, 24ch); gap: 1ch; align-items: baseline; }
  .arow:hover .aname { color: var(--accent); }
  .hand { color: var(--accent); }
  .aname { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .aname:not(.inhand) { color: var(--gray); }
  .aname.inhand { font-weight: 700; }
  .hit { color: var(--accent); text-align: right; }
  .hit.bad { color: var(--red); }
  .more { color: var(--gray); padding: 0 1ch; }
  .anotes { grid-column: 1 / -1; display: flex; flex-wrap: wrap; gap: 0 2ch; padding-left: 6ch; font-size: 0.9em; }
  .note { color: var(--accent); }
  .riders-title { margin-top: 0.75em; }

  .grid.abil { display: grid; grid-template-columns: 1fr 6ch 6ch 7ch; row-gap: 0.15em; align-items: baseline; }
  .c { text-align: center; }
  .num { text-align: center; font-weight: 700; color: var(--accent); }
  .num.save { color: var(--fg); font-weight: 400; }
  .num.save.prof { color: var(--accent); }
  .num:hover { background: var(--selected); }
  .cast { margin-top: 0.75em; }

  .skill { display: grid; grid-template-columns: 2ch 1fr auto; gap: 1ch; width: 100%; padding: 0.05em 0; }
  .skill:hover { background: var(--selected); }
  .mark { color: var(--dim); }
  .mark.on { color: var(--accent); }

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

  /* Tablets and narrow windows: the middle on top, then two columns. */
  @media (max-width: 1100px) {
    .overview { grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); }
    .mid { order: 0; grid-column: 1 / -1; }
  }
  /* Phones: one column, big targets. */
  @media (max-width: 760px) {
    .overview { grid-template-columns: minmax(0, 1fr); gap: 1.2em; }
    .left, .mid, .right { gap: 1.2em; }
    .tiles { grid-template-columns: repeat(3, 1fr); }
    .tile:nth-child(n + 4) { grid-column: span 1; }
    .buttons button { min-height: 44px; }
    .chip { min-height: 36px; }
    .arow { grid-template-columns: 4ch minmax(0, 1fr) 4ch; min-height: 44px; }
    .dmg { grid-column: 2 / -1; font-size: 0.9em; color: var(--gray); }
    .anotes { padding-left: 5ch; }
    .grid.abil > * { min-height: 40px; display: flex; align-items: center; }
    .grid.abil .c, .grid.abil .num { justify-content: center; }
    .skill { min-height: 40px; align-items: center; }
    .tracker { min-height: 44px; grid-template-columns: minmax(0, 1fr) auto 5ch 44px 44px; }
    .step { height: 40px; font-size: 1.2em; }
    .toggle { min-height: 44px; align-items: center; }
  }
</style>
