<script lang="ts">
  import { imageSource } from "./backend";
  import Dialog from "./Dialog.svelte";
  let { src, alt = "Imagen", projectId = null, compact = false }: { src: string; alt?: string; projectId?: string | null; compact?: boolean } = $props();
  let resolved = $state("");
  let error = $state("");
  let expanded = $state(false);
  $effect(() => {
    let alive = true;
    resolved = ""; error = "";
    imageSource(projectId, src).then((s) => { if (alive) resolved = s; }).catch((e) => { if (alive) error = String(e); });
    return () => { alive = false; };
  });
</script>

<span class="picture" class:compact>
  {#if error}<span class="failure" title={error}>{alt || "Imagen"}: {error}</span>
  {:else if resolved}<button class="preview" onclick={() => (expanded = true)} aria-label="Ampliar {alt || 'imagen'}" title="Ampliar imagen"><img src={resolved} alt={alt} loading="lazy" onerror={() => (error = "No se pudo cargar la imagen.")} /></button>
  {:else}<span class="loading" role="status">Cargando imagen…</span>{/if}
</span>
{#if expanded && resolved}
  <Dialog title={alt || "Imagen"} width="min(70rem, 94vw)" onclose={() => (expanded = false)}>
    {#snippet children()}<div class="full"><img src={resolved} alt={alt} /></div>{/snippet}
  </Dialog>
{/if}
<style>
  .picture { display: inline-block; max-width: 100%; vertical-align: middle; }
  .preview { display: block; max-width: 100%; padding: 0; border: 1px solid var(--line-soft); border-radius: var(--radius); background: var(--bg); overflow: hidden; cursor: zoom-in; }
  .preview img { display: block; max-width: 100%; max-height: 22rem; object-fit: contain; }
  .compact .preview img { width: 2.5rem; height: 2.5rem; object-fit: cover; }
  .failure, .loading { font-size: var(--fs-xs); color: var(--text-3); overflow-wrap: anywhere; }
  .full { padding: 1rem; display: grid; place-items: center; overflow: auto; max-height: 80vh; }
  .full img { max-width: 100%; max-height: 75vh; object-fit: contain; }
</style>
