<!-- The overview: hit points and the numbers, conditions, attacks and
     what is tracked (abilities and skills are on their own tab). Tap a
     number to roll it or see how it is made up. -->
<script lang="ts">
  import { killsOutright, type Overview, type Tracker } from "./api";
  import Pane from "./Pane.svelte";
  import Icon from "./Icon.svelte";
  import { clickRoll, pool, type SheetCtx } from "./sheetctx";
  import RollMode from "./RollMode.svelte";
  import { desktop } from "./screen";

  let { o, ctx, onconditions }: { o: Overview; ctx: SheetCtx; onconditions: () => void } = $props();

  const pct = $derived(o.hp.max ? Math.round((o.hp.current / o.hp.max) * 100) : 0);
  /// On a computer the bar holds the temporary hit points too, after the
  /// hit points: each a share of both.
  const whole = $derived(o.hp.max + o.hp.temp);
  const hpShare = $derived(whole ? (o.hp.current / whole) * 100 : 0);
  const tempShare = $derived(whole ? (o.hp.temp / whole) * 100 : 0);

  /// An attack's damage and a versatile weapon's two-handed damage apart
  /// (the core puts the latter in parentheses; on a computer it goes on a
  /// line of its own).
  function damageParts(a: { damage: string }): { main: string; alt: string } {
    const m = a.damage.match(/^(.*\S)\s*\((.+)\)$/);
    return m ? { main: m[1], alt: m[2] } : { main: a.damage, alt: "" };
  }
  /// Damage short for a phone's row: long words cut (`1d4 bludg.`).
  const shortDamage = (text: string) => text.replace(/\b([a-z]{5})[a-z]{2,}\b/g, "$1.");

  /// The traits there are, as label and text.
  const traits = $derived(
    (
      [
        ["Type", o.traits.creature_type ? [o.traits.creature_type] : []],
        ["Speeds", o.traits.speeds],
        ["Senses", o.traits.senses],
        ["Resists", o.traits.resistances],
        ["Immune", [...o.traits.immunities, ...o.traits.condition_immunities]],
        ["Vulnerable", o.traits.vulnerabilities],
        ["Tools", o.traits.tools],
      ] as [string, string[]][]
    ).filter(([, v]) => v.length),
  );
  /// No conditions and no exhaustion: the chip just says Conditions.
  const none = $derived(!o.conditions.length && !(o.exhaustion && o.exhaustion_name));
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

  /// The hit points set by hand (a phone: a tap on the title).
  function setHp() {
    ctx.prompt(
      "Hit points",
      (v) => {
        const n = parseInt(v, 10);
        if (!Number.isNaN(n)) ctx.act({ command: "set_hp", current: n });
      },
      { number: true, value: String(o.hp.current), hint: `0 to ${o.hp.max}` },
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
    const use: [string, () => void][] = [
      ["Use", () => ctx.act({ command: "spend", pool: pool(t.id), amount: 1 })],
      ["Restore", () => ctx.act({ command: "spend", pool: pool(t.id), amount: -1 })],
    ];
    if (t.id.startsWith("hit_die:")) {
      const roll: [string, () => void][] = t.left > 0 ? [["Roll one and heal", () => rollHitDie(t.id)]] : [];
      return ctx.menu(t.name, [...roll, ...use, ["How the dice are made up", () => ctx.explain(t.stat)]]);
    }
    // On a computer −/+ are by it; on a phone its row is the button.
    ctx.menu(t.name, [...(desktop.current ? [] : use), ["How the uses are made up", () => ctx.explain(t.stat)], ...trackerActions(t)]);
  }

  /// When the uses come back, short: LR, SR (the whole in its title).
  const SHORT_RECHARGE: Record<string, string> = { "long rest": "LR", "short rest": "SR", "long rest · 1 on a short": "LR, 1 SR" };

  /// A tracker's own actions beyond its uses (on a computer, buttons by it).
  function trackerActions(t: Tracker): [string, () => void][] {
    const key = t.id.startsWith("resource:") ? t.id.slice("resource:".length) : "";
    if (key.startsWith("custom_")) {
      const remove = () => ctx.confirm(`Remove ${t.name}`, "The tracker and its count are gone.", () => ctx.act({ command: "remove_tracker", key }), "Remove");
      return [["Remove the tracker…", remove]];
    }
    if (t.feature !== null) {
      const feature = t.feature;
      return [
        ["Show the feature", () => ctx.show(feature, t.part)],
        ["Change or remove its rules (and this tracker)", () => ctx.rules(feature)],
      ];
    }
    return [];
  }

  /// A tracker's actions by name on its one line (the full name its title).
  const SHORT_ACTION: Record<string, string> = {
    "Show the feature": "The feature",
    "Change or remove its rules (and this tracker)": "Its rules…",
    "Remove the tracker…": "Remove…",
  };

  /// The rolls section open (a phone; folded at first).
  let rollsOpen = $state(false);

  /// Explain beside the control it was asked from.
  const explainHere = (key: string) => (e: Event) => ctx.explain(key, e.currentTarget as Element);

  /// Holding a number (or a right click) shows how it is made up; a tap
  /// rolls it.
  const hold = (key: string) => (e: Event) => {
    e.preventDefault();
    ctx.explain(key, e.currentTarget as Element);
  };

  /// How an attack can be held (its inventory line), if it is an item:
  /// both hands only for a Versatile or Two-Handed weapon.
  function holds(a: { line: number | null; both_hands: boolean }): [string, string, () => void][] {
    const line = a.line;
    if (line === null) return [];
    return [
      ["Hold in the main hand", "Main hand", () => ctx.act({ command: "hold", line, hand: "main" })],
      ["Hold in the off hand", "Off hand", () => ctx.act({ command: "hold", line, hand: "off" })],
      ...(a.both_hands ? ([["Hold with both hands", "Both hands", () => ctx.act({ command: "hold", line, hand: "both" })]] as [string, string, () => void][]) : []),
    ];
  }

  function attackMenu(a: { slug: string; name: string; line: number | null; both_hands: boolean }) {
    const { slug, name } = a;
    const actions: [string, () => void][] = [["Roll the attack", () => ctx.roll({ type: "attack", slug })]];
    for (const [label, , run] of holds(a)) actions.push([label, run]);
    actions.push(["How the bonus is made up", () => ctx.explain(`attack.${slug}.bonus`)]);
    actions.push(["How the damage is made up", () => ctx.explain(`attack.${slug}.damage`)]);
    ctx.menu(name, actions);
  }

</script>

<div class="overview">
  <!-- Hit points first: what changes most in play (on a computer across the
       whole sheet, over both columns). -->
  <Pane title="Hit points" class="hp" fold="sheet.hp" section ontitle={desktop.current ? undefined : setHp} summary="{o.hp.current}/{o.hp.max}{o.hp.temp ? ` +${o.hp.temp}` : ''}">
    {#snippet aside()}
      {#if !desktop.current}
        <span class="hpacts">
          <button class="ghost mini dmgbtn" onclick={damage}>Damage</button>
          <button class="ghost mini healbtn" onclick={() => amount("Heal", "heal")}>Heal</button>
          <button class="ghost mini tempbtn" onclick={() => amount("Temporary hit points", "set_temp_hp")}>Temp</button>
        </span>
      {/if}
    {/snippet}
    <div class="hprow">
      <div class="bar hpbar" style:--hp="{hpShare}%" style:--tmp="{tempShare}%"><div class="fill" style:width="{pct}%" style:background={hpColor}></div><div class="tempfill"></div></div>
      <div class="hpnum"><span class="hearticon" class:down={o.hp.current === 0}><Icon name="heart" /></span> <b>{o.hp.current}</b><span class="muted">/{o.hp.max}</span>{#if o.hp.temp}<span class="temp">+{o.hp.temp} temp</span>{/if}</div>
    </div>
    <!-- The hit dice, right under: tap the bar to roll one and heal. On a
         computer one line under the hit points' bar: the temporary hit
         points, then each die with its buttons, every item kept whole. -->
    <div class="hpline">
      <span class="tempitem">Temp HP <b class="mono" class:tempn={o.hp.temp > 0}>{o.hp.temp}</b> · lost first</span>
      {#if !desktop.current && hitDice.length}
        <!-- A phone: the hit dice as chips; a tap rolls, uses or restores one. -->
        <div class="hdchips">
          <span class="dim hdlabel">HD</span>
          {#each hitDice as t (t.id)}
            <button class="hdchip tracker" onclick={() => trackerMenu(t)}><span class="tname"><span class="sr">Hit dice </span>d{t.id.slice(8)}</span><span class="count"><b class="acc">{t.left}</b>/{t.max}</span></button>
          {/each}
        </div>
      {/if}
      {#each desktop.current ? hitDice : [] as t (t.id)}
        <!-- As long as the hit points' bar, its count at the end; the die and
             −/+ (spend or restore one without rolling) under it. -->
        <div class="tracker hitdie">
          <button class="bar hdbar" aria-label="Roll d{t.id.slice(8)} + Con, heal" disabled={t.left <= 0} onclick={() => rollHitDie(t.id)}>
            <div class="fill" style:width="{t.max ? Math.round((t.left / t.max) * 100) : 0}%"></div>
            <span class="hdtext">Roll one and heal</span>
          </button>
          <span class="count">{t.left}/{t.max}</span>
          <button class="tname" onclick={() => trackerMenu(t)}>Hit dice <b>d{t.id.slice(8)}</b></button>
          <span class="steps">
            <button class="step" aria-label="Use" onclick={() => ctx.act({ command: "spend", pool: pool(t.id), amount: 1 })}>−</button>
            <button class="step" aria-label="Restore" onclick={() => ctx.act({ command: "spend", pool: pool(t.id), amount: -1 })}>+</button>
          </span>
          {#if desktop.current}<button class="info" aria-label="How the hit dice are made up" title="How the hit dice are made up" onclick={explainHere(t.stat)}>ⓘ</button>{/if}
        </div>
      {/each}
    </div>
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
    {#if desktop.current}
      <div class="buttons">
        <button class="ghost" onclick={damage}>Damage</button>
        <button class="ghost" onclick={() => amount("Heal", "heal")}>Heal</button>
        <button class="ghost" onclick={() => amount("Temporary hit points", "set_temp_hp")}>Temp</button>
      </div>
    {/if}
  </Pane>

 <div class="mid">
  <div class="tiles">
    {#each o.tiles as t (t.key)}
      <div class="tilewrap">
        <button
          class="tile"
          onclick={(e) => (t.key === "initiative" ? clickRoll(ctx, e, { type: "initiative" }) : ctx.explain(t.key, e.currentTarget))}
          oncontextmenu={hold(t.key)}
        >
          <b>{t.value}</b><span class="dim long">{t.label}</span><span class="dim short">{SHORT[t.label] ?? t.label}</span>
        </button>
        <!-- Initiative rolls when tapped: how it adds up, in sight. -->
        {#if desktop.current && t.key === "initiative"}<button class="info tileinfo" aria-label="How Initiative adds up" title="How it adds up" onclick={explainHere(t.key)}>ⓘ</button>{/if}
      </div>
    {/each}
  </div>

  <div class="state">
    <button class="chip" class:on={!none} aria-label={none ? "No conditions" : undefined} onclick={onconditions}>
      {none ? "Conditions" : [...o.conditions.map((c) => c.name), ...(o.exhaustion && o.exhaustion_name ? [`${o.exhaustion_name} ${o.exhaustion}`] : [])].join(" · ")}
    </button>
    {#if o.concentration}
      <button class="chip conc" onclick={() => ctx.act({ command: "end_concentration" })}>◎ {o.concentration} ✕</button>
    {/if}
    {#if !desktop.current}<button class="chip mode" class:on={o.next_roll !== "normal"} aria-label="next roll: {o.next_roll}" onclick={() => ctx.cycle()}>{o.next_roll}</button>{/if}
    <button class="chip dice" aria-label="Roll dice" title="Roll dice" onclick={() => ctx.dice()}><Icon name="d20" /></button>
  </div>
  {#if desktop.current}<RollMode mode={o.next_roll} {ctx} />{/if}
  {#each o.conditions as c (c.name)}<p class="dim small">{c.name}: {c.summary}</p>{/each}
  {#if o.exhaustion && o.exhaustion_name}<p class="dim small">{o.exhaustion_name}: {o.exhaustion_summary}</p>{/if}
  {#if traits.length}
    <dl class="traits small">
      {#each traits as [label, items] (label)}<dt class="dim">{label}</dt><dd>{items.join(" · ")}</dd>{/each}
    </dl>
  {/if}

  {#if o.rolls.length && !desktop.current}
    <!-- A phone: the latest rolls as a section, folded to its title line
         with the last roll on it. -->
    <section class="rolled rollsec">
      <button class="rolltitle" aria-expanded={rollsOpen} onclick={() => (rollsOpen = !rollsOpen)}>
        <b>{rollsOpen ? "▾" : "▸"} Rolls</b>
        <span class="dim">{o.rolls.length}</span>
        {#if !rollsOpen}<span class="last">{o.rolls[0].title} <b class:crit={o.rolls[0].crit === true} class:fumble={o.rolls[0].crit === false}>{o.rolls[0].result}</b></span>{/if}
      </button>
      {#if rollsOpen}
        {#each o.rolls.slice(0, 10) as r, i (i)}
          <div class="rrow" class:old={i > 0}>
            <span class="rtitle">{r.title}</span>
            <b class:crit={r.crit === true} class:fumble={r.crit === false}>{r.result}</b>
            <span class="dim rdetail">{[r.detail, ...r.extra].filter(Boolean).join(" · ")}</span>
          </div>
        {/each}
      {/if}
    </section>
  {:else if o.rolls.length}
    <!-- The latest rolls, folded away under the numbers: the status line
         (a toast on a computer) shows each roll. -->
    <details class="rolled">
      <summary>
        <Icon name="d20" />
        <span class="dim">Rolls ({o.rolls.length})</span>
        <span class="last">{o.rolls[0].title} <b class:crit={o.rolls[0].crit === true} class:fumble={o.rolls[0].crit === false}>{o.rolls[0].result}</b></span>
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

  <Pane title="Attacks" class="attacks" fold="sheet.attacks" section summary={o.held.length ? o.held.join(" · ") : `${o.attacks.length} attack${o.attacks.length === 1 ? "" : "s"}`}>
    {#snippet aside()}
      {#if !desktop.current}<span class="held inbar"><span class="dim">In hand</span> {o.held.length ? o.held.join(" · ") : "nothing"}</span>{/if}
    {/snippet}
    {#if desktop.current}<div class="held"><span class="dim">In hand</span> {o.held.length ? o.held.join(" · ") : "nothing"}</div>{/if}
    {#each o.attacks as a (a.slug)}
      {@const d = damageParts(a)}
      {#if !desktop.current}
        <!-- A phone: the name (its menu), the damage short, the roll at the
             right as everywhere. -->
        <div class="attack prow">
          <button class="pname" aria-label="{a.name}, more" onclick={() => attackMenu(a)}>
            <span class="hand">{a.hand}</span>
            <span class="aname" class:inhand={a.hand}>{a.name}</span>
            <span class="dmg">{shortDamage(d.main)}</span>
          </button>
          <button class="hit rollbox" class:bad={!a.proficient} aria-label="Roll {a.name}" onclick={(e) => clickRoll(ctx, e, { type: "attack", slug: a.slug })}>{a.hit}</button>
          <div class="anotes">
            <span class="dim">{a.range}</span>
            {#each a.notes as n (n)}<span class="note">{n}</span>{/each}
            {#each a.problems as n (n)}<span class="bad">{n}</span>{/each}
            {#if a.properties.length}<span class="dim">{a.properties.join(", ")}</span>{/if}
          </div>
        </div>
      {:else}
      <div class="attack">
        <button class="arow" onclick={(e) => clickRoll(ctx, e, { type: "attack", slug: a.slug })}>
          <span class="hand">{a.hand}</span>
          <span class="aname" class:inhand={a.hand}>{a.name}</span>
          <b class="hit rollbox" class:bad={!a.proficient}>{a.hit}</b>
          <span class="dmg">{d.main}{#if d.alt}<span class="alt"><span class="paren"> (</span>{d.alt}<span class="paren">)</span></span>{/if}</span>
        </button>
        <button class="more" aria-label="More" onclick={() => attackMenu(a)}>⋯</button>
        <div class="anotes">
          <span class="dim">{a.range}</span>
          {#each a.notes as n (n)}<span class="note">{n}</span>{/each}
          {#each a.problems as n (n)}<span class="bad">{n}</span>{/each}
          {#if a.properties.length}<span class="dim">{a.properties.join(", ")}</span>{/if}
        </div>
        {#if desktop.current}
          <!-- What the ⋯ menu does, in sight. -->
          <div class="aacts" role="group" aria-label="{a.name}: actions">
            {#each holds(a) as [label, text, run] (label)}<button class="ghost mini" aria-label={label} onclick={run}>{text}</button>{/each}
            <button class="info" aria-label="How the bonus is made up" onclick={explainHere(`attack.${a.slug}.bonus`)}>ⓘ to hit</button>
            <button class="info" aria-label="How the damage is made up" onclick={explainHere(`attack.${a.slug}.damage`)}>ⓘ damage</button>
          </div>
        {/if}
      </div>
      {/if}
    {/each}
    {#if o.riders.length}
      <div class="dim riders-title">On a hit</div>
      {#each o.riders as r (r.name)}
        <div class="rider"><b class="acc">{r.value}</b> {r.name} <span class="dim">{r.when}</span></div>
      {/each}
    {/if}
    {#if o.actions.length}
      <div class="dim riders-title">Actions</div>
      {#each o.actions as a (a.name)}
        <div class="action">
          <button class="actrow" onclick={() => ctx.act({ command: "use_action", name: a.name })}>
            <span class="aname inhand">{a.name}</span>
            <b class="acc">{a.what}</b>
            <span>{a.dice}</span>
          </button>
          <div class="anotes">
            {#if a.cost}<span class="dim">{a.cost}</span>{/if}
            {#if a.note}<span class="dim">{a.note}</span>{/if}
            <span class="dim">{a.origin}</span>
          </div>
        </div>
      {/each}
    {/if}
  </Pane>

 </div>
 <div class="side">
  {@render tracking()}
 </div>
</div>

{#snippet tracking()}
    <Pane title="Tracking" class="tracking" fold="sheet.tracking" section summary="{tracked.length} tracked">
      {#snippet aside()}
        {#if !desktop.current}<button class="addplus" aria-label="+ Tracker" onclick={addTracker}>+</button>{/if}
      {/snippet}
      {#each tracked as t (t.id)}
        {#if !desktop.current}
          <!-- A phone: the row is the button (use, restore, more). -->
          <button class="tracker trow" onclick={() => trackerMenu(t)}>
            <span class="tname">{t.name}</span>
            <span class="pips">{#if t.max <= 12}<span class="acc">{"●".repeat(Math.max(t.left, 0))}</span><span class="dim">{"○".repeat(Math.max(t.max - t.left, 0))}</span>{/if}</span>
            <span class="count">{t.left}/{t.max}</span>
            <span class="dim rshort" title={t.recharge}>{SHORT_RECHARGE[t.recharge] ?? t.recharge}</span>
          </button>
        {:else}
        <div class="tracker">
          <button class="tname" onclick={() => trackerMenu(t)}>{t.name}</button>
          <span class="pips">{#if t.max <= 12}<span class="acc">{"●".repeat(Math.max(t.left, 0))}</span><span class="dim">{"○".repeat(Math.max(t.max - t.left, 0))}</span>{:else}<b class="acc">{t.left}</b>{/if}</span>
          <span class="count">{t.left}/{t.max}</span>
          <button class="step" aria-label="Use" onclick={() => ctx.act({ command: "spend", pool: pool(t.id), amount: 1 })}>−</button>
          <button class="step" aria-label="Restore" onclick={() => ctx.act({ command: "spend", pool: pool(t.id), amount: -1 })}>+</button>
          <span class="dim recharge">{t.recharge}</span>
          {#if desktop.current}
            <!-- What its menu does, in sight. -->
            <span class="tacts" role="group" aria-label="{t.name}: actions">
              <button class="info" aria-label="How the count adds up" title="How the count adds up" onclick={explainHere(t.stat)}>ⓘ</button>
              {#each trackerActions(t) as [label, run] (label)}<button class="info" title={label} onclick={run}>{SHORT_ACTION[label] ?? label}</button>{/each}
            </span>
          {/if}
        </div>
        {/if}
      {/each}
      {#if desktop.current}<button class="ghost addtracker" onclick={addTracker}>+ Tracker</button>{/if}
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
  /* The hit points over the middle column, the side beside both. */
  .overview > :global(.hp) { grid-column: 1; }
  .mid { grid-column: 1; }
  .side { grid-column: 2; grid-row: 1 / span 2; }
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

  /* One row: the conditions and the concentration give way (they are
     spelled out below it), the next roll and the dice do not. */
  .state { display: flex; gap: 1ch; }
  .chip {
    border: 1px solid var(--border);
    border-radius: 14px;
    padding: 0.1em 1.5ch;
    color: var(--gray);
    flex-shrink: 0;
    white-space: nowrap;
  }
  .state .chip:not(.mode, .dice) { flex-shrink: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; }
  .chip.on { border-color: var(--magenta); color: var(--magenta); font-weight: 700; }
  .chip.mode.on { border-color: var(--cyan); color: var(--black); background: var(--cyan); }
  .chip.dice { padding-inline: 1.2ch; }
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
  .actrow { display: flex; flex-wrap: wrap; gap: 0 2ch; width: 100%; text-align: left; padding: 0.25em 0; }
  .traits { display: grid; grid-template-columns: auto minmax(0, 1fr); gap: 0.1em 2ch; margin: 0.5em 0; }
  .traits dd { margin: 0; }

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

  .tempfill { display: none; }
  /* The hit dice and temporary hit points line is a computer's only. */
  .hpline { display: contents; }
  .tempitem, .hdtext { display: none; }
  .tilewrap { position: relative; display: flex; min-width: 0; }
  .tilewrap .tile { flex: 1; min-width: 0; }
  .info {
    font: inherit;
    font-size: 0.85em;
    color: var(--cyan);
    background: none;
    border: 1px solid transparent;
    border-radius: 0.4rem;
    padding: 0.1rem 0.4rem;
    cursor: pointer;
    white-space: nowrap;
  }
  .info:hover { border-color: var(--cyan); }
  .mini { min-height: 1.9rem !important; padding: 0 0.6rem !important; font-size: 0.85em; }

  /* A computer: the hit points across the top (the number, the bar with
     the hit dice under it, the buttons), the numbers as cards, the attacks
     as a table; everything a share of the window. */
  @media (pointer: fine) and (min-width: 761px) {
    .overview { grid-template-columns: minmax(0, 1fr) minmax(16rem, 30%); gap: 1.2rem; padding-top: 0.2rem; }
    .side, .mid { gap: 1.2rem; }
    .overview > :global(.hp) { grid-column: 1 / -1; }
    .side { grid-row: 2; }
    .mid > p { margin: -0.8rem 0 0; }
    .overview :global(.hp .body) {
      display: grid;
      grid-template-columns: auto minmax(10rem, 1fr) auto;
      column-gap: 1.5rem;
      row-gap: 0.85rem;
      align-items: center;
    }
    .hprow { display: contents; }
    .hpnum { grid-column: 1; grid-row: 1 / span 2; font-size: 1.5em; white-space: nowrap; }
    .hpnum b { font-family: var(--mono); font-size: 1.4em; }
    .temp { font-size: 0.7em; }
    .hpbar { grid-column: 2; grid-row: 1; display: flex; gap: 2px; height: 0.75rem; border-radius: 0.4rem; background: var(--raised); }
    .hpbar .fill { width: var(--hp) !important; border-radius: 0.4rem 0 0 0.4rem; }
    .hpbar .tempfill { display: block; width: var(--tmp); background: var(--cyan); border-radius: 0 0.4rem 0.4rem 0; }
    .hpline {
      grid-column: 2;
      grid-row: 2;
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      column-gap: 1.6rem;
      row-gap: 0.4rem;
      color: var(--gray);
      font-size: 0.92em;
    }
    .tempitem { display: inline; white-space: nowrap; }
    .tempitem b { color: var(--fg); }
    .tempitem b.tempn { color: var(--cyan); }
    .tracker.hitdie { display: flex; align-items: center; gap: 0.5rem; margin: 0; white-space: nowrap; min-height: 0; }
    .tracker.hitdie .tname { order: 1; color: var(--gray); }
    .tracker.hitdie .count { order: 2; font-weight: 700; color: var(--fg); }
    .tracker.hitdie .hdbar { order: 3; }
    .tracker.hitdie .steps { order: 4; }
    .tracker.hitdie .info { order: 5; }
    .hdbar {
      width: auto;
      height: 1.9rem;
      padding: 0 0.6rem;
      background: var(--raised);
      border: 1px solid var(--border);
      border-radius: 0.5rem;
      color: var(--fg);
      font: inherit;
      font-size: 0.9em;
    }
    .hdbar .fill { display: none; }
    .hdtext { display: inline; }
    .steps .step { width: 1.9rem; height: 1.9rem; }
    .tileinfo { position: absolute; top: 0.3rem; right: 0.3rem; padding: 0 0.3rem; }
    .aacts { grid-column: 1 / -1; display: flex; flex-wrap: wrap; gap: 0.35rem; margin-top: 0.4rem; }
    .tacts { grid-column: 1 / -1; display: flex; flex-wrap: nowrap; align-items: center; gap: 0.15rem; margin: -0.1rem 0 0.35rem; font-size: 0.85em; white-space: nowrap; }
    .tacts .info { padding: 0 0.35rem; }
    .death { grid-column: 1 / -1; margin-top: 0; }
    .buttons { grid-column: 3; grid-row: 1 / span 2; margin-top: 0; flex-wrap: wrap; justify-content: flex-end; }
    .buttons button { flex: none; }

    .tiles { grid-template-columns: repeat(auto-fit, minmax(8rem, 1fr)); gap: 0.75rem; }
    .tile { background: var(--card); border-color: var(--edge); padding: 0.7rem 0.5rem; }
    .tile b { font-family: var(--mono); font-size: 1.6em; }
    .tile span { font-size: 0.85em; color: var(--gray); }

    .chip { padding: 0.2rem 0.9rem; }
    .rolled { background: var(--card); border-color: var(--edge); padding: 0.5rem 1.1rem; }
    .rolled b { font-family: var(--mono); }

    .attack { padding: 0.5rem 0; border-top-color: var(--edge); }
    /* No empty gutter: the hand marker only when an attack is in hand. */
    .arow { grid-template-columns: auto minmax(8rem, 1fr) 4.5rem minmax(12rem, 1.2fr); column-gap: 0; }
    .arow > * { margin-right: 1rem; }
    .hand { grid-column: 1; }
    .hand:empty { margin-right: 0; }
    .aname { grid-column: 2; }
    .hit { grid-column: 3; }
    .dmg { grid-column: 4; }
    .hit { justify-self: start; font-family: var(--mono); }
    .dmg { font-family: var(--mono); white-space: nowrap; }
    .alt { display: block; font-family: var(--font); font-size: 0.85em; color: var(--dim); }
    .alt .paren { display: none; }
    .alt::before { content: "Two-handed: "; }
    .anotes { padding-left: 0; color: var(--gray); }
    .tracker { grid-template-columns: minmax(0, 1fr) auto 3.5rem 2rem 2rem; }
    .step { height: 1.9rem; border-radius: 0.4rem; }
    .count { font-family: var(--mono); }
  }
  /* A wide window: the tracking narrower beside the rest. */
  @media (pointer: fine) and (min-width: 1800px) {
    .overview { grid-template-columns: minmax(0, 1fr) minmax(18rem, 24%); }
  }

  /* Tablets and narrow windows: the middle on top, then two columns. */
  @media (max-width: 1100px) {
    .overview { grid-template-columns: minmax(0, 1fr); }
    .overview > :global(.hp), .mid, .side { grid-column: 1; grid-row: auto; }
  }
  /* Phones and tablets: sections as gold lines (see Pane), rows to tap. */
  @media not ((pointer: fine) and (min-width: 761px)) {
    .hpacts { display: flex; gap: 0.6ch; }
    .hpacts .mini { min-height: 24px !important; height: 24px; padding: 0 1.2ch !important; font-size: 0.8em; line-height: 1; }
    .rolled.rollsec { border: 0; border-radius: 0; padding: 0; background: none; }
    .rolltitle {
      display: flex;
      align-items: center;
      gap: 1ch;
      width: 100%;
      min-height: 32px;
      border-bottom: 1px solid rgb(224 175 104 / 0.55) !important;
      font: inherit;
      color: var(--fg);
    }
    .rolltitle > b:first-child { color: var(--accent); font-size: 1.15em; }
    .rolltitle .last { margin-left: auto; font-size: 0.85em; color: var(--gray); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; min-width: 0; }
    .rolltitle .last b { color: var(--accent); font-size: 1.15em; }
    .rrow { display: grid; grid-template-columns: minmax(0, 1fr) auto; column-gap: 1ch; padding: 0.3em 0; border-bottom: 1px solid var(--raised); }
    .rrow > b { color: var(--accent); font-size: 1.15em; }
    .rrow.old { opacity: 0.7; }
    .rdetail { grid-column: 1 / -1; font-size: 0.8em; }
    .crit { color: var(--green) !important; }
    .fumble { color: var(--red) !important; }
    .dmgbtn { color: var(--red); }
    .healbtn { color: var(--green); }
    .tempbtn { color: var(--blue); }
    .hprow { min-height: 36px; }
    .bar { height: 8px; }
    .hdchips { display: flex; align-items: center; gap: 1ch; min-height: 36px; }
    .hdlabel { font-size: 0.85em; }
    .sr { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); white-space: nowrap; }
    .hdchip { display: inline-flex !important; gap: 1ch; align-items: center; min-height: 30px; padding: 0 1.2ch !important; background: var(--card) !important; border: 1px solid var(--border) !important; border-radius: 6px; }
    .addplus { color: var(--cyan); font-size: 1.3em; min-width: 36px; min-height: 32px; text-align: center !important; }
    .trow {
      width: 100%;
      grid-template-columns: minmax(0, 1fr) auto auto auto;
      min-height: 40px;
      border-bottom: 1px solid var(--raised) !important;
      color: var(--fg);
      font: inherit;
    }
    .rshort { font-size: 0.8em; text-align: right; white-space: nowrap; min-width: 3ch; }
  }
  /* Phones: one column. */
  @media (max-width: 760px) {
    .overview { grid-template-columns: minmax(0, 1fr); gap: 1em; }
    .side, .mid { gap: 1em; }
    /* One row: the numbers, with short labels. */
    .tiles { grid-template-columns: repeat(5, minmax(0, 1fr)); gap: 0.8ch; }
    .tile { padding: 0.3em 0; min-height: 50px; justify-content: center; background: var(--card); }
    .tile b { font-size: 1.25em; }
    .tile .long { display: none; }
    .tile .short { display: block; }
    .chip { min-height: 30px; font-size: 0.85em; }
    .traits { font-size: 0.85em; margin: 0; color: var(--gray); }
    /* One row: the name and the damage short (its menu), the roll at the
       right; the range and notes small under it. */
    .prow { grid-template-columns: minmax(0, 1fr) auto; align-items: center; column-gap: 1ch; border-top-color: var(--raised); }
    .pname { display: flex; align-items: baseline; gap: 1ch; min-height: 40px; min-width: 0; align-items: center; }
    .pname .hand:empty { display: none; }
    .pname .aname { flex: 0 1 auto; min-width: 0; color: var(--fg); }
    .pname .dmg { flex: 1 1 auto; min-width: 0; font-size: 0.85em; color: var(--gray); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
    .prow .hit { justify-self: end; min-width: 5ch; text-align: center !important; white-space: nowrap; padding: 0.15em 1ch !important; border: 1px solid var(--border) !important; border-radius: 6px; background: var(--card) !important; color: var(--accent); font-weight: 700; }
    .prow .hit.bad { color: var(--red); }
    .held.inbar { font-size: 0.8em; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; color: var(--fg); }
    .prow .anotes { padding-left: 0; font-size: 0.8em; margin-top: -0.5em; }
    .toggle { min-height: 40px; align-items: center; }
  }
</style>
