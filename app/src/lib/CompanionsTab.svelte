<!-- Companions: the creatures serving the character (a familiar, a steed,
     followers), each in a pane with its numbers. A boxed number rolls when
     tapped (for the companion); its actions roll the same way. Summoning
     asks what the rules allow: the creature, its type, the spell's level. -->
<script lang="ts">
  import * as api from "./api";
  import type { CompanionCard, CompanionRoll, Companions } from "./api";
  import { summon } from "./summon";
  import Pane from "./Pane.svelte";
  import { type SheetCtx } from "./sheetctx";
  import { desktop } from "./screen";

  let { companions, ctx }: { companions: Companions; ctx: SheetCtx } = $props();

  const ABBR: Record<string, string> = { str: "STR", dex: "DEX", con: "CON", int: "INT", wis: "WIS", cha: "CHA" };
  const signed = (n: number) => (n < 0 ? `${n}` : `+${n}`);
  /// Damage short, as on the Character tab's attacks (`1d6 + 2 pierc.`).
  const short = (text: string) => text.replace(/\b([a-z]{5})[a-z]{2,}\b/g, "$1.");
  /// Holding an ability rolls its save (a tap: the check).
  const holdSave = (c: CompanionCard, key: string) => (e: Event) => {
    e.preventDefault();
    roll(c, { roll: "save", key });
  };

  const roll = (c: CompanionCard, r: CompanionRoll) => ctx.roll({ type: "companion", index: c.index, roll: r });

  function follower() {
    ctx.pick({ type: "creatures" }, (values) => {
      if (values[0]) ctx.act({ command: "add_companion", creature: values[0], name: "", from: null });
    });
  }

  /// Add a companion: a follower (any creature), or one a feature gives
  /// (a spell's come when it is cast).
  function add() {
    const actions: [string, () => void][] = companions.givers
      .filter((g) => !g.id.startsWith("spell:"))
      .map((g) => [`${g.kind} (${g.name})${g.have >= g.most ? ` · ${g.have}/${g.most}` : ""}`, () => summon(ctx, g, null)]);
    actions.push(["A follower…", follower]);
    ctx.menu("Add a companion", actions);
  }

  function hp(c: CompanionCard, sign: 1 | -1) {
    ctx.prompt(sign > 0 ? `Damage to ${c.name}` : `Heal ${c.name}`, (v) => {
      const n = Number(v);
      if (n > 0) ctx.act({ command: "companion_hp", index: c.index, damage: sign * n });
    }, { number: true });
  }

  async function conditions(c: CompanionCard) {
    const all = await api.conditions();
    ctx.menu(
      `${c.name}: conditions`,
      all.map(([cond]) => {
        const has = c.conditions.includes(cond.name);
        return [`${has ? "✓ " : ""}${cond.name}`, () => ctx.act({ command: "set_companion_condition", index: c.index, condition: cond.name, on: !has })];
      }),
    );
  }

  function more(c: CompanionCard) {
    const actions: [string, () => void][] = [
      ["Rename…", () => ctx.prompt(`Rename ${c.name}`, (name) => ctx.act({ command: "rename_companion", index: c.index, name }), { value: c.name })],
      ["Temporary hit points…", () => ctx.prompt("Temporary hit points", (v) => ctx.act({ command: "companion_temp_hp", index: c.index, amount: Number(v) || 0 }), { number: true })],
      ["Conditions…", () => conditions(c)],
      [c.away ? "Back" : "Away (dismissed)", () => ctx.act({ command: "set_companion_away", index: c.index, away: !c.away })],
      ["Notes…", () => ctx.prompt(`${c.name}: notes`, (notes) => ctx.act({ command: "set_companion_notes", index: c.index, notes }), { value: c.notes })],
    ];
    if (c.entity != null) actions.unshift(["Stat block", () => ctx.show(c.entity as number)]);
    actions.push(["Remove…", () => ctx.confirm(`Remove ${c.name}?`, `${c.name} leaves the character.`, () => ctx.act({ command: "remove_companion", index: c.index }), "Remove")]);
    ctx.menu(c.name, actions);
  }
</script>

<div class="companions-tab">
  {#if !companions.list.length}
    <p class="muted empty">No companions. Cast a spell that summons one (Find Familiar), or add one below.</p>
  {/if}

  {#each companions.list as c (c.index)}
    {#if !desktop.current}
      <!-- A phone: the Character tab, smaller: the title line (Damage,
           Heal, ⋯), the small line, the HP row, number boxes, the
           abilities, the chips, the attacks. -->
      <Pane title={c.name} class="companion {c.away ? 'away' : ''}" fold="sheet.companion.{c.index}" section summary={c.numbers ? `HP ${c.numbers.hp}/${c.numbers.hp_max}` : ""}>
        {#snippet aside()}
          <span class="acts">
            {#if c.numbers}
              <button class="ghost mini dmg" onclick={() => hp(c, 1)}>Damage</button>
              <button class="ghost mini heal" onclick={() => hp(c, -1)}>Heal</button>
            {/if}
            <button class="ghost mini" aria-label="More for {c.name}" onclick={() => more(c)}>⋯</button>
          </span>
        {/snippet}
        <div class="line dim">
          <span class="kind">{c.kind}</span>{" · "}{[c.creature !== c.name ? c.creature : "", c.type_line, c.numbers?.cr ? `CR ${c.numbers.cr}` : "", c.numbers?.speed ?? ""].filter(Boolean).join(" · ")}{#if c.away}<span class="warn">{" · "}away</span>{/if}
        </div>
        {#if c.numbers}
          {@const n = c.numbers}
          {@const pct = n.hp_max ? Math.round((n.hp / n.hp_max) * 100) : 0}
          <div class="hp hprow">
            <span class="dim lbl">HP</span>
            <div class="bar"><div style:width="{pct}%" style:background={pct <= 25 ? "var(--red)" : pct <= 50 ? "var(--yellow)" : "var(--green)"}></div></div>
            <b>{n.hp}</b><span class="dim">/{n.hp_max}</span>{#if n.temp_hp}<span class="temp"> +{n.temp_hp}</span>{/if}
          </div>
          <div class="tiles">
            <span class="tile"><span class="lbl">AC</span> <b>{n.ac}</b></span>
            <button class="tile" onclick={() => roll(c, { roll: "initiative" })}><span class="lbl">Init</span> <b>{signed(n.initiative)}</b></button>
            <span class="tile"><span class="lbl">Passive Perception</span> <b>{n.passive_perception}</b></span>
          </div>
          <div class="abil6">
            {#each n.abilities as a (a.key)}
              <button class="tile small" aria-label="{ABBR[a.key]} check" title="{ABBR[a.key]} {a.score}; hold: the save {signed(a.save)}" onclick={() => roll(c, { roll: "ability", key: a.key })} oncontextmenu={holdSave(c, a.key)}
                ><span class="lbl">{ABBR[a.key]}</span> <b>{signed(a.modifier)}</b></button
              >
            {/each}
          </div>
          {#if n.skills.length || c.conditions.length}
            <div class="skills">
              {#each c.conditions as name (name)}<span class="cond">{name}</span>{/each}
              {#each n.skills as s (s.key)}
                <button class="chip" onclick={() => roll(c, { roll: "skill", key: s.key })}>{s.name} <b>{signed(s.bonus)}</b></button>
              {/each}
            </div>
          {/if}
          {#if n.senses || n.languages}<div class="line dim">{[n.senses, n.languages ? `Languages: ${n.languages}` : ""].filter(Boolean).join(" · ")}</div>{/if}
          {#each n.actions as a (a.name)}
            <button class="act arow" onclick={() => roll(c, { roll: "action", name: a.name })}>
              <span class="aname">{a.name} <span class="dim dmgtxt">{[a.dc != null ? `DC ${a.dc}${a.save ? ` ${a.save.toUpperCase()}` : ""}` : "", a.damage ? short(a.damage) : "", a.heal ? `heals ${a.heal}` : ""].filter(Boolean).join(" · ")}</span></span>
              {#if a.attack != null}<b class="rollbox">{signed(a.attack)}</b>{/if}
            </button>
          {/each}
        {:else}
          <p class="warn">{c.creature_id} is not in the data.</p>
        {/if}
        {#if c.notes}<p class="notes muted">{c.notes}</p>{/if}
      </Pane>
    {:else}
    <Pane title={c.name} class="companion {c.away ? 'away' : ''}" fold="sheet.companion.{c.index}" summary={c.numbers ? `HP ${c.numbers.hp}/${c.numbers.hp_max}` : ""}>
      <div class="top">
        <div class="who">
          <span class="kind">{c.kind}</span>{#if c.creature !== c.name}<span class="dim"> · {c.creature}</span>{/if}
          <div class="muted">{c.type_line}{#if c.numbers?.cr}<span class="dim"> · CR {c.numbers.cr}</span>{/if}{#if c.away}<span class="warn"> · away</span>{/if}</div>
        </div>
        <button class="more" aria-label="More for {c.name}" onclick={() => more(c)}>⋯</button>
      </div>
      {#if c.numbers}
        {@const n = c.numbers}
        <div class="vitals">
          <div class="hp">
            <span class="dim">HP</span> <b>{n.hp}</b><span class="dim">/{n.hp_max}</span>{#if n.temp_hp}<span class="temp"> +{n.temp_hp}</span>{/if}
            <button class="ghost small" onclick={() => hp(c, 1)}>Damage</button>
            <button class="ghost small" onclick={() => hp(c, -1)}>Heal</button>
          </div>
          <div><span class="dim">AC</span> <b>{n.ac}</b></div>
          <div><span class="dim">Init</span> <button class="rollbox" onclick={() => roll(c, { roll: "initiative" })}>{signed(n.initiative)}</button></div>
          <div class="speed"><span class="dim">Speed</span> {n.speed}</div>
        </div>
        <div class="abil">
          {#each n.abilities as a (a.key)}
            <div class="ab">
              <span class="dim">{ABBR[a.key]}</span>
              <span class="score">{a.score}</span>
              <button class="rollbox" aria-label="{ABBR[a.key]} check" onclick={() => roll(c, { roll: "ability", key: a.key })}>{signed(a.modifier)}</button>
              <button class="rollbox save" aria-label="{ABBR[a.key]} save" onclick={() => roll(c, { roll: "save", key: a.key })}>{signed(a.save)}</button>
            </div>
          {/each}
        </div>
        {#if n.skills.length}
          <div class="skills">
            {#each n.skills as s (s.key)}
              <button class="chip" onclick={() => roll(c, { roll: "skill", key: s.key })}>{s.name} <b>{signed(s.bonus)}</b></button>
            {/each}
          </div>
        {/if}
        <div class="lines muted">
          <div>Passive Perception {n.passive_perception}{#if n.senses}, {n.senses}{/if}</div>
          {#if n.languages}<div>Languages: {n.languages}</div>{/if}
        </div>
        {#if n.actions.length}
          <div class="actions">
            {#each n.actions as a (a.name)}
              <button class="act" onclick={() => roll(c, { roll: "action", name: a.name })}>
                <span class="aname">{a.name}</span>
                <span class="what">
                  {#if a.attack != null}<b class="rollbox">{signed(a.attack)}</b>{/if}
                  {#if a.dc != null}<b>DC {a.dc}</b>{#if a.save}<span class="dim"> {a.save.toUpperCase()}</span>{/if}{/if}
                  {#if a.damage}<span>{a.damage}</span>{/if}
                  {#if a.heal}<span>heals {a.heal}</span>{/if}
                </span>
              </button>
            {/each}
          </div>
        {/if}
      {:else}
        <p class="warn">{c.creature_id} is not in the data.</p>
      {/if}
      {#if c.conditions.length}
        <div class="conds">{#each c.conditions as name (name)}<span class="cond">{name}</span>{/each}</div>
      {/if}
      {#if c.notes}<p class="notes muted">{c.notes}</p>{/if}
    </Pane>
    {/if}
  {/each}

  <!-- At the end of the list: a slim line, out of the way. -->
  <button class="addline" aria-label="Add a companion" onclick={add}><span>+ Add a companion</span></button>
</div>

<style>
  /* A phone: the Character tab's pieces, smaller. */
  .acts {
    display: flex;
    gap: 0.6ch;
  }
  .acts .mini {
    min-height: 24px !important;
    height: 24px;
    padding: 0 1.2ch !important;
    font-size: 0.8em;
    line-height: 1;
  }
  .acts .dmg {
    color: var(--red);
  }
  .acts .heal {
    color: var(--green);
  }
  .line {
    font-size: 0.85em;
  }
  .hprow {
    display: flex;
    align-items: center;
    gap: 1ch;
    min-height: 32px;
  }
  .hprow .bar {
    flex: 1;
    height: 8px;
    background: var(--raised);
    border-radius: 3px;
    overflow: hidden;
  }
  .hprow .bar div {
    height: 100%;
  }
  .hprow b {
    font-size: 1.25em;
  }
  .lbl {
    color: var(--gray);
    font-size: 0.75em;
  }
  .tiles,
  .abil6 {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 6px;
    margin-top: 0.3em;
  }
  .abil6 {
    grid-template-columns: repeat(6, minmax(0, 1fr));
    gap: 4px;
  }
  .tile {
    display: flex;
    flex-direction: column-reverse;
    align-items: center;
    justify-content: center;
    min-height: 42px;
    background: var(--card) !important;
    border: 1px solid var(--border) !important;
    border-radius: 6px;
    text-align: center !important;
    -webkit-user-select: none;
    user-select: none;
    -webkit-touch-callout: none;
  }
  .tile b {
    color: var(--accent);
    font-size: 1.1em;
    line-height: 1.1;
  }
  .tile .lbl {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 100%;
  }
  .tile.small {
    min-height: 38px;
  }
  .arow {
    display: grid !important;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: 1ch;
    width: 100%;
    min-height: 36px;
    border-bottom: 1px solid var(--raised) !important;
  }
  .arow .aname {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dmgtxt {
    font-size: 0.85em;
  }
  .companions-tab {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 34ch), 1fr));
    gap: 1.4em 2ch;
    align-items: start;
    padding-top: 0.6em;
  }
  .empty {
    grid-column: 1 / -1;
  }
  .addline {
    grid-column: 1 / -1;
    display: flex;
    align-items: center;
    gap: 1ch;
    width: 100%;
    padding: 0.4em 0;
    color: var(--dim);
    text-align: center !important;
  }
  /* The dashes either side, as the TUI's ┄. */
  .addline::before,
  .addline::after {
    content: "";
    flex: 1;
    border-top: 1px dashed var(--border);
  }
  .addline:hover,
  .addline:focus-visible {
    color: var(--accent);
  }
  :where(button:not(.ghost):not(.primary):not(.action)) {
    background: none;
    border: 0;
    padding: 0;
    text-align: left;
    cursor: pointer;
    color: inherit;
    font: inherit;
  }
  :global(.companion.away) {
    opacity: 0.6;
  }
  .top {
    display: flex;
    justify-content: space-between;
    gap: 1ch;
  }
  .kind {
    color: var(--accent);
  }
  .more {
    font-size: 1.3em;
    padding: 0 0.5ch;
  }
  .vitals {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4em 2.5ch;
    align-items: center;
    margin: 0.6em 0;
  }
  .hp {
    display: flex;
    align-items: center;
    gap: 0.6ch;
    flex-basis: 100%;
  }
  .hp b {
    font-size: 1.2em;
  }
  .temp {
    color: var(--accent);
  }
  .small {
    font-size: 0.85em;
    padding: 0.1em 1ch;
  }
  .abil {
    display: grid;
    grid-template-columns: repeat(6, minmax(0, 1fr));
    gap: 0.3em 0.5ch;
    text-align: center;
  }
  .ab {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.15em;
  }
  .ab button {
    text-align: center;
  }
  .rollbox.save {
    font-size: 0.9em;
    opacity: 0.85;
  }
  .score {
    font-size: 0.9em;
  }
  .skills {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3em 1.5ch;
    margin-top: 0.6em;
  }
  .lines {
    margin-top: 0.5em;
    font-size: 0.9em;
  }
  .actions {
    margin-top: 0.6em;
    display: grid;
    gap: 0.2em;
  }
  .act {
    display: flex;
    justify-content: space-between;
    gap: 1ch;
    width: 100%;
    padding: 0.25em 0;
    border-top: 1px solid var(--border);
  }
  .what {
    display: flex;
    gap: 1ch;
    flex-wrap: wrap;
    justify-content: flex-end;
  }
  .conds {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3em 1ch;
    margin-top: 0.5em;
  }
  .cond {
    color: var(--red);
  }
  .warn {
    color: var(--red);
  }
  .notes {
    margin: 0.5em 0 0;
    white-space: pre-wrap;
  }
  /* Rolling numbers and names: no text selection on a tap. */
  button {
    -webkit-user-select: none;
    user-select: none;
  }
</style>
