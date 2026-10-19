<!-- One line of text (or a number), in a modal. -->
<script lang="ts">
  import { onMount } from "svelte";
  import Modal from "./Modal.svelte";

  let {
    title,
    value = "",
    hint = "",
    number = false,
    action = "OK",
    onsubmit,
    onclose,
  }: {
    title: string;
    value?: string;
    hint?: string;
    number?: boolean;
    action?: string;
    onsubmit: (value: string) => void;
    onclose: () => void;
  } = $props();

  let text = $state("");
  let input: HTMLInputElement | undefined = $state();
  onMount(() => {
    text = value;
    input?.focus();
    input?.select();
  });
</script>

<Modal {title} {onclose} footer="enter · esc">
  <form
    onsubmit={(e) => {
      e.preventDefault();
      onsubmit(text);
    }}
  >
    <label class="line">
      <span>❯</span>
      <input bind:this={input} bind:value={text} inputmode={number ? "numeric" : "text"} spellcheck="false" autocomplete="off" />
    </label>
    {#if hint}<p class="dim">{hint}</p>{/if}
    <div class="actions">
      <button type="button" class="ghost" onclick={onclose}>Cancel</button>
      <button type="submit" class="primary">{action}</button>
    </div>
  </form>
</Modal>

<style>
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
  p {
    margin: 0.6em 0 0;
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
    .actions button {
      flex: 1;
    }
  }
</style>
