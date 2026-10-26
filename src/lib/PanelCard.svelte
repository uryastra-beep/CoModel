<script lang="ts">
  import { onMount } from "svelte";
  import { saveTextFile } from "./backend";
  import { bumpFiles, setCardProject, type Card } from "./canvas.svelte";
  import { trackPointer } from "./cardDrag";
  import type { Editor } from "./editor";
  import FileTree from "./FileTree.svelte";
  import FolderMenu from "./FolderMenu.svelte";
  import Icon from "./Icon.svelte";
  import { cardIn, cardOut } from "./motion";
  let { card, scale, focused, onfocus, onclose, onmove, onresize }: {
    card: Card; scale: number; focused: boolean; onfocus: () => void; onclose: () => void;
    onmove: (dx: number, dy: number) => void; onresize: (dw: number, dh: number) => void;
  } = $props();
  let host = $state<HTMLDivElement>();
  let editor: Editor | undefined;
  let notice = $state("");
  let saving = $state(false);
  let dragging = $state(false);
  async function save() {
    if (!editor || saving) return;
    saving = true;
    try { const path = await saveTextFile(card.title, editor.getText()); if (path) notice = `Guardado: ${path}`; }
    catch (e) { notice = String(e); }
    finally { saving = false; }
  }
  onMount(() => {
    let alive = true;
    if (card.kind === "code") import("./editor").then(({ createEditor }) => {
      if (!alive) return;
      editor = createEditor({ parent: host!, text: card.content ?? "", filename: card.title, onChange: () => (card.content = editor!.getText()), onSave: save });
    });
    return () => { alive = false; editor?.destroy(); };
  });
  function drag(e: PointerEvent) {
    if ((e.target as HTMLElement).closest("button, input")) return;
    trackPointer(e, scale, onfocus, onmove, (on) => (dragging = on));
  }
</script>
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<article class="card" class:focused class:dragging data-card-id={card.id} style:left="{card.x}px" style:top="{card.y}px" style:width="{card.w}px" style:height="{card.h}px" style:z-index={card.z} onpointerdown={onfocus} in:cardIn out:cardOut aria-label={card.title}>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <header onpointerdown={drag}>
    <Icon name={card.kind === "files" ? "folder" : "terminal"} size="1rem" /><strong>{card.title}</strong><span class="spacer"></span>
    {#if card.kind === "code"}<button class="btn sm" onclick={save} disabled={saving}>Guardar como…</button>{:else}<button class="icon-btn sm" onclick={bumpFiles} aria-label="Actualizar archivos" title="Actualizar archivos"><Icon name="refresh" size="0.9rem" /></button>{/if}
    <button class="icon-btn sm" onclick={onclose} aria-label="Cerrar {card.title}" title="Cerrar panel"><Icon name="x" size="0.95rem" /></button>
  </header>
  {#if notice}<p class="notice" role="status">{notice}</p>{/if}
  {#if card.kind === "files"}
    <div class="folder"><FolderMenu projectId={card.projectId} locked={false} up={false} onchange={(id) => setCardProject(card.id, id)} /></div>
    <div class="tree">{#if card.projectId}{#key card.projectId}<FileTree projectId={card.projectId} />{/key}{:else}<p class="notice">Elige una carpeta. Al abrir un archivo aparecerá en su propio panel.</p>{/if}</div>
  {:else}<div class="editor" bind:this={host}></div>{/if}
  <div class="grip" aria-hidden="true" onpointerdown={(e) => { e.stopPropagation(); trackPointer(e, scale, onfocus, onresize, (on) => (dragging = on)); }}></div>
</article>
<style>
  .card { position: absolute; display: flex; flex-direction: column; background: var(--bg); border: 1px solid var(--line); border-radius: var(--radius-xl); box-shadow: var(--shadow-card); transition: border-color var(--dur-base) var(--ease), box-shadow var(--dur-base) var(--ease); }
  .focused { border-color: var(--accent); }
  .dragging { box-shadow: var(--lift); }
  header { display: flex; gap: 0.5rem; align-items: center; padding: 0 0.4rem 0 0.85rem; height: 2.6rem; flex: none; background: var(--surface); border-bottom: 1px solid var(--line-soft); border-radius: var(--radius-xl) var(--radius-xl) 0 0; cursor: grab; touch-action: none; }
  strong { font-size: var(--fs-sm); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .spacer { flex: 1; }
  .folder { padding: 0.4rem; }
  .tree, .editor { flex: 1; min-height: 0; overflow: auto; border-radius: 0 0 var(--radius-xl) var(--radius-xl); }
  .tree { padding: 0.4rem; }
  .notice { margin: 0.5rem 0.8rem; font-size: var(--fs-xs); color: var(--text-3); overflow-wrap: anywhere; }
  .grip { position: absolute; right: -1px; bottom: -1px; width: 1rem; height: 1rem; cursor: nwse-resize; touch-action: none; border-bottom: 2px solid var(--line); border-right: 2px solid var(--line); border-radius: 0 0 var(--radius-xl) 0; }
</style>
