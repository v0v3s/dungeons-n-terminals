<!-- What kind of container a line is: as the item says, a limit in
     pounds, no limit, contents that weigh nothing (a Bag of Holding), or
     not a container. The core reads these as the settings text
     (equipment::set_container). -->
<script lang="ts">
  import Modal from "./Modal.svelte";

  let { name, current, onset, onclose }: { name: string; current: string | null; onset: (settings: string) => void; onclose: () => void } = $props();

  type Kind = "item" | "limit" | "nolimit" | "weightless" | "weightless-nolimit" | "off";
  const KINDS: [Kind, string][] = [
    ["item", "As the item says"],
    ["limit", "Holds up to a weight"],
    ["nolimit", "Holds anything (no weight limit)"],
    ["weightless", "Contents weigh nothing, up to a weight (a Bag of Holding)"],
    ["weightless-nolimit", "Contents weigh nothing, no limit"],
    ["off", "Not a container"],
  ];

  /// The kind and pounds of the line's settings now (`30`, `no limit
  /// weightless`, or nothing: not a container yet).
  function read(c: string | null): [Kind, string] {
    const t = (c ?? "").trim();
    if (!t) return ["limit", "30"];
    const light = t.includes("weightless");
    const pounds = t.match(/\d+(\.\d+)?/)?.[0];
    if (t.includes("no limit")) return [light ? "weightless-nolimit" : "nolimit", "30"];
    return [light ? "weightless" : "limit", pounds ?? "30"];
  }
  /// Starts from the settings it opened with (the dialog is new each time).
  const start = (() => read(current))();
  let kind: Kind = $state(start[0]);
  let pounds = $state(start[1]);
  let error = $state("");
  const weighed = $derived(kind === "limit" || kind === "weightless");

  function submit(e: SubmitEvent) {
    e.preventDefault();
    const n = Number(String(pounds).replace(",", "."));
    if (weighed && (!Number.isFinite(n) || n <= 0)) return void (error = "The weight it holds: pounds, more than 0.");
    const settings = { item: "", limit: `${n}`, nolimit: "no limit", weightless: `${n} weightless`, "weightless-nolimit": "no limit weightless", off: "off" }[kind];
    onset(settings);
  }
</script>

<Modal title="{name}: container" {onclose} footer="enter · esc">
  <form onsubmit={submit}>
    <label class="field">
      <span>Kind</span>
      <select bind:value={kind}>
        {#each KINDS as [k, label] (k)}<option value={k}>{label}</option>{/each}
      </select>
    </label>
    {#if weighed}
      <label class="field">
        <span>Holds up to (lb)</span>
        <input type="number" min="0.1" step="0.1" bind:value={pounds} />
      </label>
    {/if}
    {#if error}<p class="err">{error}</p>{/if}
    <div class="actions">
      <button type="button" class="ghost" onclick={onclose}>Cancel</button>
      <button type="submit" class="primary">Set</button>
    </div>
  </form>
</Modal>

<style>
  form {
    display: flex;
    flex-direction: column;
    gap: 0.8em;
  }
  .err {
    margin: 0;
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
  select,
  input {
    min-width: 0;
    max-width: 100%;
  }
  input {
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    padding: 0.4em 0.6em;
    font: inherit;
    color: var(--fg);
  }
  input:focus {
    outline: none;
    border-color: var(--accent);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 1.5ch;
  }
  @media (pointer: coarse) {
    select,
    input {
      min-height: 44px;
    }
    .actions button {
      flex: 1;
    }
  }
</style>
