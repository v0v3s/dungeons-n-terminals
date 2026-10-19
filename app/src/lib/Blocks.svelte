<!-- An entry's document (dnt_data::doc), drawn as the TUI draws it. -->
<script lang="ts">
  import type { Block, Link } from "./api";
  import Blocks from "./Blocks.svelte";
  import Spans from "./Spans.svelte";

  let { blocks, onlink }: { blocks: Block[]; onlink: (link: Link) => void } = $props();

  const label = (spans: { text: string }[]) => (spans[0]?.text ?? "").replace(/[:\s]+$/, "");
</script>

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
  {:else if b.type === "table"}
    <div class="table">
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

<style>
  p {
    margin: 0 0 1em;
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
  .table {
    overflow-x: auto;
    margin: 0 0 1em;
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
