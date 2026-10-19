<!-- Creating a character: the steps, the question, and a preview of what
     is highlighted (or of the character so far). -->
<script lang="ts">
  import * as api from "./api";
  import Ask from "./Ask.svelte";
  import Blocks from "./Blocks.svelte";
  import Pane from "./Pane.svelte";

  let { step = $bindable(), ondone, oncancel }: { step: api.CreationStep; ondone: (s: api.Sheet) => void; oncancel: () => void } = $props();

  let error = $state("");
  let preview: api.Preview | null = $state(null);
  let highlighted: string | null = $state(null);
  let scores: number[] | null = $state(null);
  let asked = 0;

  async function refresh() {
    const ask = ++asked;
    try {
      const p = await api.creationPreview(highlighted, step.question.type === "abilities" ? scores : null);
      if (ask === asked) preview = p;
    } catch {
      // A preview is a nicety.
    }
  }

  $effect(() => {
    void step;
    void highlighted;
    void scores;
    refresh();
  });

  async function answer(a: api.Answer) {
    try {
      error = "";
      highlighted = null;
      step = await api.creationAnswer(a);
    } catch (e) {
      error = String(e);
    }
  }

  async function back() {
    const prev = await api.creationBack();
    if (prev) step = prev;
    else oncancel();
  }

  async function finish() {
    try {
      ondone(await api.creationFinish());
    } catch (e) {
      error = String(e);
    }
  }
</script>

<div class="creation">
  <Pane title="New character · {step.name}" class="steps">
    {#each step.progress as [label, done, current] (label)}
      <div class="step" class:done class:current>{current ? "▸" : done ? "✓" : " "} {label}</div>
    {/each}
    {#if step.summary.length}
      <div class="summary">
        {#each step.summary as [k, v], i (i)}<div><span class="dim">{k}</span> {v}</div>{/each}
      </div>
    {/if}
  </Pane>

  <Pane title={step.question.type === "confirm" ? "Review" : "Choose"} focused class="question">
    <div class="ask">
      <!-- On a phone the steps and summary panes are hidden: say where we are. -->
      <div class="phone step-now dim">
        {#each step.progress as [label, , current] (label)}{#if current}<span class="acc">▸ {label}</span>{/if}{/each}
        <span>· step {step.progress.findIndex(([, , c]) => c) + 1} of {step.progress.length}</span>
      </div>
      {#if step.question.type === "confirm"}
        <p class="muted">Everything is chosen. The character is saved to the campaign.</p>
        <div class="phone review">
          {#each step.summary as [k, v], i (i)}<div><span class="dim">{k}</span> {v}</div>{/each}
        </div>
      {/if}
      {#key JSON.stringify(step.progress) + step.summary.length}
        <Ask question={step.question} onanswer={answer} onconfirm={finish} confirm="Create the character" onhighlight={(v) => (highlighted = v)} onscores={(s) => (scores = s)} />
      {/key}
    </div>
    <div class="nav">
      <button class="ghost" onclick={back}>‹ Back</button>
      <button class="ghost" onclick={oncancel}>Cancel</button>
    </div>
    {#if error}<p class="err">{error}</p>{/if}
  </Pane>

  <Pane title="Preview" class="preview">
    {#if preview?.equipment}
      <div class="acc">{preview.equipment[0]}</div>
      <ul>{#each preview.equipment[1] as e (e)}<li>{e}</li>{/each}</ul>
    {:else if preview?.entity}
      <div class="acc b">{preview.entity.name}</div>
      <Blocks blocks={preview.entity.blocks} onlink={() => {}} />
    {:else if preview}
      <div class="b">{step.name}</div>
      <div class="muted">{preview.identity}</div>
      <div class="stats">
        {#each preview.stats as [k, v] (k)}<span class="dim">{k}</span><b>{v}</b>{/each}
      </div>
      <div class="stats">
        {#each preview.abilities as [k, v, m] (k)}<span class="dim">{k}</span><span><b>{v}</b> <span class="acc">{m}</span></span>{/each}
      </div>
      {#each preview.warnings as w (w)}<div class="warn">⚠ {w}</div>{/each}
    {/if}
  </Pane>
</div>

<style>
  .creation {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(24ch, 32ch) minmax(0, 1.3fr) minmax(0, 1fr);
    gap: 0 2ch;
    padding-top: 0.6em;
  }
  .creation > :global(*) {
    min-height: 0;
  }
  .creation :global(.steps .body),
  .creation :global(.preview .body) {
    overflow-y: auto;
  }
  .step {
    white-space: pre;
    color: var(--dim);
  }
  .step.done {
    color: var(--fg);
  }
  .step.current {
    color: var(--accent);
    font-weight: 700;
  }
  .summary {
    margin-top: 1em;
    border-top: 1px solid var(--border);
    padding-top: 0.5em;
  }
  .ask {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
  }
  .nav {
    display: flex;
    gap: 1ch;
    margin-top: 0.75em;
  }
  .err {
    color: var(--red);
  }
  .acc {
    color: var(--accent);
  }
  .b {
    font-weight: 700;
  }
  .stats {
    display: grid;
    grid-template-columns: 14ch 1fr;
    margin-top: 0.75em;
  }
  .warn {
    color: var(--yellow);
  }
  ul {
    margin: 0.5em 0;
    padding-left: 2ch;
  }
  .phone {
    display: none;
  }
  .review {
    margin-bottom: 1em;
  }
  .review div {
    padding: 0.2em 0;
    border-top: 1px solid rgb(65 72 104 / 0.3);
  }
  @media (max-width: 1000px) {
    .phone {
      display: block;
    }
    .step-now {
      margin-bottom: 0.6em;
    }
    .creation {
      grid-template-columns: minmax(0, 1fr);
      grid-template-rows: auto minmax(0, 1fr);
      gap: 1.2em;
      overflow-y: auto;
    }
    .creation :global(.steps),
    .creation :global(.preview) {
      display: none;
    }
    .nav button {
      flex: 1;
    }
  }
</style>
