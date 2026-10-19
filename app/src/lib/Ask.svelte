<!-- A flow's question (dnt_core::flow): pick from a list, type a number,
     set ability scores, or confirm. -->
<script lang="ts">
  import type { Answer, Question } from "./api";
  import Picker from "./Picker.svelte";

  let {
    question,
    onanswer,
    onconfirm,
    confirm = "Apply",
    onhighlight,
    onscores,
  }: {
    question: Question;
    onanswer: (a: Answer) => void;
    onconfirm?: () => void;
    confirm?: string;
    onhighlight?: (value: string) => void;
    onscores?: (scores: number[]) => void;
  } = $props();

  const NAMES = ["Strength", "Dexterity", "Constitution", "Intelligence", "Wisdom", "Charisma"];
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
    <Picker choices={question.choices} count={question.count} onpick={(v) => onanswer({ type: "pick", value: v })} {onhighlight} />
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
  <div class="title muted">
    {#if method === "point_buy"}Points left <b class:bad={left < 0}>{left}/27</b>{:else if method === "standard_array"}Swap values between abilities{:else}Set each score{/if}
  </div>
  <div class="scores">
    {#each NAMES as name, i (name)}
      <div class="score">
        <span class="name">{name}</span>
        <button class="step" aria-label="Lower" onclick={() => adjust(i, false)}>‹</button>
        <b class="value">{scores[i]}</b>
        <button class="step" aria-label="Raise" onclick={() => adjust(i, true)}>›</button>
        <span class="mod">{mod(scores[i] ?? 10)}</span>
        {#if method === "point_buy"}<span class="dim cost">cost {cost(scores[i])}</span>{/if}
      </div>
    {/each}
  </div>
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
    grid-template-columns: 14ch 3ch 3ch 3ch 4ch 1fr;
    align-items: center;
    gap: 0.5ch;
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
      grid-template-columns: 1fr 44px 4ch 44px 4ch;
      min-height: 52px;
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
