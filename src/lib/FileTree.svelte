<script lang="ts">
  import { untrack } from "svelte";
  import { gitStatus, listDir, type DirEntry } from "./backend";
  import { filesChanged, openFileCard } from "./canvas.svelte";
  import Curtain from "./Curtain.svelte";
  import Icon from "./Icon.svelte";
  import { showCanvas } from "./ui.svelte";

  let { projectId }: { projectId: string } = $props();

  let tree = $state<Record<string, DirEntry[]>>({});
  let expanded = $state<Record<string, boolean>>({});
  let changes = $state<Record<string, string>>({});
  let error = $state("");
  let loading = $state(true);

  const join = (dir: string, name: string) => (dir ? `${dir}/${name}` : name);

  async function loadDir(path: string) {
    try {
      tree[path] = await listDir(projectId, path);
      error = "";
    } catch (e) {
      error = String(e);
    }
  }

  async function loadStatus() {
    try {
      const list = await gitStatus(projectId);
      changes = Object.fromEntries(list.map((c) => [c.path, c.status]));
    } catch {
      changes = {};
    }
  }

  /** Vuelve a leer lo que está abierto, sin cerrar las carpetas desplegadas. */
  async function refresh() {
    await Promise.all([...Object.keys(tree).map(loadDir), loadStatus()]);
  }

  // Al cambiar de proyecto se empieza de cero. Solo `projectId` es una dependencia: lo demás se escribe
  // aquí mismo, así que se aísla con `untrack` (si no, cada carga volvería a lanzar el efecto sin fin).
  $effect(() => {
    void projectId;
    untrack(() => {
      tree = {};
      expanded = {};
      changes = {};
      loading = true;
      Promise.all([loadDir(""), loadStatus()]).finally(() => (loading = false));
    });
  });

  // Una IA o el editor cambiaron algo.
  let seen = 0;
  $effect(() => {
    const v = filesChanged.version;
    untrack(() => {
      if (v !== seen && !loading) refresh();
    });
    seen = v;
  });

  async function toggle(path: string) {
    if (!expanded[path] && !tree[path]) await loadDir(path);
    expanded[path] = !expanded[path];
  }

  function open(path: string) {
    showCanvas();
    openFileCard(projectId, path);
  }

  const hasChanges = (dir: string) => Object.keys(changes).some((p) => p.startsWith(`${dir}/`));
  const statusColor = (s: string) => (s === "D" ? "bad" : s === "M" || s === "R" ? "warn" : "ok");
</script>

{#snippet branch(dir: string, depth: number)}
  <ul role={depth === 0 ? "tree" : "group"} aria-label={depth === 0 ? "Archivos del proyecto" : undefined}>
    {#each tree[dir] ?? [] as e (e.name)}
      {@const path = join(dir, e.name)}
      <li role="none">
        {#if e.is_dir}
          <button role="treeitem" aria-expanded={!!expanded[path]} aria-selected="false" class="row" style:padding-left="{0.4 + depth * 0.95}rem" onclick={() => toggle(path)}>
            <span class="caret" class:open={expanded[path]} aria-hidden="true"><Icon name="right" size="0.8rem" /></span>
            <span class="n">{e.name}</span>
            {#if hasChanges(path)}<span class="dotc" title="Contiene cambios" aria-label="Contiene cambios"></span>{/if}
          </button>
          {#if tree[path]}
            <Curtain open={!!expanded[path]}>
              {@render branch(path, depth + 1)}
            </Curtain>
          {/if}
        {:else}
          <button role="treeitem" aria-selected="false" class="row file" style:padding-left="{0.4 + depth * 0.95 + 1.2}rem" onclick={() => open(path)}>
            <span class="n">{e.name}</span>
            {#if changes[path]}
              <span class="st {statusColor(changes[path])}" title={{ M: "Modificado", A: "Nuevo", "?": "Sin seguimiento", D: "Borrado", R: "Renombrado" }[changes[path]] ?? ""}>{changes[path] === "?" ? "N" : changes[path]}</span>
            {/if}
          </button>
        {/if}
      </li>
    {/each}
  </ul>
{/snippet}

{#if loading}
  <p class="msg">Leyendo la carpeta…</p>
{:else if error}
  <p class="msg bad" role="alert">{error}</p>
{:else if (tree[""] ?? []).length === 0}
  <p class="msg">La carpeta está vacía.</p>
{:else}
  {@render branch("", 0)}
{/if}

<style>
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    width: 100%;
    min-height: 1.75rem;
    padding-right: 0.5rem;
    border: 0;
    border-radius: var(--radius);
    background: none;
    text-align: left;
    color: var(--text-2);
    transition: background-color 120ms var(--ease), color 120ms var(--ease);
  }
  .row:hover {
    background: var(--raised);
    color: var(--text);
  }
  .n {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .caret {
    display: grid;
    flex: none;
    color: var(--text-3);
    transition: transform var(--dur-base) var(--ease);
  }
  .caret.open {
    transform: rotate(90deg);
  }
  .file .n {
    font-family: var(--mono);
    font-size: var(--fs-xs);
  }
  .dotc {
    flex: none;
    width: 0.4rem;
    height: 0.4rem;
    border-radius: 50%;
    background: var(--warn);
  }
  .st {
    flex: none;
    font-family: var(--mono);
    font-size: var(--fs-xs);
    font-weight: 600;
  }
  .st.warn {
    color: var(--warn);
  }
  .st.ok {
    color: var(--ok);
  }
  .st.bad {
    color: var(--danger);
  }
  .msg {
    margin: 0.35rem 0.5rem;
    font-size: var(--fs-sm);
    color: var(--text-3);
  }
  .msg.bad {
    color: var(--danger);
  }
</style>
