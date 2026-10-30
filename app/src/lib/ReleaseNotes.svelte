<!-- A release's notes (Markdown as the changelog writes it): headings,
     list items with their wrapped lines, and **bold**, as elements. -->
<script lang="ts">
  let { text }: { text: string } = $props();

  type Block = { kind: "h" | "li" | "p"; text: string };

  const blocks = $derived.by(() => {
    const out: Block[] = [];
    for (const raw of text.split("\n")) {
      const line = raw.trimEnd();
      if (!line.trim()) continue;
      const h = line.match(/^#+\s+(.*)$/);
      const li = line.match(/^\s*[-*]\s+(.*)$/);
      if (h) out.push({ kind: "h", text: h[1] });
      else if (li) out.push({ kind: "li", text: li[1] });
      // A wrapped line goes on with the item or paragraph before it.
      else if (out.length && out[out.length - 1].kind !== "h") out[out.length - 1].text += " " + line.trim();
      else out.push({ kind: "p", text: line.trim() });
    }
    return out;
  });

  /// `**bold**` and `*emphasis*` as runs.
  const runs = (s: string) => s.split(/(\*\*[^*]+\*\*|\*[^*]+\*)/).filter(Boolean);
</script>

<div class="notes">
  {#each blocks as b, i (i)}
    {#snippet inline(s: string)}
      {#each runs(s) as r, j (j)}
        {#if r.startsWith("**")}<b>{r.slice(2, -2)}</b>{:else if r.startsWith("*") && r.length > 2}<i>{r.slice(1, -1)}</i>{:else}{r}{/if}
      {/each}
    {/snippet}
    {#if b.kind === "h"}<p class="h">{@render inline(b.text)}</p>
    {:else if b.kind === "li"}<p class="li"><span class="dot">•</span><span>{@render inline(b.text)}</span></p>
    {:else}<p>{@render inline(b.text)}</p>{/if}
  {/each}
</div>

<style>
  .notes p {
    margin: 0 0 0.4em;
  }
  .h {
    color: var(--accent);
    font-weight: 700;
    margin-top: 0.6em !important;
  }
  .li {
    display: flex;
    gap: 1ch;
  }
  .dot {
    color: var(--dim);
  }
</style>
