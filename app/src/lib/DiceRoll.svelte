<!-- A roll typed by hand: dice and the character's numbers, by tapping or typing. -->
<script lang="ts">
  import { onMount } from "svelte";
  import Modal from "./Modal.svelte";

  let {
    vars,
    value = "",
    error = "",
    onroll,
    onclose,
  }: {
    /// The names a roll can use, with their values.
    vars: [string, number][];
    value?: string;
    /// Why the last try did not work.
    error?: string;
    onroll: (text: string) => void;
    onclose: () => void;
  } = $props();

  const DICE = [4, 6, 8, 10, 12, 20, 100];

  let text = $state("");
  let input: HTMLInputElement | undefined = $state();
  onMount(() => {
    text = value;
    input?.focus();
    input?.select();
  });

  /// Add a term: a second d6 makes `2d6`, anything else is added on.
  function add(term: string) {
    const die = /^d(\d+)$/.exec(term);
    const last = /(\d*)d(\d+)$/.exec(text.trimEnd());
    if (die && last && last[2] === die[1]) {
      text = text.trimEnd().slice(0, last.index) + `${Number(last[1] || 1) + 1}d${die[1]}`;
    } else if (!text.trim() || /[+\-*/(]\s*$/.test(text)) {
      text = `${text}${term}`;
    } else {
      text = `${text.trimEnd()} + ${term}`;
    }
    input?.focus();
  }

  const signed = (k: string, v: number) => (k === "level" ? `${v}` : v < 0 ? `${v}` : `+${v}`);
</script>

<Modal title="Roll dice" {onclose} footer="enter · esc">
  <form
    onsubmit={(e) => {
      e.preventDefault();
      onroll(text);
    }}
  >
    <label class="line">
      <span>❯</span>
      <input bind:this={input} bind:value={text} placeholder="d20 + prof + dex" spellcheck="false" autocomplete="off" autocapitalize="off" />
      {#if text}<button type="button" class="ghost clear" aria-label="Clear" onclick={() => ((text = ""), input?.focus())}>✕</button>{/if}
    </label>
    <div class="chips">
      {#each DICE as d (d)}<button type="button" class="chip" onclick={() => add(`d${d}`)}>d{d}</button>{/each}
    </div>
    <div class="chips">
      {#each vars as [k, v] (k)}
        <button type="button" class="chip" onclick={() => add(k)}>{k} <span class="dim">{signed(k, v)}</span></button>
      {/each}
    </div>
    {#if error}<p class="err">{error}</p>{/if}
    <div class="actions">
      <button type="button" class="ghost" onclick={onclose}>Cancel</button>
      <button type="submit" class="primary" disabled={!text.trim()}>Roll</button>
    </div>
  </form>
</Modal>

<style>
  .err {
    color: var(--red);
    margin: 0.6em 0 0;
  }
  .line {
    display: flex;
    gap: 1ch;
    align-items: center;
    border-bottom: 1px solid var(--border);
    padding: 0.3em 0;
  }
  .line span {
    color: var(--accent);
  }
  input {
    flex: 1;
    background: none;
    border: 0;
    outline: none;
    min-width: 0;
    padding: 0.2em 0;
  }
  .clear {
    padding: 0 0.5ch;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5em 1ch;
    margin-top: 0.8em;
  }
  .chip {
    background: none;
    border: 1px solid var(--border);
    border-radius: 14px;
    padding: 0.1em 1.5ch;
  }
  .chip:hover {
    border-color: var(--accent);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 1.5ch;
    margin-top: 1em;
  }
  @media (pointer: coarse) {
    .line {
      min-height: 48px;
      font-size: 1.1em;
    }
    .chips .chip {
      min-height: 40px;
      min-width: 5ch;
    }
    .actions button {
      flex: 1;
    }
  }
</style>
