<script lang="ts">
  import { copyText, openExternal } from "./clipboard";
  import Icon from "./Icon.svelte";
  import ChatImage from "./ChatImage.svelte";
  import { openPanel } from "./canvas.svelte";
  import { showCanvas } from "./ui.svelte";
  import { parseMarkdown, type Inline, type ListItem } from "./markdown";

  import { saveGeneratedFiles, saveGeneratedZip, type SaveReport } from "./backend";
  import { projectById } from "./projects.svelte";

  let { text, projectId = null, live = false }: { text: string; projectId?: string | null; /** La respuesta se sigue escribiendo: aún no se pueden descargar sus archivos. */ live?: boolean } = $props();
  const blocks = $derived(parseMarkdown(text));
  // Archivos que la IA entrega para guardar; si repite uno, vale el último.
  const files = $derived([...new Map(blocks.flatMap((b) => (b.t === "code" && b.file ? [[b.file, { path: b.file, content: b.v }] as const] : []))).values()]);
  let fileNote = $state<{ ok: boolean; text: string } | null>(null);
  let fileBusy = $state(false);

  async function downloadZip() {
    fileBusy = true;
    try {
      const path = await saveGeneratedZip(files);
      fileNote = path ? { ok: true, text: `Zip guardado en ${path}` } : null;
    } catch (e) {
      fileNote = { ok: false, text: e instanceof Error ? e.message : String(e) };
    } finally {
      fileBusy = false;
    }
  }

  async function saveToFolder() {
    fileBusy = true;
    try {
      const r: SaveReport = await saveGeneratedFiles(projectId, files);
      const where = projectById(projectId)?.name ?? "la carpeta";
      fileNote = {
        ok: true,
        text: `${r.written.length ? `Guardados en ${where}: ${r.written.join(", ")}.` : "No se guardó nada."}${r.skipped.length ? ` Ya existían y no se tocaron: ${r.skipped.join(", ")}.` : ""}`,
      };
    } catch (e) {
      fileNote = { ok: false, text: e instanceof Error ? e.message : String(e) };
    } finally {
      fileBusy = false;
    }
  }

  // Qué bloque de código se acaba de copiar (para mostrar la confirmación un momento).
  let copied = $state<number | null>(null);
  async function copy(i: number, v: string) {
    if (await copyText(v)) {
      copied = i;
      setTimeout(() => {
        if (copied === i) copied = null;
      }, 1500);
    }
  }
</script>

{#snippet inline(parts: Inline[])}
  {#each parts as p}
    {#if p.t === "image"}<ChatImage src={p.src} alt={p.alt} {projectId} />{:else if p.t === "code"}<code>{p.v}</code>{:else if p.t === "strong"}<strong>{@render inline(p.v)}</strong>{:else if p.t === "em"}<em>{@render inline(p.v)}</em>{:else if p.t === "del"}<del>{@render inline(p.v)}</del>{:else if p.t === "link"}<a
        href={p.href}
        title={p.href}
        onclick={(e) => {
          e.preventDefault();
          openExternal(p.href);
        }}>{@render inline(p.v)}</a
      >{:else}{p.v}{/if}
  {/each}
{/snippet}

{#snippet items(list: ListItem[])}
  {#each list as it}
    <li>
      {@render inline(it.inl)}
      {#if it.sub}
        {#if it.sub.ordered}
          <ol>{#each it.sub.items as s}<li>{@render inline(s)}</li>{/each}</ol>
        {:else}
          <ul>{#each it.sub.items as s}<li>{@render inline(s)}</li>{/each}</ul>
        {/if}
      {/if}
    </li>
  {/each}
{/snippet}

<div class="md">
  {#each blocks as b, i}
    {#if b.t === "p"}
      <p>{@render inline(b.inl)}</p>
    {:else if b.t === "h"}
      <p class="h h{b.level}">{@render inline(b.inl)}</p>
    {:else if b.t === "ul"}
      <ul>{@render items(b.items)}</ul>
    {:else if b.t === "ol"}
      <ol start={b.start}>{@render items(b.items)}</ol>
    {:else if b.t === "quote"}
      <blockquote>{@render inline(b.inl)}</blockquote>
    {:else if b.t === "hr"}
      <hr />
    {:else if b.t === "table"}
      <div class="table">
        <table>
          <thead>
            <tr>{#each b.head as h, c}<th style:text-align={b.align[c] ?? "left"}>{@render inline(h)}</th>{/each}</tr>
          </thead>
          <tbody>
            {#each b.rows as r}
              <tr>{#each r as cell, c}<td style:text-align={b.align[c] ?? "left"}>{@render inline(cell)}</td>{/each}</tr>
            {/each}
          </tbody>
        </table>
      </div>
    {:else}
      <div class="code">
        <div class="code-head">
          <span class="lang" class:path={b.file}>{b.file ?? (b.lang || "texto")}</span>
          <button class="copy" onclick={() => { openPanel("code", projectId, b.v, b.lang || "txt"); showCanvas(); }} title="Abrir en un panel independiente"><Icon name="terminal" size="0.85rem" />Abrir panel</button>
          <button class="copy" onclick={() => copy(i, b.v)} aria-label="Copiar código" title="Copiar código">
            {#if copied === i}<Icon name="check" size="0.85rem" />Copiado{:else}<Icon name="copy" size="0.85rem" />Copiar{/if}
          </button>
        </div>
        <pre><code>{b.v}</code></pre>
      </div>
    {/if}
  {/each}
  {#if files.length}
    <div class="genfiles" role="group" aria-label="Archivos entregados">
      <span class="gen-count">{files.length === 1 ? "1 archivo listo" : `${files.length} archivos listos`}</span>
      <span class="grow"></span>
      <button class="btn sm" onclick={downloadZip} disabled={live || fileBusy} title={live ? "Espera a que termine de escribir" : "Guardar todos los archivos en un zip"}>Descargar zip</button>
      {#if projectId}<button class="btn sm" onclick={saveToFolder} disabled={live || fileBusy} title="Guardarlos en la carpeta del workspace, sin sobrescribir lo que ya exista">Guardar en la carpeta</button>{/if}
    </div>
    {#if fileNote}<p class="gen-note" class:bad={!fileNote.ok} role={fileNote.ok ? "status" : "alert"}>{fileNote.text}</p>{/if}
  {/if}
</div>

<style>
  .lang.path {
    font-family: var(--mono);
    font-size: var(--fs-xs);
  }
  .genfiles {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin: 0.4rem 0 0;
    padding: 0.45rem 0.6rem;
    border: 1px solid var(--line);
    border-radius: var(--radius-lg);
    background: var(--surface);
  }
  .gen-count {
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .grow {
    flex: 1;
  }
  .gen-note {
    margin: 0.35rem 0 0;
    font-size: var(--fs-xs);
    color: var(--ok);
    overflow-wrap: anywhere;
  }
  .gen-note.bad {
    color: var(--danger);
  }
  .md {
    font-size: var(--fs-md);
    line-height: 1.62;
    overflow-wrap: anywhere;
    color: var(--text);
  }
  .md > :first-child {
    margin-top: 0;
  }
  .md > :last-child {
    margin-bottom: 0;
  }
  p {
    margin: 0 0 0.7em;
  }
  .h {
    margin: 1.1em 0 0.4em;
    font-weight: 600;
    line-height: 1.35;
    letter-spacing: -0.005em;
  }
  .h1 {
    font-size: 1.2em;
  }
  .h2 {
    font-size: 1.08em;
  }
  ul,
  ol {
    margin: 0 0 0.7em;
    padding-left: 1.35rem;
  }
  li {
    margin: 0.15em 0;
  }
  li > ul,
  li > ol {
    margin: 0.15em 0 0.2em;
  }
  li::marker {
    color: var(--text-3);
  }
  ol > li::marker {
    font-variant-numeric: tabular-nums;
  }
  strong {
    font-weight: 600;
  }
  del {
    color: var(--text-3);
  }
  a {
    color: var(--accent);
    text-decoration: underline;
    text-decoration-color: color-mix(in oklch, var(--accent) 40%, transparent);
    text-underline-offset: 0.2em;
  }
  a:hover {
    text-decoration-color: currentColor;
  }
  blockquote {
    margin: 0 0 0.7em;
    padding: 0.1em 0 0.1em 0.85rem;
    border-left: 1px solid var(--line);
    color: var(--text-2);
  }
  hr {
    margin: 1em 0;
    border: 0;
    border-top: 1px solid var(--line-soft);
  }
  code {
    font-family: var(--mono);
    font-size: 0.86em;
    padding: 0.08em 0.32em;
    border-radius: var(--radius-sm);
    background: var(--raised);
    border: 1px solid var(--line-soft);
  }
  .code {
    margin: 0 0 0.8em;
    border: 1px solid var(--line-soft);
    border-radius: var(--radius);
    background: var(--bg);
    overflow: hidden;
  }
  .code-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 1.9rem;
    padding: 0 0.3rem 0 0.75rem;
    border-bottom: 1px solid var(--line-soft);
    background: color-mix(in oklch, var(--raised) 60%, var(--bg));
  }
  .lang {
    font-family: var(--mono);
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .copy {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    height: 1.4rem;
    padding: 0 0.45rem;
    border: 0;
    border-radius: var(--radius-sm);
    background: none;
    font-size: var(--fs-xs);
    color: var(--text-3);
    transition:
      background-color var(--dur-fast) var(--ease),
      color var(--dur-fast) var(--ease);
  }
  .copy:hover {
    background: var(--raised-2);
    color: var(--text);
  }
  pre {
    margin: 0;
    padding: 0.65rem 0.8rem;
    overflow-x: auto;
  }
  pre code {
    padding: 0;
    border: 0;
    background: none;
    font-size: var(--fs-sm);
    line-height: 1.55;
    overflow-wrap: normal;
    white-space: pre;
  }
  .table {
    margin: 0 0 0.8em;
    overflow-x: auto;
    border: 1px solid var(--line-soft);
    border-radius: var(--radius);
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--fs-sm);
    line-height: 1.45;
    font-variant-numeric: tabular-nums;
  }
  th,
  td {
    padding: 0.4rem 0.65rem;
    border-bottom: 1px solid var(--line-soft);
    vertical-align: top;
    overflow-wrap: normal;
  }
  th {
    font-weight: 600;
    color: var(--text-2);
    background: var(--raised);
    white-space: nowrap;
  }
  tbody tr:last-child td {
    border-bottom: 0;
  }
</style>
