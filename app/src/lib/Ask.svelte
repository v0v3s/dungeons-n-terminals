<!-- A flow's question (dnt_core::flow): pick from a list, type a number,
     set ability scores (the method, then each score up or down), or
     confirm. -->
<script lang="ts">
  import type { Answer, Choice, Method, Question } from "./api";
  import Picker from "./Picker.svelte";

  let {
    question,
    onanswer,
    onconfirm,
    confirm = "Apply",
    onhighlight,
    onscores,
    oninfo,
  }: {
    question: Question;
    onanswer: (a: Answer) => void;
    onconfirm?: () => void;
    confirm?: string;
    onhighlight?: (value: string) => void;
    onscores?: (scores: number[]) => void;
    /// Read about a choice (a ? on each row).
    oninfo?: (c: Choice) => void;
  } = $props();

  const NAMES = ["Strength", "Dexterity", "Constitution", "Intelligence", "Wisdom", "Charisma"];
  /// What each ability is for, under its name.
  const USES = [
    "melee, Athletics",
    "AC, initiative",
    "hit points",
    "Arcana, History",
    "Perception, Insight",
    "Persuasion, Deception",
  ];
  const METHODS: [Method, string][] = [
    ["standard_array", "Standard array"],
    ["point_buy", "Point buy"],
    ["manual", "Manual"],
  ];
  const ARRAY = [15, 14, 13, 12, 10, 8];
  const cost = (s: number) => ({ 9: 1, 10: 2, 11: 3, 12: 4, 13: 5, 14: 7, 15: 9 })[s] ?? 0;

  let number = $state("");
  let scores: number[] = $state([]);
  let keyed = $state("");

  // A new abilities question starts from its scores.
  $effect(() => {
    if (question.type === "abilities") {
      const key = JSON.stringify(question.scores);
      if (key !== keyed) {
        keyed = key;
        scores = [...question.scores.scores];
      }
    }
  });
  $effect(() => onscores?.(scores));

  const method = $derived(question.type === "abilities" ? question.scores.method : "manual");
  const left = $derived(27 - scores.reduce((n, s) => n + cost(s), 0));

  function adjust(i: number, up: boolean) {
    const next = [...scores];
    if (method === "standard_array") {
      const sorted = [...ARRAY].sort((a, b) => a - b);
      const at = sorted.indexOf(next[i]);
      const target = sorted[at + (up ? 1 : -1)];
      const j = next.indexOf(target);
      if (target === undefined || j < 0) return;
      [next[i], next[j]] = [next[j], next[i]];
    } else if (method === "point_buy") {
      const v = next[i] + (up ? 1 : -1);
      if (v < 8 || v > 15) return;
      next[i] = v;
      if (27 - next.reduce((n, s) => n + cost(s), 0) < 0) return;
    } else {
      next[i] = Math.max(1, Math.min(30, next[i] + (up ? 1 : -1)));
    }
    scores = next;
  }
  const mod = (s: number) => {
    const m = Math.floor((s - 10) / 2);
    return m >= 0 ? `+${m}` : `${m}`;
  };
</script>

{#if question.type === "pick"}
  <div class="title muted">{question.title}</div>
  {#key question.title + question.choices.length}
    <Picker choices={question.choices} count={question.count} onpick={(v) => onanswer({ type: "pick", value: v })} {onhighlight} {oninfo} />
  {/key}
{:else if question.type === "number"}
  <div class="title muted">{question.title}</div>
  <form
    class="number"
    onsubmit={(e) => {
      e.preventDefault();
      const n = parseInt(number, 10);
      if (!Number.isNaN(n)) onanswer({ type: "number", value: n });
    }}
  >
    <span>❯</span>
    <input bind:value={number} inputmode="numeric" placeholder="the roll" />
    <button class="primary" type="submit">OK</button>
  </form>
{:else if question.type === "abilities"}
  <div class="methods" role="group" aria-label="Method">
    {#each METHODS as [m, label] (m)}
      <button class="method" aria-pressed={m === method} onclick={() => m !== method && onanswer({ type: "method", value: m })}>{label}</button>
    {/each}
  </div>
  <div class="title muted">
    {#if method === "point_buy"}
      <span class="budget" class:bad={left < 0}><b>{left}</b> / 27 points left</span>
    {:else if method === "standard_array"}Raising a score swaps it with the next value{:else}Set each score: rolled at the table, or anything else{/if}
  </div>
  <div class="scores">
    {#each NAMES as name, i (name)}
      <div class="score">
        <span class="name">{name}<span class="uses dim">{USES[i]}</span></span>
        <button class="step" aria-label="Lower" onclick={() => adjust(i, false)}>−</button>
        <b class="value">{scores[i]}</b>
        <button class="step" aria-label="Raise" onclick={() => adjust(i, true)}>+</button>
        <span class="mod">{mod(scores[i] ?? 10)}</span>
        {#if method === "point_buy"}<span class="dim cost">cost {cost(scores[i])}</span>{/if}
      </div>
    {/each}
  </div>
  <div class="dim later">Species and background increases come next.</div>
  <div class="actions"><button class="primary" onclick={() => onanswer({ type: "scores", value: scores })}>Next</button></div>
{:else if question.type === "confirm"}
  <div class="actions"><button class="primary" onclick={() => onconfirm?.()}>{confirm}</button></div>
{/if}

<style>
  .title {
    margin-bottom: 0.5em;
  }
  .number {
    display: flex;
    gap: 1ch;
    align-items: center;
  }
  .number span {
    color: var(--accent);
  }
  .number input {
    flex: 1;
    background: none;
    border: 0;
    border-bottom: 1px solid var(--border);
    outline: none;
    padding: 0.3em 0;
  }
  .scores {
    display: flex;
    flex-direction: column;
    gap: 0.2em;
  }
  .score {
    display: grid;
    grid-template-columns: minmax(14ch, 1fr) 3ch 3ch 3ch 4ch 7ch;
    align-items: center;
    gap: 0.5ch;
  }
  .name {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .uses {
    font-size: 0.8em;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .methods {
    display: flex;
    gap: 1ch;
    margin-bottom: 0.75em;
  }
  .method {
    flex: 1;
    background: none;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    color: var(--gray);
    padding: 0.3em 1ch;
    cursor: pointer;
  }
  .method[aria-pressed="true"] {
    border-color: var(--accent);
    color: var(--accent);
    font-weight: 700;
  }
  .budget {
    display: inline-block;
    border: 1px solid var(--accent);
    border-radius: var(--radius);
    padding: 0.2em 1.5ch;
    color: var(--fg);
  }
  .budget b {
    color: var(--accent);
    font-size: 1.3em;
  }
  .budget.bad,
  .budget.bad b {
    border-color: var(--red);
    color: var(--red);
  }
  .later {
    margin-top: 0.75em;
  }
  .value {
    text-align: center;
  }
  .mod {
    color: var(--accent);
    font-weight: 700;
    text-align: right;
  }
  .step {
    background: none;
    border: 1px solid var(--border);
    border-radius: 3px;
    color: var(--accent);
    cursor: pointer;
    padding: 0;
  }
  .bad {
    color: var(--red);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    margin-top: 1em;
  }
  @media (pointer: coarse) {
    .score {
      grid-template-columns: minmax(0, 1fr) 44px 4ch 44px 4ch;
      min-height: 52px;
    }
    .method {
      min-height: 44px;
    }
    .cost {
      display: none;
    }
    .step {
      height: 44px;
      font-size: 1.2em;
    }
    .actions button {
      flex: 1;
    }
  }
</style>
