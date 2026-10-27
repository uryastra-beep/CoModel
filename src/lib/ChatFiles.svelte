<script lang="ts">
  // Los archivos que la IA entregó en un mensaje: imágenes con vista previa y el resto con nombre y tamaño.
  import { downloadOutput, openOutput, type OutFile } from "./backend";
  import { openFileCard } from "./canvas.svelte";
  import ChatImage from "./ChatImage.svelte";
  import Icon from "./Icon.svelte";

  let { files, projectId = null }: { files: OutFile[]; projectId?: string | null } = $props();
  let error = $state("");
  let saved = $state("");

  const sizeText = (n: number) => (n >= 1024 * 1024 ? `${(n / 1024 / 1024).toFixed(1)} MB` : n >= 1024 ? `${Math.round(n / 1024)} KB` : `${n} B`);
  const typeText = (f: OutFile) => {
    const ext = f.name.includes(".") ? f.name.split(".").pop()!.toUpperCase() : "";
    return ext && ext.length <= 5 ? ext : f.kind === "image" ? "Imagen" : "Archivo";
  };
  // Texto y código se abren como tarjeta de archivo; el resto con la aplicación del sistema.
  const isText = (f: OutFile) => f.mime.startsWith("text/") || f.mime === "application/json";

  async function open(f: OutFile) {
    error = ""; saved = "";
    try {
      if (isText(f) && projectId) openFileCard(projectId, f.path);
      else await openOutput(projectId, f);
    } catch (e) { error = `${f.name}: ${e}`; }
  }

  async function download(f: OutFile) {
    error = ""; saved = "";
    try {
      if (await downloadOutput(projectId, f)) saved = `${f.name} guardado.`;
    } catch (e) { error = `${f.name}: ${e}`; }
  }
</script>

<div class="files">
  {#each files as f (f.path)}
    {#if f.kind === "image"}
      <figure class="image">
        <ChatImage src={f.path} alt={f.name} {projectId} />
        <figcaption>
          <span class="name" title={f.name}>{f.name}</span>
          <span class="meta">{sizeText(f.size)}</span>
          <button type="button" class="btn ghost sm" onclick={() => download(f)} title="Guardar una copia de {f.name}">Descargar</button>
        </figcaption>
      </figure>
    {:else}
      <div class="file">
        <Icon name="file" size="1rem" />
        <span class="info">
          <span class="name" title={f.name}>{f.name}</span>
          <span class="meta">{typeText(f)} · {sizeText(f.size)}</span>
        </span>
        <button type="button" class="btn ghost sm" onclick={() => open(f)} title="Abrir {f.name}">Abrir</button>
        <button type="button" class="btn ghost sm" onclick={() => download(f)} title="Guardar una copia de {f.name}">Descargar</button>
      </div>
    {/if}
  {/each}
  {#if error}<p class="note bad" role="alert">{error}</p>{:else if saved}<p class="note" role="status">{saved}</p>{/if}
</div>

<style>
  .files {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.45rem;
    margin-top: 0.6rem;
    max-width: 100%;
  }
  .image {
    margin: 0;
    max-width: 100%;
  }
  figcaption {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-top: 0.25rem;
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .file {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    width: 100%;
    padding: 0.45rem 0.55rem 0.45rem 0.7rem;
    border: 1px solid var(--line-soft);
    border-radius: var(--radius);
    background: var(--bg);
    color: var(--text-2);
  }
  .file > :global(.icon) {
    flex: none;
    color: var(--text-3);
  }
  .info {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--fs-sm);
    color: var(--text);
  }
  figcaption .name {
    font-size: var(--fs-xs);
    color: var(--text-2);
    max-width: 14rem;
  }
  .meta {
    font-size: var(--fs-xs);
    color: var(--text-3);
    font-variant-numeric: tabular-nums;
  }
  .note {
    margin: 0;
    font-size: var(--fs-xs);
    color: var(--text-3);
    overflow-wrap: anywhere;
  }
  .note.bad {
    color: var(--danger);
  }
</style>
