<!-- Writing a feature's rules (TOML), checked as you type against the open
     character: problems, or what the rules change on the sheet. -->
<script lang="ts">
  import { onMount, untrack } from "svelte";
  import * as api from "./api";
  import Modal from "./Modal.svelte";
  import { working } from "./working.svelte";

  let {
    entity,
    character,
    onsaved,
    onclose,
  }: { entity: number; character: string; onsaved: (s: api.Sheet, message: string) => void; onclose: () => void } = $props();

  let draft = $state<api.RulesDraft | null>(null);
  let text = $state("");
  let checked = $state<api.Checked | null>(null);
  let error = $state("");
  let timer: ReturnType<typeof setTimeout> | undefined;

  onMount(async () => {
    try {
      draft = await api.rulesOpen(untrack(() => entity));
      text = draft.text;
      checked = draft.checked;
    } catch (e) {
      error = String(e);
    }
  });

  function edited() {
    clearTimeout(timer);
    timer = setTimeout(async () => {
      try {
        checked = await api.rulesCheck(entity, text);
      } catch (e) {
        error = String(e);
      }
    }, 300);
  }

  async function save(t: string, message: string) {
    try {
      error = "";
      onsaved(await working("Saving the rules…", () => api.rulesSave(entity, t)), message);
    } catch (e) {
      error = String(e);
    }
  }

  const ORIGIN: Record<api.RulesDraft["origin"], string> = {
    user: "Your rules for this feature.",
    builtin: "dnt's rules for this feature; saving makes your own version, which replaces them.",
    new: "Nothing automates this feature yet: a template to start from.",
  };
  const unchanged = $derived(!!draft && text.trim() === draft.text.trim());
</script>

<Modal title="Rules · {draft?.name ?? ''}" {onclose} wide>
  {#if draft}
    <p class="dim">{ORIGIN[draft.origin]}</p>
    <textarea bind:value={text} oninput={edited} spellcheck="false" autocapitalize="off" autocomplete="off" rows="14"></textarea>
    <p class="ref dim">
      Targets: <code>ac</code> <code>speed</code> <code>hp.max</code> <code>initiative</code> <code>save.dex</code>
      <code>skill.stealth</code> <code>attack.bonus</code> <code>attack.melee.damage</code> <code>rider.&lt;key&gt;</code>
      <code>resource.&lt;key&gt;.max</code> · effects: <code>add</code> <code>set</code> <code>max</code> <code>min</code>
      <code>mul</code>, with an optional <code>when</code>.
    </p>
    <div class="check">
      {#if checked?.errors.length}
        {#each checked.errors as e (e)}<div class="bad">✗ {e}</div>{/each}
      {:else if checked?.changes.length}
        <div class="dim">On {character}:</div>
        {#each checked.changes as c (c)}<div class="good">{c}</div>{/each}
      {:else if checked}
        <div class="dim">✓ Valid; changes nothing on {character}.</div>
      {/if}
    </div>
  {/if}
  {#if error}<p class="bad">{error}</p>{/if}
  <div class="buttons">
    {#if draft?.origin === "user"}
      <button class="ghost" onclick={() => save("", "Removed your rules")}>{"Remove mine"}</button>
    {/if}
    <span class="gap"></span>
    <button class="ghost" onclick={onclose}>Cancel</button>
    <button class="primary" disabled={unchanged || !!checked?.errors.length} onclick={() => save(text, "Saved your rules")}>Save</button>
  </div>
</Modal>

<style>
  p {
    margin: 0 0 0.6em;
  }
  textarea {
    width: 100%;
    box-sizing: border-box;
    min-height: 14em;
    background: var(--black);
    color: var(--fg);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 0.6em 1ch;
    font: inherit;
    resize: vertical;
  }
  .ref {
    font-size: 0.9em;
    margin-top: 0.5em;
  }
  code {
    color: var(--code);
  }
  .check {
    margin-top: 0.4em;
    max-height: 9em;
    overflow-y: auto;
  }
  .bad {
    color: var(--red);
  }
  .good {
    color: var(--green);
  }
  .buttons {
    display: flex;
    flex-wrap: wrap;
    gap: 1ch;
    margin-top: 1em;
  }
  .gap {
    flex: 1;
  }
  @media (pointer: coarse) {
    .buttons button {
      flex: 1;
      min-height: 44px;
    }
    .gap {
      display: none;
    }
  }
</style>
