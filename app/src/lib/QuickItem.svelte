<!-- A quick item: one only this character has (not in the compendium or
     the campaign's homebrew), with its name, what it is, how many and its
     weight. -->
<script lang="ts">
  import { onMount } from "svelte";
  import Modal from "./Modal.svelte";

  type Quick = { name: string; description: string | null; quantity: number; weight: number | null };

  let { into = null, onadd, onclose }: { into?: string | null; onadd: (item: Quick) => void; onclose: () => void } = $props();

  let name = $state("");
  let description = $state("");
  let quantity = $state("1");
  let weight = $state("");
  let error = $state("");
  let first: HTMLInputElement | undefined = $state();
  onMount(() => first?.focus());

  function submit(e: SubmitEvent) {
    e.preventDefault();
    // A number field's value may come as a number.
    const q = Number(String(quantity).trim() || "1");
    const ws = String(weight ?? "").trim();
    const w = ws ? Number(ws.replace(",", ".")) : null;
    if (!name.trim()) return void (error = "It needs a name.");
    if (!Number.isInteger(q) || q < 1) return void (error = "How many: a whole number, 1 or more.");
    if (w !== null && (!Number.isFinite(w) || w < 0)) return void (error = "The weight: pounds, 0 or more.");
    onadd({ name: name.trim(), description: description.trim() || null, quantity: q, weight: w });
  }
</script>

<Modal title={into ? `A quick item, into ${into}` : "A quick item"} {onclose} footer="enter · esc">
  <form onsubmit={submit}>
    <p class="dim lead">An item only this character has: it is not added to the compendium or the campaign's homebrew.</p>
    <label class="field">
      <span>Name</span>
      <input bind:this={first} bind:value={name} spellcheck="false" autocomplete="off" />
    </label>
    <label class="field">
      <span>Description</span>
      <textarea bind:value={description} rows="3" placeholder="What it is, in a few words (optional)"></textarea>
    </label>
    <div class="pair">
      <label class="field">
        <span>How many</span>
        <input type="number" min="1" step="1" bind:value={quantity} />
      </label>
      <label class="field">
        <span>Weight, each (lb)</span>
        <input type="number" min="0" step="0.1" bind:value={weight} placeholder="none" />
      </label>
    </div>
    {#if error}<p class="err">{error}</p>{/if}
    <div class="actions">
      <button type="button" class="ghost" onclick={onclose}>Cancel</button>
      <button type="submit" class="primary">Add the item</button>
    </div>
  </form>
</Modal>

<style>
  form {
    display: flex;
    flex-direction: column;
    gap: 0.8em;
  }
  .lead,
  .err {
    margin: 0;
  }
  .err {
    color: var(--red);
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 0.25em;
    min-width: 0;
  }
  .field > span {
    font-size: 0.85em;
    color: var(--gray);
    white-space: nowrap;
  }
  input,
  textarea {
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    padding: 0.4em 0.6em;
    font: inherit;
    color: var(--fg);
    min-width: 0;
  }
  textarea {
    resize: vertical;
  }
  input:focus,
  textarea:focus {
    outline: none;
    border-color: var(--accent);
  }
  .pair {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 1em;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 1.5ch;
  }
  @media (pointer: coarse) {
    input,
    textarea {
      min-height: 44px;
    }
    .actions button {
      flex: 1;
    }
  }
</style>
