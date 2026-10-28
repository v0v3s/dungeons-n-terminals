<!-- Creating a character: a checklist of the steps with what was chosen
     in each (tap a finished one to change it), the question, and a preview
     of what is highlighted (or of the character so far). On a phone the
     checklist folds above the question, and a ? on a row shows what the
     preview would. -->
<script lang="ts">
  import { MediaQuery } from "svelte/reactivity";
  import * as api from "./api";
  import Ask from "./Ask.svelte";
  import Blocks from "./Blocks.svelte";
  import Modal from "./Modal.svelte";
  import Pane from "./Pane.svelte";

  /// One pane at a time: no steps or preview beside the question.
  const phone = new MediaQuery("(max-width: 1000px)");

  let { step = $bindable(), ondone, oncancel }: { step: api.CreationStep; ondone: (s: api.Sheet) => void; oncancel: () => void } = $props();

  let error = $state("");
  let preview: api.Preview | null = $state(null);
  let highlighted: string | null = $state(null);
  let scores: number[] | null = $state(null);
  let asked = 0;
  /// The checklist, unfolded on a phone.
  let open = $state(false);
  /// A choice being read about (on a phone), with its preview once loaded.
  let about: { choice: api.Choice; preview: api.Preview | null } | null = $state(null);

  const at = $derived(step.steps.findIndex((s) => s.current));
  const review = $derived(step.question.type === "confirm");

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
      open = false;
    } catch (e) {
      error = String(e);
    }
  }

  /// Choose again from a finished step; what came after is undone.
  async function reopen(i: number) {
    try {
      error = "";
      highlighted = null;
      open = false;
      step = await api.creationReopen(i);
    } catch (e) {
      error = String(e);
    }
  }

  async function info(choice: api.Choice) {
    about = { choice, preview: null };
    try {
      const preview = await api.creationPreview(choice.value, null);
      if (about?.choice === choice) about = { choice, preview };
    } catch {
      // The hint is still there to read.
    }
  }

  function choose(value: string) {
    about = null;
    answer({ type: "pick", value: [value] });
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

{#snippet checklist()}
  <ol class="checklist">
    {#each step.steps as s, i (s.label)}
      <li class:done={s.done} class:current={s.current}>
        {#if s.reopens && !s.current}
          <button class="srow" onclick={() => reopen(i)}>
            <span class="mark">✓</span><span class="slabel">{s.label}</span><span class="change">change</span>
          </button>
        {:else}
          <div class="srow"><span class="mark">{s.current ? "▸" : s.done ? "✓" : " "}</span><span class="slabel">{s.label}</span></div>
        {/if}
        {#each s.chosen as [k, v], j (j)}<div class="chosen"><span class="dim">{k}</span> {v}</div>{/each}
      </li>
    {/each}
  </ol>
{/snippet}

<div class="creation">
  <Pane title="New character · {step.name}" class="steps">
    {@render checklist()}
  </Pane>

  <Pane title={review ? "Review" : "Choose"} focused class="question">
    <div class="ask" class:unfolded={phone.current && (open || review)}>
      <!-- On a phone the steps fold above the question. -->
      {#if !review}
        <button class="phone where" aria-expanded={open} onclick={() => (open = !open)}>
          <span class="acc">▸ {step.steps[at]?.label}</span>
          <span class="dim">{at + 1}/{step.steps.length}</span>
          <span class="fold">{open ? "▾" : "▸"} steps</span>
        </button>
      {/if}
      {#if review}
        <p class="muted">Everything is chosen. Tap a step to change it; the character is saved to the campaign.</p>
        {#if preview && phone.current}
          <div class="phone stats">
            {#each preview.stats as [k, v] (k)}<span class="box"><span class="dim">{k}</span> <b>{v}</b></span>{/each}
          </div>
        {/if}
      {/if}
      {#if phone.current && (open || review)}{@render checklist()}{/if}
      {#key JSON.stringify(step.steps)}
        <Ask
          question={step.question}
          onanswer={answer}
          onconfirm={finish}
          confirm="Create the character"
          onhighlight={(v) => (highlighted = v)}
          onscores={(s) => (scores = s)}
          oninfo={step.details && phone.current ? info : undefined}
        />
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

{#if about}
  {@const a = about}
  <Modal title={a.choice.label} onclose={() => (about = null)}>
    <div class="about">
      {#if a.preview?.equipment}
        <ul>{#each a.preview.equipment[1] as e (e)}<li>{e}</li>{/each}</ul>
      {:else if a.preview?.entity}
        <Blocks blocks={a.preview.entity.blocks} onlink={() => {}} />
      {:else}
        <p class="muted">{a.choice.hint}</p>
      {/if}
    </div>
    {#if step.question.type === "pick" && step.question.count === 1}
      <button class="primary choose" onclick={() => choose(a.choice.value)}>Choose {a.choice.label}</button>
    {/if}
  </Modal>
{/if}

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
  .checklist {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .checklist li {
    color: var(--dim);
  }
  .checklist li + li {
    margin-top: 0.2em;
  }
  .srow {
    display: flex;
    gap: 1ch;
    align-items: baseline;
    width: 100%;
    background: none;
    border: 0;
    padding: 0;
    color: inherit;
    text-align: left;
  }
  button.srow {
    cursor: pointer;
  }
  .mark {
    white-space: pre;
    width: 1ch;
  }
  .slabel {
    flex: 1;
  }
  .done .srow {
    color: var(--fg);
  }
  .done .mark {
    color: var(--accent);
  }
  .current .srow {
    color: var(--accent);
    font-weight: 700;
  }
  .change {
    color: var(--cyan);
    font-size: 0.85em;
  }
  button.srow:hover .slabel {
    color: var(--accent);
  }
  .chosen {
    padding-left: 2ch;
    color: var(--gray);
    font-size: 0.9em;
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
  .where {
    width: 100%;
    gap: 1ch;
    align-items: baseline;
    background: none;
    border: 0;
    border-bottom: 1px solid var(--border);
    padding: 0 0 0.4em;
    margin-bottom: 0.6em;
    text-align: left;
    cursor: pointer;
  }
  .where > span {
    white-space: nowrap;
  }
  .fold {
    margin-left: auto;
    color: var(--cyan);
  }
  .stats.phone {
    flex-wrap: wrap;
    gap: 0.5em 1ch;
    margin: 0 0 1em;
  }
  .stats .box {
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 0.2em 1ch;
  }
  .ask > .checklist {
    margin-bottom: 1em;
  }
  /* With the checklist above it, the list scrolls with the page instead
     of shrinking to nothing. */
  .ask.unfolded :global(ul) {
    flex: none;
    overflow: visible;
  }
  .ask > .checklist li {
    padding: 0.3em 0;
    border-bottom: 1px solid rgb(65 72 104 / 0.3);
  }
  .about {
    min-height: 0;
    overflow-y: auto;
  }
  .choose {
    margin-top: 1em;
    min-height: 44px;
  }
  @media (max-width: 1000px) {
    .phone {
      display: flex;
    }
    .where {
      min-height: 40px;
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
