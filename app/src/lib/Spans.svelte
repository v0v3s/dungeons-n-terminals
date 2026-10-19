<!-- Inline text of an entry: styles, game mechanics and links. -->
<script lang="ts">
  import type { Link, Span } from "./api";

  let { spans, onlink }: { spans: Span[]; onlink: (link: Link) => void } = $props();

  const classes = (s: Span) =>
    [
      s.style.bold && "b",
      s.style.italic && "i",
      s.style.underline && "u",
      s.style.strike && "s",
      s.style.code && "code",
      s.style.mechanic && "mech",
      s.style.muted && "dim",
    ]
      .filter(Boolean)
      .join(" ");
</script>

{#each spans as s, i (i)}{#if s.link}<a
      class="link {classes(s)}"
      href="#{s.link.tag}"
      onclick={(e) => {
        e.preventDefault();
        onlink(s.link!);
      }}>{s.text}</a
    >{:else}<span
      class={classes(s)}>{s.text}</span
    >{/if}{/each}

<style>
  .b { font-weight: 700; }
  .i { font-style: italic; }
  .u { text-decoration: underline; }
  .s { text-decoration: line-through; }
  .code { color: var(--code); }
  .mech { color: var(--mechanic); }
  /* Inline, so links wrap with the text around them. */
  .link {
    color: var(--link);
    text-decoration: none;
    cursor: pointer;
  }
  .link:hover,
  .link:focus-visible {
    background: var(--link);
    color: var(--black);
    outline: none;
  }
</style>
