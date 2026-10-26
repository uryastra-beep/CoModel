<script lang="ts">
  import { canvas } from "./canvas.svelte";
  import { filesChanged } from "./canvas.svelte";
  import FileTree from "./FileTree.svelte";
  import Icon from "./Icon.svelte";
  import Menu from "./Menu.svelte";
  import { projects } from "./projects.svelte";
  import { ui } from "./ui.svelte";

  // La carpeta que se muestra: la elegida a mano, la del chat enfocado o la primera que haya.
  const focused = $derived(canvas.cards.find((c) => c.id === canvas.focusId));
  const current = $derived(
    projects.list.find((p) => p.id === ui.filesProject)?.id ?? projects.list.find((p) => p.id === focused?.projectId)?.id ?? projects.list[0]?.id ?? null,
  );
</script>

{#if projects.list.length === 0}
  <p class="none">Todavía no hay carpetas. Elige una debajo del cuadro de texto de un chat y aparecerá aquí.</p>
{:else if current}
  <div class="bar">
    <Menu
      label="Proyecto"
      value={current}
      options={projects.list.map((p) => ({ id: p.id, label: p.name }))}
      onchange={(id) => (ui.filesProject = id)}
      up={false}
    />
    <button class="icon-btn sm" onclick={() => (filesChanged.version += 1)} aria-label="Actualizar archivos" title="Actualizar"><Icon name="refresh" size="0.9rem" /></button>
  </div>
  {#key current}
    <FileTree projectId={current} />
  {/key}
{/if}

<style>
  .none {
    margin: 0.4rem 0.55rem;
    font-size: var(--fs-sm);
    color: var(--text-3);
  }
  .bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin: 0 0 0.35rem;
    padding-right: 0.1rem;
  }
</style>
