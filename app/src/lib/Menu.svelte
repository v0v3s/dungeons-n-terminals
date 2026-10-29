<!-- A ⋯ button for the actions not needed often, listed in a dialog
     titled `title` when it is pressed. -->
<script lang="ts">
  import Modal from "./Modal.svelte";

  let { title, actions }: { title: string; actions: [label: string, run: () => void, danger?: boolean][] } = $props();

  let open = $state(false);
</script>

<button class="ghost icon more" onclick={() => (open = true)} aria-label="More">⋯</button>

{#if open}
  <Modal {title} onclose={() => (open = false)}>
    <div class="menu">
      {#each actions as [label, run, danger] (label)}
        <button
          class:danger
          onclick={() => {
            open = false;
            run();
          }}>{label}</button
        >
      {/each}
    </div>
  </Modal>
{/if}

<style>
  .more {
    flex: none;
    padding: 0 1ch;
  }
  .menu {
    display: flex;
    flex-direction: column;
  }
  .menu button {
    background: none;
    border: 0;
    text-align: left;
    padding: 0.3em 1ch;
    cursor: pointer;
  }
  .menu button:hover {
    background: var(--selected);
  }
  .menu button.danger {
    color: var(--red);
  }
  @media (pointer: coarse) {
    .more {
      min-width: 44px;
      min-height: 40px;
    }
    .menu button {
      min-height: 48px;
      border-top: 1px solid rgb(65 72 104 / 0.3);
    }
  }
</style>
