<script lang="ts">
  import { onDestroy, onMount, tick, untrack } from "svelte";
  import { fileMtime, gitDiff, readFile, writeFile, type FileContent } from "./backend";
  import { bumpFiles, type Card } from "./canvas.svelte";
  import { trackPointer } from "./cardDrag";
  import { parseUnifiedDiff, type Row } from "./diff";
  import DiffView from "./DiffView.svelte";
  import type { Editor } from "./editor";
  import Icon from "./Icon.svelte";
  import { cardIn, cardOut } from "./motion";
  import { projectById } from "./projects.svelte";

  interface Props {
    card: Card;
    scale: number;
    focused: boolean;
    onfocus: () => void;
    onclose: () => void;
    onmove: (dx: number, dy: number) => void;
    onresize: (dw: number, dh: number) => void;
  }
  let { card, scale, focused, onfocus, onclose, onmove, onresize }: Props = $props();

  // Un archivo abierto no cambia de proyecto ni de ruta mientras la tarjeta existe.
  const projectId = $derived(card.projectId ?? "");
  const path = $derived(card.path ?? "");
  const dir = $derived(path.includes("/") ? path.slice(0, path.lastIndexOf("/")) : "");

  let loaded = $state<FileContent | null>(null);
  let failure = $state("");
  let host = $state<HTMLDivElement>();
  let editor: Editor | undefined;
  let mtime = 0;
  let saved = "";
  let dirty = $state(false);
  let saving = $state(false);
  let conflict = $state(false);
  let external = $state<"changed" | "deleted" | null>(null);
  let notice = $state("");
  let confirmClose = $state(false);
  let dragging = $state(false);
  let mode = $state<"code" | "diff">("code");
  let diffRows = $state<Row[]>([]);
  let diffMessage = $state("");

  const project = $derived(projectById(card.projectId));

  async function load() {
    try {
      const f = await readFile(projectId, path);
      loaded = f;
      mtime = f.mtime;
      if (f.kind === "text") saved = f.content;
    } catch (e) {
      failure = String(e);
    }
  }

  // El editor se crea cuando ya hay contenido de texto y un lugar donde ponerlo.
  $effect(() => {
    if (loaded?.kind !== "text" || !host || editor) return;
    const text = untrack(() => (loaded?.kind === "text" ? loaded.content : ""));
    const target = host;
    import("./editor").then(({ createEditor }) => {
      if (editor || !target.isConnected) return;
      editor = createEditor({
        parent: target,
        text,
        filename: card.title,
        onChange: () => (dirty = editor!.getText() !== saved),
        onSave: save,
      });
    });
  });

  async function save() {
    if (!editor || saving || !dirty) return;
    saving = true;
    const text = editor.getText();
    try {
      mtime = await writeFile(projectId, path, text, mtime);
      saved = text;
      dirty = false;
      conflict = false;
      external = null;
      notice = "Guardado";
      bumpFiles();
    } catch (e) {
      if (e === "CONFLICT") conflict = true;
      else notice = String(e);
    } finally {
      saving = false;
    }
  }

  async function overwrite() {
    if (!editor) return;
    saving = true;
    const text = editor.getText();
    try {
      mtime = await writeFile(projectId, path, text, null);
      saved = text;
      dirty = false;
      conflict = false;
      external = null;
      notice = "Guardado";
      bumpFiles();
    } catch (e) {
      notice = String(e);
    } finally {
      saving = false;
    }
  }

  async function reloadFromDisk(quiet = false) {
    try {
      const f = await readFile(projectId, path);
      if (f.kind !== "text") return;
      editor?.setText(f.content);
      saved = f.content;
      mtime = f.mtime;
      dirty = false;
      conflict = false;
      external = null;
      notice = quiet ? "Actualizado: el archivo cambió en disco" : "";
    } catch (e) {
      failure = String(e);
    }
  }

  // Si el archivo cambia en disco (otra IA, otro programa), se avisa o se recarga solo si no hay cambios míos.
  async function poll() {
    if (saving || (loaded?.kind !== "text" && loaded?.kind !== "image")) return;
    try {
      const m = await fileMtime(projectId, path);
      if (m === mtime) return;
      if (loaded.kind === "image" || !dirty) {
        if (loaded.kind === "image") {
          mtime = m;
          await load();
        } else await reloadFromDisk(true);
      } else external = "changed";
    } catch {
      external = "deleted";
    }
  }

  onMount(() => {
    load();
    const t = setInterval(poll, 2500);
    return () => clearInterval(t);
  });

  // Una IA o el árbol avisan de cambios: se comprueba enseguida.
  import { filesChanged } from "./canvas.svelte";
  $effect(() => {
    void filesChanged.version;
    untrack(() => {
      if (loaded) poll();
    });
  });

  onDestroy(() => editor?.destroy());

  async function toggleDiff() {
    if (mode === "diff") {
      mode = "code";
      await tick();
      editor?.focus();
      return;
    }
    mode = "diff";
    diffMessage = "";
    try {
      const d = await gitDiff(projectId, path);
      if (d === null) diffMessage = "Esta carpeta no es un repositorio git, así que no hay con qué comparar.";
      else if (!d.trim()) diffMessage = "Sin cambios respecto al último commit.";
      else diffRows = parseUnifiedDiff(d);
    } catch (e) {
      diffMessage = String(e);
    }
  }

  function requestClose() {
    if (dirty) confirmClose = true;
    else onclose();
  }

  async function saveAndClose() {
    await save();
    if (!dirty) onclose();
  }

  function startDrag(e: PointerEvent) {
    if ((e.target as HTMLElement).closest("button")) return;
    trackPointer(e, scale, onfocus, onmove, (on) => (dragging = on));
  }

  function headerKey(e: KeyboardEvent) {
    if (e.target !== e.currentTarget) return;
    const step = e.shiftKey ? 64 : 16;
    const d: Record<string, [number, number]> = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] };
    const v = d[e.key];
    if (v) {
      e.preventDefault();
      onmove(...v);
    }
  }

  const sizeText = (n: number) => (n >= 1024 * 1024 ? `${(n / 1024 / 1024).toFixed(1)} MB` : n >= 1024 ? `${Math.round(n / 1024)} KB` : `${n} B`);
</script>

<article
  class="card"
  class:focused
  class:dragging
  data-card-id={card.id}
  style:left="{card.x}px"
  style:top="{card.y}px"
  style:width="{card.w}px"
  style:height="{card.h}px"
  style:z-index={card.z}
  aria-label="Archivo {path}"
  onpointerdown={onfocus}
  in:cardIn|global
  out:cardOut|global
>
  <!-- El encabezado es el asa de arrastre; también se mueve con las flechas. -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
  <header role="group" onpointerdown={startDrag} onkeydown={headerKey} tabindex="0" aria-label="Mover {card.title}. Flechas para mover.">
    <span class="fico" class:dirty aria-hidden="true"><Icon name="file" size="0.95rem" /></span>
    <span class="name" title="{project?.name ?? ''} / {path}">{card.title}</span>
    {#if dir}<span class="dir">{dir}</span>{/if}
    <span class="spacer"></span>
    {#if notice && !dirty}<span class="notice" role="status">{notice}</span>{/if}
    {#if loaded?.kind === "text"}
      <button class="btn ghost sm" class:on={mode === "diff"} onclick={toggleDiff} title="Ver lo que cambió respecto al último commit de git">{mode === "diff" ? "Ver el código" : "Cambios"}</button>
      {#if dirty}<button class="btn primary sm" onclick={save} disabled={saving} title="Guardar (Ctrl+S)">Guardar</button>{/if}
    {/if}
    <button class="icon-btn sm" onclick={requestClose} aria-label="Cerrar {card.title}" title="Cerrar"><Icon name="x" size="0.95rem" /></button>
  </header>

  {#if confirmClose}
    <div class="banner warn" role="alert">
      <span>Hay cambios sin guardar.</span>
      <span class="b-actions">
        <button class="btn primary sm" onclick={saveAndClose}>Guardar y cerrar</button>
        <button class="btn sm danger" onclick={onclose}>Cerrar sin guardar</button>
        <button class="btn ghost sm" onclick={() => (confirmClose = false)}>Cancelar</button>
      </span>
    </div>
  {:else if conflict}
    <div class="banner warn" role="alert">
      <span>El archivo cambió en disco mientras lo editabas.</span>
      <span class="b-actions">
        <button class="btn sm" onclick={overwrite}>Sobrescribir</button>
        <button class="btn sm" onclick={() => reloadFromDisk()}>Recargar y perder mis cambios</button>
      </span>
    </div>
  {:else if external === "changed"}
    <div class="banner warn" role="alert">
      <span>El archivo cambió en disco (otra IA u otro programa).</span>
      <span class="b-actions">
        <button class="btn sm" onclick={() => reloadFromDisk()}>Recargar y perder mis cambios</button>
        <button class="btn sm" onclick={() => (external = null)}>Conservar los míos</button>
      </span>
    </div>
  {:else if external === "deleted"}
    <div class="banner warn" role="alert"><span>El archivo ya no existe en disco.</span></div>
  {/if}

  <div class="body">
    {#if failure}
      <p class="msg bad" role="alert">{failure}</p>
    {:else if !loaded}
      <p class="msg">Abriendo…</p>
    {:else if loaded.kind === "unsupported"}
      <p class="msg">{loaded.reason} ({sizeText(loaded.size)})</p>
    {:else if loaded.kind === "image"}
      <div class="image"><img src="data:{loaded.mime};base64,{loaded.data}" alt={card.title} /></div>
    {:else}
      <div class="editor" bind:this={host} hidden={mode === "diff"}></div>
      {#if mode === "diff"}
        <div class="diff">
          {#if diffMessage}<p class="msg">{diffMessage}</p>{:else}<DiffView rows={diffRows} limit={400} />{/if}
        </div>
      {/if}
    {/if}
  </div>

  <div
    class="grip"
    aria-hidden="true"
    onpointerdown={(e) => {
      e.stopPropagation();
      trackPointer(e, scale, onfocus, onresize, (on) => (dragging = on));
    }}
  ></div>
</article>

<style>
  .card {
    position: absolute;
    display: flex;
    flex-direction: column;
    background: var(--bg);
    border: 1px solid var(--line);
    border-radius: var(--radius-xl);
    box-shadow: var(--shadow-card);
    transition:
      border-color var(--dur-base) var(--ease),
      box-shadow var(--dur-base) var(--ease);
  }
  .card.focused {
    border-color: color-mix(in oklch, var(--accent) 50%, var(--line));
    box-shadow:
      var(--shadow-card),
      0 0 0 3px color-mix(in oklch, var(--accent) 12%, transparent);
  }
  .card.dragging {
    box-shadow: var(--lift);
  }
  :global(.card.drop) {
    border-color: var(--accent);
  }

  header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0 0.4rem 0 0.85rem;
    height: 2.6rem;
    flex: none;
    background: var(--surface);
    border-bottom: 1px solid var(--line-soft);
    border-radius: var(--radius-xl) var(--radius-xl) 0 0;
    cursor: grab;
    touch-action: none;
    user-select: none;
  }
  .card.dragging header {
    cursor: grabbing;
  }
  .fico {
    display: grid;
    color: var(--text-3);
  }
  .fico.dirty {
    color: var(--warn);
  }
  .name {
    font-family: var(--mono);
    font-weight: 600;
    font-size: var(--fs-sm);
    white-space: nowrap;
  }
  .dir {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--mono);
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .spacer {
    flex: 1;
  }
  .notice {
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .btn.on {
    background: var(--raised-2);
    color: var(--text);
  }

  .banner {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    padding: 0.4rem 0.75rem;
    font-size: var(--fs-xs);
    background: color-mix(in oklch, var(--warn) 14%, var(--surface));
    border-bottom: 1px solid color-mix(in oklch, var(--warn) 40%, var(--line));
  }
  .b-actions {
    display: inline-flex;
    flex-wrap: wrap;
    gap: 0.3rem;
  }


  .body {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    border-radius: 0 0 var(--radius-xl) var(--radius-xl);
    overflow: hidden;
  }
  .editor {
    flex: 1;
    min-height: 0;
  }
  .editor[hidden] {
    display: none;
  }
  .diff {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 0.6rem;
  }
  .image {
    flex: 1;
    min-height: 0;
    display: grid;
    place-items: center;
    overflow: auto;
    padding: 1rem;
    background-image: radial-gradient(var(--dot) 1px, transparent 1.5px);
    background-size: 18px 18px;
  }
  .image img {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
  }
  .msg {
    margin: 1rem;
    color: var(--text-3);
  }
  .msg.bad {
    color: var(--danger);
  }

  .grip {
    position: absolute;
    right: -1px;
    bottom: -1px;
    width: 1rem;
    height: 1rem;
    cursor: nwse-resize;
    touch-action: none;
    border-bottom-right-radius: var(--radius-xl);
    background: linear-gradient(135deg, transparent 55%, var(--line) 55%, var(--line) 62%, transparent 62%, transparent 74%, var(--line) 74%, var(--line) 81%, transparent 81%);
  }
</style>
