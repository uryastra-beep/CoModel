<script lang="ts">
  import { canvas, removeWorkspace, renameWorkspace, switchSpace } from "./canvas.svelte";
  import { chatsIn } from "./spaces";
  import Curtain from "./Curtain.svelte";
  import Icon from "./Icon.svelte";
  import { projectById, projects } from "./projects.svelte";
  import { rise } from "./motion";
  import { onMount } from "svelte";
  import FilesPanel from "./FilesPanel.svelte";
  import ProjectsPanel from "./ProjectsPanel.svelte";
  import { connections, refreshConnections, statusOf } from "./connections.svelte";
  import Avatar from "./Avatar.svelte";
  import { profile } from "./profile.svelte";
  import { account, openLogin } from "./account.svelte";
  import { PROVIDER_INFO } from "./providers";
  import { startWorkspace } from "./start";
  import { openSettings, showCanvas, showHome, toggleSidebar, ui } from "./ui.svelte";
  import { usage } from "./usage.svelte";


  /** Al editar el nombre, el campo toma el foco con el texto seleccionado. */
  function focusSelect(node: HTMLInputElement) {
    node.focus();
    node.select();
  }

  onMount(() => {
    if (!connections.loaded) refreshConnections();
  });

  const workspaces = $derived(canvas.workspaces);
  let renaming = $state<string | null>(null);
  const collapsed = $derived(ui.sidebarCollapsed);

  // La sección de relevo se pliega; se recuerda la elección.
  const RELAY_KEY = "comodel.relayOpen";
  let relayOpen = $state(read());

  function read(): boolean {
    try {
      return localStorage.getItem(RELAY_KEY) !== "0";
    } catch {
      return true;
    }
  }

  function toggleRelay() {
    relayOpen = !relayOpen;
    try {
      localStorage.setItem(RELAY_KEY, relayOpen ? "1" : "0");
    } catch {
      // Sin almacenamiento: solo dura esta sesión.
    }
  }

  /** Las IAs del chat enfocado (las de su proyecto, o las por defecto si no tiene carpeta), en orden de relevo. */
  const focused = $derived(canvas.cards.find((c) => c.id === canvas.focusId));
  const focusedProject = $derived(projectById(focused?.projectId));
  const relayIds = $derived(focusedProject?.relay ?? projects.defaultRelay);

  function relayState(id: string): { text: string; tone: "ok" | "muted" | "warn" } {
    const st = statusOf(id);
    if (!connections.loaded) return { text: "…", tone: "muted" };
    if (!st?.available) return { text: "Próximamente", tone: "muted" };
    if (!st.ready) return { text: "Sin conectar", tone: "warn" };
    const five = usage.byProvider[id]?.five_hour;
    return { text: five ? `${Math.round(five.fraction * 100)}% usado` : "Lista", tone: "ok" };
  }

  function openWorkspace(id: string) {
    showCanvas();
    switchSpace(id);
  }

  const busyIn = (space: string) => canvas.cards.some((c) => c.space === space && c.busy);
  const providerIn = (space: string) => canvas.cards.find((c) => c.space === space && c.kind === "chat")?.ai?.provider ?? "claude";

  const onHome = $derived(ui.screen === "home");
  const userAction = () => (account.user || !account.configured ? openSettings("perfil") : openLogin());
  const userSub = $derived(account.user ? account.user.email : account.configured ? "Iniciar sesión" : "Perfil local");
</script>

<aside class:collapsed aria-label="Barra lateral">
  <div class="top">
    {#if collapsed}
      <button class="icon-btn mark" onclick={toggleSidebar} aria-label="Mostrar barra lateral" title="Mostrar barra lateral (Ctrl+B)">
        <img src="/mark.png" alt="" width="18" height="15" />
      </button>
    {:else}
      <span class="brand"><img src="/mark.png" alt="" width="18" height="15" />CoModel</span>
      <button class="icon-btn sm" onclick={toggleSidebar} aria-label="Ocultar barra lateral" title="Ocultar barra lateral (Ctrl+B)"><Icon name="sidebar" size="1rem" /></button>
    {/if}
  </div>

  <nav class="primary" aria-label="Pantallas">
    <button class="nav" aria-current={onHome ? "page" : undefined} onclick={showHome} title="Inicio">
      <Icon name="home" size="1.05rem" /><span class="lbl">Inicio</span>
    </button>
    <button class="nav" aria-current={!onHome && ui.view !== "settings" ? "page" : undefined} onclick={showCanvas} title="Lienzo">
      <Icon name="canvas" size="1.05rem" /><span class="lbl">Lienzo</span>
      {#if workspaces.length && !collapsed}<span class="count">{workspaces.length}</span>{/if}
    </button>
    {#if collapsed}
      <button class="nav" onclick={() => (ui.paletteOpen = true)} title="Buscar (Ctrl+K)" aria-label="Buscar"><Icon name="search" size="1.05rem" /></button>
      <button class="nav" onclick={() => startWorkspace()} title="Nuevo workspace" aria-label="Nuevo workspace"><Icon name="new-chat" size="1.05rem" /></button>
    {/if}
  </nav>

  {#if !collapsed}
    <div class="body">
      <div class="tabs" role="tablist" aria-label="Contenido de la barra lateral">
        {#each [["chats", "Workspaces"], ["projects", "Proyectos"], ["files", "Archivos"]] as [id, label] (id)}
          <button
            role="tab"
            aria-selected={ui.sidebarTab === id}
            class:on={ui.sidebarTab === id}
            onclick={() => (ui.sidebarTab = id as typeof ui.sidebarTab)}>{label}</button
          >
        {/each}
      </div>

      <div class="panel">
        {#if ui.sidebarTab === "files"}
          <FilesPanel />
        {:else if ui.sidebarTab === "projects"}
          <ProjectsPanel />
        {:else if workspaces.length === 0}
          <div class="empty">
            <p>Todavía no hay workspaces.</p>
            <button class="btn sm" onclick={() => startWorkspace()}><Icon name="plus" size="0.9rem" />Nuevo workspace</button>
            <button class="link imp" onclick={() => (ui.importOpen = true)}>Importar un chat</button>
          </div>
        {:else}
          <ul class="chats">
            {#each workspaces as w (w.id)}
              {@const proj = projectById(w.projectId)}
              {@const n = chatsIn(w.id)}
              <li in:rise={{ x: -6, y: 0, duration: 170 }} out:rise={{ x: -6, y: 0, duration: 120 }}>
                {#if renaming === w.id}
                  <input
                    class="input rename"
                    aria-label="Nombre del workspace"
                    value={w.name}
                    maxlength="40"
                    use:focusSelect
                    onblur={() => (renaming = null)}
                    onkeydown={(e) => {
                      if (e.key === "Enter") { renameWorkspace(w.id, e.currentTarget.value); renaming = null; }
                      else if (e.key === "Escape") renaming = null;
                    }}
                  />
                {:else}
                  <button class="chat" class:current={canvas.space === w.id && !onHome} aria-current={canvas.space === w.id} onclick={() => openWorkspace(w.id)} ondblclick={() => (renaming = w.id)} title="Doble clic para renombrar">
                    {#if busyIn(w.id)}<span class="activity on" aria-hidden="true"></span>{:else}<span class="pdot" data-p={providerIn(w.id)} aria-hidden="true"></span>{/if}
                    <span class="name">{w.name}</span>
                    {#if busyIn(w.id)}<span class="tag">Trabajando</span>{:else}<span class="tag" title={proj?.path}>{n === 1 ? "1 chat" : `${n} chats`}</span>{/if}
                  </button>
                {/if}
                <button class="close icon-btn sm" onclick={() => removeWorkspace(w.id)} aria-label="Cerrar {w.name}" title="Cerrar el workspace (su historial se guarda en su carpeta)"><Icon name="x" size="0.85rem" /></button>
              </li>
            {/each}
          </ul>
          <button class="link imp" onclick={() => startWorkspace()}><Icon name="plus" size="0.85rem" />Nuevo workspace</button>
          <button class="link imp" onclick={() => (ui.importOpen = true)}><Icon name="import" size="0.85rem" />Importar un chat</button>
        {/if}
      </div>
    </div>

    <section class="relay" aria-label="Orden de relevo">
      <button class="fold" aria-expanded={relayOpen} aria-controls="relay-body" onclick={toggleRelay}>
        <span class="ftitle">Relevo</span>
        <span class="fsub">{focusedProject ? focusedProject.name : "Chats sin carpeta"}</span>
        <span class="chev" class:open={relayOpen} aria-hidden="true"><Icon name="down" size="0.85rem" /></span>
      </button>
      <Curtain open={relayOpen} id="relay-body">
        <ol>
          {#each relayIds as id, i (id)}
            {@const st = relayState(id)}
            <li>
              <span class="num" aria-hidden="true">{i + 1}</span>
              <span class="pdot" data-p={id} aria-hidden="true"></span>
              <span class="rname">{PROVIDER_INFO[id]?.label ?? id}</span>
              <span class="rstate {st.tone}">{st.text}</span>
            </li>
          {/each}
        </ol>
      </Curtain>
    </section>
  {:else}
    <div class="spacer"></div>
  {/if}

  <div class="foot">
    <button class="user" onclick={userAction} aria-label={account.user || !account.configured ? `Perfil de ${profile.name}` : "Iniciar sesión"} title={collapsed ? profile.name : undefined}>
      <Avatar />
      {#if !collapsed}
        <span class="who">
          <span class="uname">{profile.name}</span>
          <span class="plan">{userSub}</span>
        </span>
      {/if}
    </button>
    <button class="icon-btn" class:on={ui.view === "settings"} onclick={() => openSettings()} aria-label="Ajustes" title="Ajustes (Ctrl+,)"><Icon name="settings" size="1.05rem" /></button>
  </div>
</aside>

<style>
  .rename {
    height: var(--control-sm);
    flex: 1;
    min-width: 0;
  }
  aside {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-width: 0;
    overflow: hidden;
    background: var(--surface);
    border-right: 1px solid var(--line-soft);
  }

  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex: none;
    height: 3rem;
    padding: 0 0.6rem 0 1rem;
  }
  .collapsed .top {
    justify-content: center;
    padding: 0;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 0.55rem;
    font-size: var(--fs-sm);
    font-weight: 650;
    letter-spacing: 0.005em;
    white-space: nowrap;
  }
  .mark img,
  .brand img {
    display: block;
  }

  .primary {
    display: grid;
    gap: 0.1rem;
    padding: 0.15rem 0.5rem 0.6rem;
  }
  .collapsed .primary {
    justify-items: center;
    padding: 0.15rem 0.4rem 0.6rem;
  }
  .nav {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    height: var(--control);
    padding: 0 0.55rem;
    border: 0;
    border-radius: var(--radius);
    background: none;
    color: var(--text-2);
    font-size: var(--fs-sm);
    text-align: left;
    white-space: nowrap;
    transition:
      background-color var(--dur-fast) var(--ease),
      color var(--dur-fast) var(--ease);
  }
  .collapsed .nav {
    justify-content: center;
    width: var(--control);
    padding: 0;
  }
  .nav:hover {
    background: var(--raised);
    color: var(--text);
  }
  .nav[aria-current="page"] {
    background: var(--raised-2);
    color: var(--text);
    font-weight: 500;
  }
  .nav .lbl {
    flex: 1;
  }
  .count {
    font-size: var(--fs-xs);
    color: var(--text-3);
    font-variant-numeric: tabular-nums;
  }

  .body {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
    border-top: 1px solid var(--line-soft);
  }
  .tabs {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 0.15rem;
    margin: 0.65rem 0.6rem 0.4rem;
    padding: 0.15rem;
    border-radius: var(--radius);
    background: var(--bg);
  }
  .tabs button {
    height: 1.55rem;
    border: 0;
    border-radius: var(--radius-sm);
    background: none;
    font-size: var(--fs-xs);
    font-weight: 500;
    color: var(--text-3);
    transition:
      background-color var(--dur-fast) var(--ease),
      color var(--dur-fast) var(--ease);
  }
  .tabs button:hover {
    color: var(--text);
  }
  .tabs button.on {
    background: var(--raised-2);
    color: var(--text);
    box-shadow: 0 1px 2px oklch(0 0 0 / 0.15);
  }
  .panel {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 0.2rem 0.5rem 0.75rem;
  }

  .empty {
    display: grid;
    justify-items: start;
    gap: 0.6rem;
    padding: 0.6rem 0.5rem;
  }
  .empty p {
    margin: 0;
    color: var(--text-3);
  }
  .imp {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    margin: 0.6rem 0.55rem 0;
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .imp:hover {
    color: var(--accent);
  }
  .empty .imp {
    margin: 0;
  }

  .chats {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .chats li {
    position: relative;
  }
  .chat {
    display: flex;
    align-items: center;
    gap: 0.55rem;
    width: 100%;
    height: 1.9rem;
    padding: 0 0.55rem;
    border: 0;
    border-radius: var(--radius);
    background: none;
    color: var(--text-2);
    text-align: left;
    transition: background-color var(--dur-fast) var(--ease);
  }
  .chat:hover {
    background: var(--raised);
  }
  .chat.current {
    background: var(--raised-2);
    color: var(--text);
    font-weight: 500;
  }
  .name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tag {
    max-width: 6.5rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--fs-xs);
    font-weight: 400;
    color: var(--text-3);
    transition: opacity var(--dur-fast) var(--ease);
  }
  .close {
    position: absolute;
    top: 50%;
    right: 0.25rem;
    transform: translateY(-50%);
    opacity: 0;
    color: var(--text-3);
    transition:
      opacity var(--dur-fast) var(--ease),
      background-color var(--dur-fast) var(--ease);
  }
  .chats li:hover .close,
  .close:focus-visible {
    opacity: 1;
  }
  .chats li:hover .tag {
    opacity: 0;
  }

  .relay {
    flex: none;
    padding: 0.35rem 0.5rem 0.45rem;
    border-top: 1px solid var(--line-soft);
  }
  .fold {
    display: flex;
    align-items: baseline;
    gap: 0.5rem;
    width: 100%;
    padding: 0.35rem 0.55rem;
    border: 0;
    border-radius: var(--radius);
    background: none;
    text-align: left;
    transition: background-color var(--dur-fast) var(--ease);
  }
  .fold:hover {
    background: var(--raised);
  }
  .ftitle {
    font-size: var(--fs-xs);
    font-weight: 600;
    color: var(--text-2);
  }
  .fsub {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .chev {
    align-self: center;
    display: grid;
    color: var(--text-3);
    transform: rotate(-90deg);
    transition: transform var(--dur-base) var(--ease);
  }
  .chev.open {
    transform: rotate(0deg);
  }
  .relay ol {
    margin: 0;
    padding: 0.1rem 0 0;
    list-style: none;
  }
  .relay li {
    display: grid;
    grid-template-columns: 0.9rem auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 0.5rem;
    padding: 0.25rem 0.55rem;
  }
  .num {
    font-family: var(--mono);
    font-size: var(--fs-xs);
    color: var(--text-3);
    font-variant-numeric: tabular-nums;
  }
  .rname {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text);
  }
  .rstate {
    font-size: var(--fs-xs);
    font-variant-numeric: tabular-nums;
    color: var(--text-3);
  }
  .rstate.ok {
    color: var(--ok);
  }
  .rstate.warn {
    color: var(--warn);
  }

  .spacer {
    flex: 1;
  }
  .foot {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    flex: none;
    padding: 0.5rem;
    border-top: 1px solid var(--line-soft);
  }
  .collapsed .foot {
    flex-direction: column-reverse;
    gap: 0.4rem;
    padding: 0.5rem 0.4rem 0.6rem;
  }
  .user {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    flex: 1;
    min-width: 0;
    padding: 0.3rem 0.4rem;
    border: 0;
    border-radius: var(--radius);
    background: none;
    text-align: left;
    transition: background-color var(--dur-fast) var(--ease);
  }
  .collapsed .user {
    flex: none;
    padding: 0.2rem;
  }
  .user:hover {
    background: var(--raised);
  }
  .who {
    display: grid;
    min-width: 0;
    line-height: 1.3;
  }
  .uname {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 500;
  }
  .plan {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
</style>
