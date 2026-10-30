<!-- A newer dnt is out (the day's check found it): what is new, and
     updating, until closed. -->
<script lang="ts">
  import { installUpdate, update } from "./update.svelte";
  import Modal from "./Modal.svelte";
  import ReleaseNotes from "./ReleaseNotes.svelte";

  let notes = $state(false);
</script>

{#if update.newer && !update.dismissed}
  {@const v = update.newer}
  <div class="notice" role="status">
    <span class="what"><span class="acc">◆</span> dnt {v.version} is out</span>
    <span class="go">
      <button class="ghost" onclick={() => (notes = true)}>What's new</button>
      <button class="primary" onclick={installUpdate}>Update</button>
      <button class="ghost close" aria-label="Close the notice" onclick={() => (update.dismissed = true)}>×</button>
    </span>
    {#if update.message}<p class="ok">{update.message}</p>{/if}
    {#if update.error}<p class="err">{update.error}</p>{/if}
  </div>
  {#if notes}
    <Modal title="What's new in dnt {v.version}" onclose={() => (notes = false)}>
      <ReleaseNotes text={v.notes || "No notes."} />
      <div><button class="primary" onclick={installUpdate}>Update</button></div>
      {#if update.message}<p class="ok">{update.message}</p>{/if}
      {#if update.error}<p class="err">{update.error}</p>{/if}
    </Modal>
  {/if}
{/if}

<style>
  .notice {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 0.5em 1.5ch;
    border: 1px solid var(--accent);
    border-radius: 3px;
    padding: 0.4em 1ch;
    margin-bottom: 0.6em;
  }
  .acc {
    color: var(--accent);
  }
  .go {
    display: flex;
    gap: 1ch;
  }
  .notice p {
    flex-basis: 100%;
    margin: 0;
  }
  .ok {
    color: var(--green);
  }
  .err {
    color: var(--red);
  }
  @media (pointer: coarse) {
    .go button {
      min-height: 44px;
    }
  }
</style>
