<script lang="ts">
  import { countOf, type Row } from "./diff";

  interface Props {
    rows: Row[];
    /** Líneas visibles antes de "Mostrar más". */
    limit?: number;
  }
  let { rows, limit = 12 }: Props = $props();

  let all = $state(false);
  const shown = $derived(all ? rows : rows.slice(0, limit));
  const n = $derived(countOf(rows));
</script>

<div class="diff" role="group" aria-label="Cambios: {n.add} líneas añadidas, {n.del} quitadas">
  <pre>{#each shown as r}<span class="r {r.t}"><span class="m" aria-hidden="true">{r.t === "add" ? "+" : r.t === "del" ? "−" : r.t === "hunk" ? "…" : " "}</span>{r.text || " "}
</span>{/each}</pre>
  {#if rows.length > limit}
    <button class="more" onclick={() => (all = !all)}>{all ? "Mostrar menos" : `Mostrar ${rows.length - limit} líneas más`}</button>
  {/if}
</div>

<style>
  .diff {
    border: 1px solid var(--line-soft);
    border-radius: var(--radius);
    background: var(--bg);
    overflow: hidden;
  }
  pre {
    margin: 0;
    padding: 0.35rem 0;
    overflow-x: auto;
    font-family: var(--mono);
    font-size: var(--fs-xs);
    line-height: 1.55;
  }
  .r {
    display: block;
    padding: 0 0.6rem 0 0;
    white-space: pre;
    color: var(--text-2);
  }
  .m {
    display: inline-block;
    width: 1.4rem;
    text-align: center;
    color: var(--text-3);
    user-select: none;
  }
  .add {
    background: color-mix(in oklch, var(--ok) 13%, transparent);
    color: var(--text);
  }
  .add .m {
    color: var(--ok);
  }
  .hunk {
    margin-top: 0.15rem;
    color: var(--text-3);
    background: var(--raised);
    font-style: italic;
  }
  .del {
    background: color-mix(in oklch, var(--danger) 13%, transparent);
    color: var(--text);
  }
  .del .m {
    color: var(--danger);
  }
  .more {
    display: block;
    width: 100%;
    padding: 0.3rem;
    border: 0;
    border-top: 1px solid var(--line-soft);
    background: none;
    font-size: var(--fs-xs);
    color: var(--text-2);
  }
  .more:hover {
    background: var(--raised);
    color: var(--text);
  }
</style>
