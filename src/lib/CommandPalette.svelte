<script lang="ts">
  // Paleta de búsqueda y acciones (Ctrl+K): ir a un chat, empezar uno en un proyecto o lanzar una acción sin el ratón.
  import { onMount, tick } from "svelte";
  import { canvas, centerOn, fitAll, openPanel, switchSpace, workspaceById } from "./canvas.svelte";
  import Icon, { type IconName } from "./Icon.svelte";
  import { ms } from "./motion";
  import { projectById, projects } from "./projects.svelte";
  import { startChat, startWorkspace } from "./start";
  import { setTheme, THEMES } from "./theme";
  import { openSettings, showCanvas, showHome, toggleSidebar, ui } from "./ui.svelte";

  interface Item {
    id: string;
    group: string;
    label: string;
    hint?: string;
    keys?: string;
    icon?: IconName;
    dot?: string;
    run: () => void;
  }

  let dialog: HTMLDialogElement;
  let input = $state<HTMLInputElement>();
  let query = $state("");
  let active = $state(0);
  let closing = $state(false);

  const items = $derived.by<Item[]>(() => {
    const list: Item[] = [];
    for (const w of canvas.workspaces) {
      list.push({ id: `ws-${w.id}`, group: "Workspaces", label: w.name, hint: projectById(w.projectId)?.name ?? "Sin carpeta todavía", icon: "canvas", run: () => { showCanvas(); switchSpace(w.id); } });
    }
    for (const c of canvas.cards.filter((c) => c.kind === "chat" || c.kind === "file")) {
      const proj = projectById(c.projectId);
      list.push({
        id: `card-${c.id}`,
        group: c.kind === "chat" ? "Chats abiertos" : "Archivos abiertos",
        label: c.title,
        hint: c.kind === "file" ? (c.path ?? "") : (workspaceById(c.space)?.name ?? proj?.name ?? "Sin carpeta"),
        icon: c.kind === "file" ? "file" : undefined,
        dot: c.kind === "chat" ? (c.ai?.provider ?? "claude") : undefined,
        run: () => {
          showCanvas();
          centerOn(c.id);
        },
      });
    }
    list.push(
      { id: "new", group: "Acciones", label: "Nuevo chat", hint: "En el workspace actual", keys: "N", icon: "new-chat", run: () => startChat("", null) },
      { id: "term", group: "Acciones", label: "Abrir una terminal", hint: "En la carpeta del workspace", icon: "terminal", run: () => { openPanel("terminal"); showCanvas(); } },
      { id: "new-ws", group: "Acciones", label: "Nuevo workspace", icon: "new-chat", run: () => startWorkspace() },
      { id: "home", group: "Acciones", label: "Ir a Inicio", icon: "home", run: showHome },
      { id: "canvas", group: "Acciones", label: "Ir al lienzo", icon: "canvas", run: showCanvas },
      {
        id: "fit",
        group: "Acciones",
        label: "Encuadrar todos los chats del workspace",
        icon: "fit",
        run: () => {
          showCanvas();
          fitAll();
        },
      },
      { id: "import", group: "Acciones", label: "Importar un chat", hint: "Desde un enlace de ChatGPT o Claude, o un archivo de Claude Code", icon: "import", run: () => (ui.importOpen = true) },
      { id: "sidebar", group: "Acciones", label: ui.sidebarCollapsed ? "Mostrar la barra lateral" : "Ocultar la barra lateral", keys: "Ctrl B", icon: "sidebar", run: toggleSidebar },
      { id: "settings", group: "Ajustes", label: "Abrir Ajustes", keys: "Ctrl ,", icon: "settings", run: () => openSettings("perfil") },
      { id: "ias", group: "Ajustes", label: "Conectar IAs", hint: "Claude, Codex y OpenRouter", icon: "plug", run: () => openSettings("ias") },
      { id: "usage", group: "Ajustes", label: "Ver el uso de las IAs", icon: "chart", run: () => openSettings("uso") },
      { id: "keys", group: "Ajustes", label: "Atajos de teclado", icon: "keyboard", run: () => openSettings("atajos") },
    );
    for (const t of THEMES) list.push({ id: `theme-${t.id}`, group: "Ajustes", label: `Tema: ${t.label}`, hint: t.hint, icon: "palette", run: () => setTheme(t.id) });
    for (const p of projects.list) list.push({ id: `proj-${p.id}`, group: "Proyectos", label: `Nuevo chat en ${p.name}`, hint: p.path, icon: "folder", run: () => startChat("", p.id) });
    return list;
  });

  const norm = (s: string) => s.normalize("NFD").replace(/\p{Diacritic}/gu, "").toLowerCase();
  const shown = $derived.by(() => {
    const q = norm(query.trim());
    if (!q) return items;
    const words = q.split(/\s+/);
    return items.filter((it) => {
      const hay = norm(`${it.label} ${it.hint ?? ""} ${it.group}`);
      return words.every((w) => hay.includes(w));
    });
  });

  // Grupos en el orden en que aparecen, para pintar sus títulos.
  const groups = $derived.by(() => {
    const out: { name: string; items: { it: Item; i: number }[] }[] = [];
    shown.forEach((it, i) => {
      let g = out.find((x) => x.name === it.group);
      if (!g) out.push((g = { name: it.group, items: [] }));
      g.items.push({ it, i });
    });
    return out;
  });

  $effect(() => {
    void query;
    active = 0;
  });

  $effect(() => {
    document.getElementById(`pal-${shown[active]?.id}`)?.scrollIntoView({ block: "nearest" });
  });

  onMount(() => {
    dialog.showModal();
    tick().then(() => input?.focus());
    return () => {
      if (dialog.open) dialog.close();
    };
  });

  function close() {
    if (closing) return;
    closing = true;
    setTimeout(() => {
      if (dialog?.open) dialog.close();
    }, ms(130) + 40);
  }

  function run(it: Item | undefined) {
    if (!it) return;
    it.run();
    close();
  }

  function onkeydown(e: KeyboardEvent) {
    const n = shown.length;
    if (e.key === "ArrowDown") active = n ? (active + 1) % n : 0;
    else if (e.key === "ArrowUp") active = n ? (active - 1 + n) % n : 0;
    else if (e.key === "Enter") run(shown[active]);
    else return;
    e.preventDefault();
  }
</script>

<dialog
  bind:this={dialog}
  class:closing
  aria-label="Buscar chats y acciones"
  onclose={() => (ui.paletteOpen = false)}
  oncancel={(e) => {
    e.preventDefault();
    close();
  }}
  onclick={(e) => {
    if (e.target === dialog) close();
  }}
>
  <div class="box">
    <div class="field">
      <Icon name="search" size="1.05rem" />
      <input
        bind:this={input}
        bind:value={query}
        placeholder="Busca un chat, un proyecto o una acción"
        aria-label="Buscar"
        aria-controls="pal-list"
        aria-activedescendant={shown[active] ? `pal-${shown[active].id}` : undefined}
        role="combobox"
        aria-expanded="true"
        spellcheck="false"
        autocomplete="off"
        {onkeydown}
      />
      <kbd>Esc</kbd>
    </div>
    <div class="list" id="pal-list" role="listbox" aria-label="Resultados">
      {#each groups as g (g.name)}
        <div class="group" role="group" aria-label={g.name}>
          <div class="gname" aria-hidden="true">{g.name}</div>
          {#each g.items as { it, i } (it.id)}
            <div
              id="pal-{it.id}"
              class="item"
              class:active={i === active}
              role="option"
              tabindex="-1"
              aria-selected={i === active}
              onpointermove={() => (active = i)}
              onclick={() => run(it)}
              onkeydown={() => {}}
            >
              <span class="ico" aria-hidden="true">
                {#if it.dot}<span class="pdot" data-p={it.dot}></span>{:else if it.icon}<Icon name={it.icon} size="1rem" />{/if}
              </span>
              <span class="text">
                <span class="label">{it.label}</span>
                {#if it.hint}<span class="hint">{it.hint}</span>{/if}
              </span>
              {#if it.keys}<kbd>{it.keys}</kbd>{/if}
            </div>
          {/each}
        </div>
      {:else}
        <p class="none">Nada coincide con «{query}».</p>
      {/each}
    </div>
    <div class="foot" aria-hidden="true">
      <span><kbd>↑</kbd><kbd>↓</kbd> moverse</span>
      <span><kbd>Enter</kbd> abrir</span>
      <span><kbd>Esc</kbd> cerrar</span>
    </div>
  </div>
</dialog>

<style>
  dialog {
    width: min(38rem, 92vw);
    max-width: none;
    max-height: none;
    margin: 12vh auto auto;
    padding: 0;
    border: 1px solid var(--line);
    border-radius: var(--radius-xl);
    background: var(--surface);
    color: var(--text);
    box-shadow: var(--lift);
    animation: pal-in var(--dur-base) var(--ease) both;
  }
  dialog::backdrop {
    background: var(--scrim);
    animation: fade-in var(--dur-base) var(--ease) both;
  }
  dialog.closing {
    animation: pal-out var(--dur-out) var(--ease-in) both;
  }
  dialog.closing::backdrop {
    animation: fade-out var(--dur-out) var(--ease-in) both;
  }
  @keyframes pal-in {
    from {
      opacity: 0;
      transform: translateY(-0.5rem) scale(0.985);
    }
  }
  @keyframes pal-out {
    to {
      opacity: 0;
      transform: translateY(-0.3rem) scale(0.99);
    }
  }
  @keyframes fade-in {
    from {
      opacity: 0;
    }
  }
  @keyframes fade-out {
    to {
      opacity: 0;
    }
  }
  .box {
    display: flex;
    flex-direction: column;
    max-height: min(32rem, 70vh);
  }
  .field {
    display: flex;
    align-items: center;
    gap: 0.65rem;
    padding: 0 0.9rem;
    height: 3.1rem;
    border-bottom: 1px solid var(--line-soft);
    color: var(--text-3);
  }
  .field input {
    flex: 1;
    min-width: 0;
    border: 0;
    background: none;
    font-size: var(--fs-md);
    color: var(--text);
  }
  .field input:focus {
    outline: none;
  }
  .field input::placeholder {
    color: var(--text-3);
  }
  .list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overscroll-behavior: contain;
    padding: 0.35rem;
  }
  .group + .group {
    margin-top: 0.35rem;
  }
  .gname {
    padding: 0.45rem 0.6rem 0.25rem;
    font-size: var(--fs-xs);
    font-weight: 500;
    color: var(--text-3);
  }
  .item {
    display: grid;
    grid-template-columns: 1.25rem minmax(0, 1fr) auto;
    align-items: center;
    gap: 0.65rem;
    min-height: 2.25rem;
    padding: 0.3rem 0.6rem;
    border-radius: var(--radius);
    cursor: pointer;
  }
  .item.active {
    background: var(--raised-2);
  }
  .ico {
    display: grid;
    place-items: center;
    color: var(--text-3);
  }
  .item.active .ico {
    color: var(--text-2);
  }
  .text {
    display: flex;
    align-items: baseline;
    gap: 0.6rem;
    min-width: 0;
  }
  .label {
    flex: none;
    max-width: 70%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .hint {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .none {
    margin: 1.2rem 0.8rem;
    color: var(--text-3);
    text-align: center;
  }
  .foot {
    display: flex;
    gap: 1.1rem;
    padding: 0.5rem 0.9rem;
    border-top: 1px solid var(--line-soft);
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .foot span {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
  }
</style>
