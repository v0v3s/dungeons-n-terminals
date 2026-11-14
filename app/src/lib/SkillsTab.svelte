<!-- Abilities and skills: a table of the abilities (score, check, save) and
     the skills. A boxed number rolls when tapped; holding it (or a name)
     shows how it adds up. -->
<script lang="ts">
  import type { Overview } from "./api";
  import Pane from "./Pane.svelte";
  import type { SheetCtx } from "./sheetctx";
  import { desktop, touch } from "./screen";

  let { o, ctx }: { o: Overview; ctx: SheetCtx } = $props();

  /// How a number adds up, in a pop-up beside the control asked from.
  const explainHere = (key: string) => (e: Event) => ctx.explain(key, e.currentTarget as Element);

  /// How to see how a number adds up: hold it, or right-click it.
  const how = $derived(touch.current ? "hold" : "right-click");

  const MARK = { none: "○", half: "◐", proficient: "●", expertise: "◉" };

  const hold = (key: string) => (e: Event) => {
    e.preventDefault();
    ctx.explain(key, e.currentTarget as Element);
  };
</script>

<div class="skills-tab">
  <Pane title="Abilities" class="abilities" fold="sheet.abilities" footer="{how} a number: how it adds up">
    <div class="grid abil" class:desk={desktop.current}>
      <span class="dim c">score</span><span class="dim">ability</span><span class="dim c">check</span>{#if desktop.current}<span></span>{/if}<span class="dim c">save</span>{#if desktop.current}<span></span>{/if}
      {#each o.abilities as a (a.key)}
        <span class="score" data-of={a.name}>{a.score}</span>
        <button class="aname" onclick={(e) => ctx.explain(a.key, e.currentTarget)} oncontextmenu={hold(a.key)}>{a.name}</button>
        <button class="num rollbox" onclick={() => ctx.roll({ type: "ability", key: a.key })} oncontextmenu={hold(`mod.${a.key}`)}>{a.modifier}</button>
        {#if desktop.current}<button class="info" aria-label="How the {a.name} check adds up" title="How it adds up" onclick={explainHere(`mod.${a.key}`)}>ⓘ</button>{/if}
        <button
          class="num rollbox save"
          class:prof={a.proficient}
          onclick={() => ctx.roll({ type: "save", key: a.key })}
          oncontextmenu={hold(`save.${a.key}`)}>{a.save}{a.proficient ? " ●" : ""}</button
        >
        {#if desktop.current}<button class="info" aria-label="How the {a.name} save adds up" title="How it adds up" onclick={explainHere(`save.${a.key}`)}>ⓘ</button>{/if}
      {/each}
    </div>
    {#each o.casting as c (c.name)}
      <div class="cast"><span class="muted">{c.name}</span> <span class="dim">DC</span> <b>{c.dc}</b> <span class="dim">attack</span> <b>{c.attack}</b></div>
    {/each}
  </Pane>

  <Pane title="Skills" class="skills" fold="sheet.skills" footer="{how} one: how it adds up">
    {#each o.skills as s (s.key)}
      <div class="skrow">
        <button class="skill" onclick={() => ctx.roll({ type: "skill", key: s.key })} oncontextmenu={hold(`skill.${s.key}`)}>
          <span class="mark" class:on={s.proficiency !== "none"}>{MARK[s.proficiency]}</span>
          <span class="sname">{s.name} <span class="dim">{s.ability.toUpperCase()}</span></span>
          <b class="rollbox">{s.bonus}</b>
        </button>
        {#if desktop.current}<button class="info" aria-label="How {s.name} adds up" title="How it adds up" onclick={explainHere(`skill.${s.key}`)}>ⓘ</button>{/if}
      </div>
    {/each}
  </Pane>
</div>

<style>
  .skills-tab {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 1.4em 2ch;
    align-items: start;
    padding-top: 0.6em;
  }
  :where(button:not(.ghost):not(.primary)) {
    background: none;
    border: 0;
    padding: 0;
    text-align: left;
    cursor: pointer;
  }
  .grid.abil {
    display: grid;
    grid-template-columns: 5ch minmax(0, 1fr) 6ch 7ch;
    gap: 0.3em 1ch;
    align-items: center;
  }
  .c,
  .score {
    text-align: center;
  }
  .score {
    font-size: 1.15em;
  }
  .aname {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .num.save:not(.prof) {
    color: var(--fg);
    font-weight: 400;
  }
  /* Holding shows a breakdown: no text selection or callout on the way. */
  .num,
  .aname,
  .skill {
    -webkit-user-select: none;
    user-select: none;
    -webkit-touch-callout: none;
  }
  .cast {
    margin-top: 0.75em;
  }
  .skill {
    display: grid;
    grid-template-columns: 2ch 1fr 6ch;
    gap: 1ch;
    align-items: center;
    width: 100%;
    padding: 0.15em 0;
  }
  .skill:hover .sname {
    color: var(--accent);
  }
  .mark {
    color: var(--dim);
  }
  .mark.on {
    color: var(--accent);
  }
  /* A computer: the abilities beside the skills, which flow into as many
     columns as the window has room for (alphabetical, top to bottom). */
  @media (pointer: fine) and (min-width: 761px) {
    .skills-tab {
      grid-template-columns: minmax(24rem, 1fr) minmax(0, 2fr);
      gap: 1.2rem;
      padding-top: 0.2rem;
    }
    .grid.abil {
      grid-template-columns: 3rem minmax(0, 1fr) 4.5rem 5rem;
      gap: 0.45rem 1rem;
    }
    .grid.abil.desk {
      grid-template-columns: 2.8rem minmax(6.5rem, 1fr) 3.8rem 1.5rem 4.6rem 1.5rem;
      column-gap: 0.35rem;
    }
    /* The column heads small, each in its own column with room apart. */
    .grid.abil.desk > span.dim {
      font-size: 0.8em;
      white-space: nowrap;
    }
    .grid.abil.desk > span.dim:nth-child(2) {
      padding-left: 0.5rem;
    }
    .aname {
      text-decoration: underline dotted var(--dim);
      text-underline-offset: 0.25em;
    }
    .skrow {
      display: flex;
      align-items: center;
      gap: 0.2rem;
      break-inside: avoid;
    }
    .skrow .skill {
      flex: 1;
      min-width: 0;
    }
    .info {
      font: inherit;
      font-size: 0.85em;
      color: var(--cyan);
      background: none;
      border: 1px solid transparent;
      border-radius: 0.4rem;
      padding: 0.1rem 0.35rem;
      cursor: pointer;
    }
    .info:hover {
      border-color: var(--cyan);
    }
    .score {
      font-family: var(--mono);
    }
    .num {
      font-family: var(--mono);
      white-space: nowrap;
    }
    .skills-tab :global(.skills .body) {
      display: block;
      columns: 15rem;
      column-gap: 2rem;
    }
    .skill {
      break-inside: avoid;
      grid-template-columns: 1.2rem minmax(0, 1fr) 4rem;
      padding: 0.3rem 0.4rem;
      border-radius: 0.4rem;
    }
    .skill:hover {
      background: var(--raised);
    }
    .skill b {
      font-family: var(--mono);
    }
    .sname {
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
    }
  }
  @media (max-width: 760px) {
    .skills-tab {
      grid-template-columns: minmax(0, 1fr);
      gap: 1.6em;
    }
    .grid.abil > * {
      min-height: 40px;
      display: flex;
      align-items: center;
    }
    .grid.abil .c,
    .grid.abil .score,
    .grid.abil .num {
      justify-content: center;
    }
    .skill {
      min-height: 44px;
    }
  }
</style>
