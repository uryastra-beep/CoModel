<script lang="ts">
  import Canvas from "./lib/Canvas.svelte";
  import CommandPalette from "./lib/CommandPalette.svelte";
  import Home from "./lib/Home.svelte";
  import Icon from "./lib/Icon.svelte";
  import ImportChat from "./lib/ImportChat.svelte";
  import Login from "./lib/Login.svelte";
  import Settings from "./lib/Settings.svelte";
  import Sidebar from "./lib/Sidebar.svelte";
  import StatusBar from "./lib/StatusBar.svelte";
  import { account, loadAccount } from "./lib/account.svelte";
  import { canvas, here, openPanel, switchSpace, syncLinkedWorkspaces, layoutSnapshot, loadLayout, saveLayout } from "./lib/canvas.svelte";
  import Menu from "./lib/Menu.svelte";
  import { workspaceChoices } from "./lib/spaces";
  import { loadProjects, projects } from "./lib/projects.svelte";
  import { startChat } from "./lib/start";
  import { onMount } from "svelte";
  import { openSettings, showCanvas, ui, toggleSidebar } from "./lib/ui.svelte";

  const chats = $derived(here().filter((c) => c.kind === "chat").length);
  const files = $derived(here().filter((c) => c.kind === "file").length);
  const working = $derived(here().filter((c) => c.busy).length);

  onMount(() => {
    loadAccount();
    // Primero los proyectos (para saber cuáles siguen existiendo) y luego el lienzo guardado.
    loadProjects().then(async () => {
      loadLayout((id) => projects.list.find((p) => p.id === id)?.name ?? null);
      await syncLinkedWorkspaces();
    });
    // Antes de cerrar o recargar se guarda el lienzo.
    const flush = () => projects.loaded && saveLayout();
    window.addEventListener("pagehide", flush);
    return () => window.removeEventListener("pagehide", flush);
  });

  // El lienzo se guarda solo, poco después de cada cambio.
  $effect(() => {
    if (!projects.loaded) return;
    void JSON.stringify(layoutSnapshot());
    const t = setTimeout(saveLayout, 400);
    return () => clearTimeout(t);
  });

  const onHome = $derived(ui.screen === "home");

  const typing = (t: EventTarget | null) =>
    t instanceof Element && !!t.closest("input, textarea, [contenteditable], [role='combobox'], [role='listbox'], .cm-editor");

  function onKey(e: KeyboardEvent) {
    const mod = e.ctrlKey || e.metaKey;
    const k = e.key.toLowerCase();
    if (mod && k === "k") {
      e.preventDefault();
      ui.paletteOpen = !ui.paletteOpen;
      return;
    }
    if (mod && k === "b" && !typing(e.target)) {
      e.preventDefault();
      toggleSidebar();
      return;
    }
    if (mod && e.key === ",") {
      e.preventDefault();
      openSettings("perfil");
      return;
    }
    if (k !== "n" || mod || e.altKey || typing(e.target) || ui.view === "settings" || ui.paletteOpen) return;
    e.preventDefault();
    startChat("", null);
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="shell" class:collapsed={ui.sidebarCollapsed}>
  <Sidebar />
  <div class="main">
    <header class="toolbar">
      <div class="where">
        {#if onHome}
          <h1>Inicio</h1>
        {:else}
          <h1 class="sr-only">Workspace</h1>
          {#if canvas.workspaces.length}
            <Menu up={false} label="Workspace" icon="canvas" value={canvas.space} options={workspaceChoices()} onchange={switchSpace} title="Cambiar de workspace" />
          {:else}
            <span class="noname">Sin workspaces</span>
          {/if}
        {/if}
        {#if !onHome}
          <span class="meta">
            {#if chats + files === 0}Vacío{:else}{chats === 1 ? "1 chat" : `${chats} chats`}{#if files}, {files === 1 ? "1 archivo" : `${files} archivos`}{/if}{/if}
            {#if working > 0}<span class="busy"><span class="activity on" aria-hidden="true"></span>{working === 1 ? "1 trabajando" : `${working} trabajando`}</span>{/if}
          </span>
        {/if}
      </div>
      <div class="acts">
        <button class="search" onclick={() => (ui.paletteOpen = true)} aria-label="Buscar chats y acciones" title="Buscar chats y acciones (Ctrl+K)">
          <Icon name="search" size="0.95rem" />
          <span class="ph">Buscar</span>
          <kbd>Ctrl K</kbd>
        </button>
        <button class="btn sm" onclick={() => { openPanel("files", canvas.cards.find((c) => c.id === canvas.focusId)?.projectId ?? null); showCanvas(); }} title="Abrir un panel de archivos"><Icon name="folder" size="0.95rem" />Archivos</button>
        <button class="btn sm" onclick={() => { openPanel("code"); showCanvas(); }} title="Abrir un panel de código"><Icon name="code" size="0.95rem" />Código</button>
        <button class="btn sm" onclick={() => { openPanel("terminal"); showCanvas(); }} title="Abrir una terminal en la carpeta del workspace"><Icon name="terminal" size="0.95rem" />Terminal</button>
        <button class="btn primary sm new" onclick={() => startChat("", null)} title="Nuevo chat (N)">
          <Icon name="plus" size="0.95rem" />Nuevo chat
        </button>
      </div>
    </header>
    <div class="stage">
      {#if canvas.projectError}<div class="workspace-error" role="alert">{canvas.projectError}<button class="btn ghost sm" onclick={() => (canvas.projectError = "")}>Cerrar</button></div>{/if}
      <div class="layer" inert={onHome}>
        <Canvas />
      </div>
      {#if onHome}<Home />{/if}
    </div>
    <StatusBar />
  </div>
</div>

{#if ui.view === "settings"}<Settings />{/if}
{#if account.dialog}<Login />{/if}
{#if ui.importOpen}<ImportChat />{/if}
{#if ui.paletteOpen}<CommandPalette />{/if}

<style>
  .noname {
    font-size: var(--fs-sm);
    color: var(--text-3);
  }
  .shell {
    display: grid;
    grid-template-columns: 15.5rem minmax(0, 1fr);
    height: 100%;
    transition: grid-template-columns var(--dur-slow) var(--ease);
  }
  .shell.collapsed {
    grid-template-columns: 3.25rem minmax(0, 1fr);
  }
  .main {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    grid-template-rows: 3rem minmax(0, 1fr) auto;
    min-width: 0;
  }
  .toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0 0.75rem 0 1.25rem;
    border-bottom: 1px solid var(--line-soft);
    background: var(--surface);
  }
  .where {
    display: flex;
    align-items: baseline;
    gap: 0.75rem;
    min-width: 0;
  }
  h1 {
    margin: 0;
    font-size: var(--fs-sm);
    font-weight: 600;
  }
  .meta {
    display: inline-flex;
    align-items: center;
    gap: 0.75rem;
    font-size: var(--fs-xs);
    color: var(--text-3);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .busy {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    color: var(--text-2);
  }
  .acts {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    width: 15rem;
    height: var(--control-sm);
    padding: 0 0.35rem 0 0.6rem;
    border: 1px solid var(--line-soft);
    border-radius: var(--radius);
    background: var(--bg);
    color: var(--text-3);
    font-size: var(--fs-xs);
    transition:
      border-color var(--dur-fast) var(--ease),
      color var(--dur-fast) var(--ease);
  }
  .search:hover {
    border-color: var(--line);
    color: var(--text-2);
  }
  .ph {
    flex: 1;
    text-align: left;
  }
  .new {
    gap: 0.35rem;
  }
  .workspace-error { position: absolute; top: 0.5rem; left: 1rem; right: 1rem; z-index: 10000; display: flex; align-items: center; justify-content: space-between; padding: 0.5rem 0.75rem; border: 1px solid var(--danger); border-radius: var(--radius); background: var(--surface); color: var(--danger); font-size: var(--fs-sm); }
  .stage {
    position: relative;
    min-height: 0;
  }
  .layer {
    position: absolute;
    inset: 0;
  }
  @media (max-width: 52rem) {
    .search {
      width: auto;
    }
    .search .ph,
    .search kbd {
      display: none;
    }
  }
</style>
