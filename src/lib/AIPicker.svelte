<script lang="ts">
  // Selector de IA de un chat: primero la IA y, según cuál sea, sus modelos, esfuerzos y modos.
  import { untrack } from "svelte";
  import { CHAT_PROVIDERS, catalog, defaultsFor, effortsFor, isReady, loadCatalog, modelsFor, modesFor, normalize, remember, type ChatAI } from "./ai.svelte";
  import { connections } from "./connections.svelte";
  import Menu from "./Menu.svelte";
  import { openSettings } from "./ui.svelte";

  interface Props {
    ai: ChatAI;
    hasFolder: boolean;
    onchange: (ai: ChatAI) => void;
    /** Mientras la IA trabaja no se cambia nada. */
    disabled?: boolean;
    /** Los menús abren hacia arriba (caja pegada abajo) o hacia abajo. */
    up?: boolean;
  }
  let { ai, hasFolder, onchange, disabled = false, up = true }: Props = $props();

  const providers = $derived(
    CHAT_PROVIDERS.map((p) => ({
      id: p.id,
      label: p.label,
      dot: p.id,
      hint: p.via,
      note: connections.loaded && !isReady(p.id) ? "Sin conectar" : undefined,
    })),
  );
  const models = $derived(modelsFor(ai.provider));
  const efforts = $derived(effortsFor(ai.provider, ai.model));
  const modes = $derived(modesFor(ai.provider, hasFolder));

  // La lista de la IA elegida se pide la primera vez; al llegar, la elección se corrige si ya no vale.
  $effect(() => {
    const provider = ai.provider;
    loadCatalog(provider);
  });
  $effect(() => {
    void catalog.codex;
    void catalog.chatgpt;
    void catalog.openrouter;
    void hasFolder;
    const fixed = untrack(() => normalize(ai, hasFolder));
    if (untrack(() => JSON.stringify(fixed) !== JSON.stringify(ai))) onchange(fixed);
  });

  function set(patch: Partial<ChatAI>) {
    const next = normalize({ ...ai, ...patch }, hasFolder);
    remember(next);
    onchange(next);
  }

  async function pickProvider(id: string) {
    if (id === ai.provider) return;
    // Una IA sin conectar no se elige: se lleva al usuario a conectarla.
    if (connections.loaded && !isReady(id)) {
      openSettings("ias");
      return;
    }
    // Cada IA vuelve con lo último que se usó con ella (o, la primera vez, con lo que su cuenta tiene por defecto).
    await loadCatalog(id);
    set({ ...defaultsFor(id) });
  }
</script>

<div class="picker" class:disabled inert={disabled}>
  <Menu {up} label="IA" value={ai.provider} options={providers} onchange={pickProvider} title="Qué IA responde en este chat" />
  <span class="sep" aria-hidden="true"></span>
  <Menu
    {up}
    label="Modelo"
    value={ai.model}
    options={models.main}
    more={models.more}
    searchable={models.searchable}
    fallback={ai.model || "Por defecto"}
    onchange={(model) => set({ model })}
    title="Modelo de {CHAT_PROVIDERS.find((p) => p.id === ai.provider)?.label}"
  />
  {#if efforts.length}
    <Menu {up} label="Esfuerzo" icon="gauge" value={ai.effort} options={efforts} onchange={(effort) => set({ effort })} title="Esfuerzo: cuánto piensa antes de responder" />
  {/if}
  {#if modes.length}
    <Menu {up} label="Modo" icon="shield" value={ai.mode} options={modes} onchange={(mode) => set({ mode })} title="Modo: qué puede hacer con tus archivos" />
  {/if}
</div>

<style>
  .picker {
    display: inline-flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.05rem;
    min-width: 0;
  }
  .picker.disabled {
    opacity: 0.55;
  }
  .sep {
    width: 1px;
    height: 0.9rem;
    margin: 0 0.15rem;
    background: var(--line);
  }
</style>
