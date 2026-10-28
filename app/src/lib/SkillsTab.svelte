<!-- Abilities and skills: a table of the abilities (score, check, save) and
     the skills. A boxed number rolls when tapped; holding it (or a name)
     shows how it adds up. -->
<script lang="ts">
  import type { Overview } from "./api";
  import Pane from "./Pane.svelte";
  import type { SheetCtx } from "./sheetctx";

  let { o, ctx }: { o: Overview; ctx: SheetCtx } = $props();

  const MARK = { none: "○", half: "◐", proficient: "●", expertise: "◉" };

  const hold = (key: string) => (e: Event) => {
    e.preventDefault();
    ctx.explain(key);
  };
</script>

<div class="skills-tab">
  <Pane title="Abilities" class="abilities" fold="sheet.abilities" footer="hold a number: how it adds up">
    <div class="grid abil">
      <span class="dim c">score</span><span class="dim">ability</span><span class="dim c">check</span><span class="dim c">save</span>
      {#each o.abilities as a (a.key)}
        <span class="score" data-of={a.name}>{a.score}</span>
        <button class="aname" onclick={() => ctx.explain(a.key)} oncontextmenu={hold(a.key)}>{a.name}</button>
        <button class="num rollbox" onclick={() => ctx.roll({ type: "ability", key: a.key })} oncontextmenu={hold(`mod.${a.key}`)}>{a.modifier}</button>
        <button
          class="num rollbox save"
          class:prof={a.proficient}
          onclick={() => ctx.roll({ type: "save", key: a.key })}
          oncontextmenu={hold(`save.${a.key}`)}>{a.save}{a.proficient ? " ●" : ""}</button
        >
      {/each}
    </div>
    {#each o.casting as c (c.name)}
      <div class="cast"><span class="muted">{c.name}</span> <span class="dim">DC</span> <b>{c.dc}</b> <span class="dim">attack</span> <b>{c.attack}</b></div>
    {/each}
  </Pane>

  <Pane title="Skills" class="skills" fold="sheet.skills" footer="hold one: how it adds up">
    {#each o.skills as s (s.key)}
      <button class="skill" onclick={() => ctx.roll({ type: "skill", key: s.key })} oncontextmenu={hold(`skill.${s.key}`)}>
        <span class="mark" class:on={s.proficiency !== "none"}>{MARK[s.proficiency]}</span>
        <span class="sname">{s.name} <span class="dim">{s.ability.toUpperCase()}</span></span>
        <b class="rollbox">{s.bonus}</b>
      </button>
    {/each}
  </Pane>
</div>

<style>
  .skills-tab {
    display: grid;
    grid-template-columns: minmax(0, 46ch) minmax(0, 46ch);
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
