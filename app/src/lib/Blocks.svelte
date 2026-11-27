<!-- An entry's document (dnt_data::doc), drawn as the TUI draws it. -->
<script lang="ts">
  import type { Block, Link } from "./api";
  import Blocks from "./Blocks.svelte";
  import Spans from "./Spans.svelte";

  let { blocks, onlink }: { blocks: Block[]; onlink: (link: Link) => void } = $props();

  const label = (spans: { text: string }[]) => (spans[0]?.text ?? "").replace(/[:\s]+$/, "");
  const text = (spans: { text: string }[]) => spans.map((s) => s.text).join("").trim();
  /// A cell with nothing to say ("", "—", "-").
  const empty = (spans: { text: string }[]) => ["", "—", "-", "–"].includes(text(spans));

  /// Tables too wide for their place, by index: drawn as one block per row
  /// instead, never scrolled sideways (a sideways swipe changes the page).
  let stacked = $state<Record<number, boolean>>({});
  // Another entry's blocks: measured afresh.
  let measured: Block[] | null = null;
  $effect.pre(() => {
    if (measured !== null && blocks !== measured) stacked = {};
    measured = blocks;
  });
  function fits(node: HTMLElement, i: number) {
    const check = () => {
      if (node.scrollWidth > node.clientWidth + 1) stacked = { ...stacked, [i]: true };
    };
    const seen = new ResizeObserver(check);
    seen.observe(node);
    check();
    return { destroy: () => seen.disconnect() };
  }
</script>

{#key blocks}
{#each blocks as b, i (i)}
  {#if b.type === "heading"}
    {#if b.c.level <= 1}
      <h3 class="h1"><Spans spans={b.c.text} {onlink} /></h3>
    {:else}
      <h4 class="h2"><Spans spans={b.c.text} {onlink} /></h4>
    {/if}
  {:else if b.type === "paragraph"}
    <p><Spans spans={b.c} {onlink} /></p>
  {:else if b.type === "list"}
    <ul>
      {#each b.c as item, j (j)}<li><Blocks blocks={item} {onlink} /></li>{/each}
    </ul>
  {:else if b.type === "table" && stacked[i]}
    <!-- Too wide: each row a block, its first cell leading, then each
         column's name and value, wrapping; empty cells left out. -->
    <div class="rowcards">
      {#if b.c.caption}<div class="caption"><Spans spans={b.c.caption} {onlink} /></div>{/if}
      {#each b.c.rows as row, j (j)}
        <div class="rowcard">
          <span class="lead">{#if b.c.header[0] && text(b.c.header[0])}<span class="lbl">{text(b.c.header[0])}</span>{" "}{/if}<Spans spans={row[0] ?? []} {onlink} /></span>
          {#each row.slice(1) as cell, k (k)}
            {#if !empty(cell)}<span class="pair">{#if b.c.header[k + 1]}<span class="lbl">{text(b.c.header[k + 1])}</span>{" "}{/if}<Spans spans={cell} {onlink} /></span>{/if}
          {/each}
        </div>
      {/each}
    </div>
  {:else if b.type === "table"}
    <div class="table" use:fits={i}>
      {#if b.c.caption}<div class="caption"><Spans spans={b.c.caption} {onlink} /></div>{/if}
      <table>
        {#if b.c.header.length}
          <thead><tr>{#each b.c.header as h, j (j)}<th><Spans spans={h} {onlink} /></th>{/each}</tr></thead>
        {/if}
        <tbody>
          {#each b.c.rows as row, j (j)}
            <tr>{#each row as cell, k (k)}<td><Spans spans={cell} {onlink} /></td>{/each}</tr>
          {/each}
        </tbody>
      </table>
    </div>
  {:else if b.type === "inset"}
    <div class="inset">
      {#if b.c.title}<div class="inset-title"><Spans spans={b.c.title} {onlink} /></div>{/if}
      <Blocks blocks={b.c.body} {onlink} />
    </div>
  {:else if b.type === "quote"}
    <blockquote>
      <Blocks blocks={b.c.body} {onlink} />
      {#if b.c.by}<div class="dim">— {b.c.by}</div>{/if}
    </blockquote>
  {:else if b.type === "properties"}
    <dl>
      {#each b.c as p, j (j)}
        <dt>{label(p)}</dt>
        <dd><Spans spans={p.slice(1)} {onlink} /></dd>
      {/each}
    </dl>
  {:else if b.type === "rule"}
    <hr />
  {/if}
{/each}
{/key}

<style>
  p {
    margin: 0 0 1em;
  }
  /* On a computer, running text keeps a comfortable line however wide its
     pane is (the pane still fills the window). */
  @media (pointer: fine) and (min-width: 761px) {
    p,
    li {
      max-width: 75ch;
    }
  }
  .h1 {
    margin: 1em 0 0.25em;
    font-size: 1em;
    color: var(--accent);
    border-left: 3px solid var(--accent);
    padding-left: 0.6ch;
  }
  .h2 {
    margin: 0.75em 0 0.25em;
    font-size: 1em;
  }
  ul {
    margin: 0 0 1em;
    padding: 0;
    list-style: none;
  }
  li {
    display: flex;
    gap: 1ch;
  }
  li::before {
    content: "•";
    color: var(--accent);
  }
  li > :global(*) {
    flex: 1;
  }
  li :global(p) {
    margin: 0;
  }
  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    column-gap: 2ch;
    margin: 0 0 0.5em;
  }
  dt {
    color: var(--dim);
  }
  dd {
    margin: 0;
  }
  hr {
    border: 0;
    border-top: 1px solid var(--dark);
    margin: 0.75em 0;
  }
  .inset {
    border: 1px solid var(--blue);
    border-radius: var(--radius);
    padding: 0.75em 1.5ch 0;
    margin: 0.5em 0 1em;
    position: relative;
  }
  .inset-title {
    position: absolute;
    top: -0.75em;
    left: 1ch;
    padding: 0 1ch;
    background: var(--bg);
    font-weight: 700;
  }
  blockquote {
    margin: 0 0 1em;
    padding-left: 1.5ch;
    border-left: 2px solid var(--dark);
    font-style: italic;
  }
  /* A table keeps each header and cell on one line (the exception to text
     wrapping): it scrolls sideways in its box instead, the bar in sight. */
  .table {
    max-width: 100%;
    overflow-x: auto;
    margin: 0 0 1em;
    padding-bottom: 0.3em;
    scrollbar-width: thin;
    scrollbar-color: var(--dark) transparent;
  }
  .table::-webkit-scrollbar {
    height: 9px;
  }
  .table::-webkit-scrollbar-thumb {
    background: var(--dark);
    border: 0;
    border-top: 2px solid transparent;
    background-clip: padding-box;
    border-radius: 4px;
  }
  .rowcards {
    margin: 0 0 1em;
  }
  .rowcard {
    display: flex;
    flex-wrap: wrap;
    gap: 0.1em 2ch;
    padding: 0.35em 0;
    border-bottom: 1px solid var(--raised);
  }
  .rowcard .lead {
    flex-basis: 100%;
    color: var(--accent);
    font-weight: 700;
  }
  .rowcard .lbl {
    color: var(--gray);
    font-size: 0.85em;
    font-weight: 400;
  }
  .caption {
    font-weight: 700;
    margin-bottom: 0.25em;
  }
  table {
    border-collapse: collapse;
  }
  th {
    color: var(--accent);
    text-align: left;
    font-weight: 700;
  }
  th,
  td {
    padding: 0 2ch 0 0;
    vertical-align: top;
    white-space: nowrap;
    overflow-wrap: normal;
  }
  /* Phones: tables scroll sideways rather than squeezing every column. */
  @media (max-width: 760px) {
    th,
    td {
      min-width: 8ch;
      padding: 0.15em 1.5ch 0.15em 0;
    }
    tbody tr:nth-child(odd) {
      background: rgb(65 72 104 / 0.18);
    }
  }
</style>
