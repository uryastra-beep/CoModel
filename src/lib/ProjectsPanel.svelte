<script lang="ts">
  import { inTauri, pickFolder } from "./backend";
  import { canvas } from "./canvas.svelte";
  import Icon from "./Icon.svelte";
  import { rise } from "./motion";
  import { openFolder, projects, recent } from "./projects.svelte";
  import { startChat } from "./start";

  let error = $state("");

  // Más recientes primero; los que nunca se usaron, al final en su orden.
  const ordered = $derived([...projects.list].sort((a, b) => (recent.at[b.id] ?? 0) - (recent.at[a.id] ?? 0)));
  const chatsIn = (id: string) => canvas.cards.filter((c) => c.kind === "chat" && c.projectId === id).length;
  const folderName = (p: string) => p.replace(/[\\/]+$/, "").split(/[\\/]/).pop() || p;

  /** Elegir una carpeta con el selector del sistema y abrir un chat en ella. */
  async function choose() {
    error = "";
    try {
      const path = inTauri ? await pickFolder() : window.prompt("Ruta de la carpeta");
      if (!path) return;
      const p = await openFolder(path.trim());
      startChat("", p.id);
    } catch (e) {
      error = String(e);
    }
  }
</script>

{#if ordered.length === 0}
  <p class="none">Un proyecto es una carpeta tuya en la que trabajan las IAs. Todavía no hay ninguno.</p>
{:else}
  <ul>
    {#each ordered as p (p.id)}
      {@const n = chatsIn(p.id)}
      <li in:rise={{ y: 4, duration: 160 }}>
        <button class="row" onclick={() => startChat("", p.id)} title="Abrir un chat nuevo en {p.name} ({p.path})">
          <Icon name="folder" size="0.95rem" />
          <span class="name">{p.name}</span>
          <span class="meta">{n > 0 ? `${n} ${n === 1 ? "chat" : "chats"}` : folderName(p.path)}</span>
        </button>
      </li>
    {/each}
  </ul>
{/if}
<button class="link add" onclick={choose}><Icon name="plus" size="0.85rem" />Elegir una carpeta</button>
{#if error}<p class="error" role="alert">{error}</p>{/if}

<style>
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .none {
    margin: 0.4rem 0.55rem;
    font-size: var(--fs-sm);
    color: var(--text-3);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 0.55rem;
    width: 100%;
    height: 1.9rem;
    padding: 0 0.55rem;
    border: 0;
    border-radius: var(--radius);
    background: none;
    text-align: left;
    color: var(--text-2);
    transition:
      background-color var(--dur-fast) var(--ease),
      color var(--dur-fast) var(--ease);
  }
  .row :global(.icon) {
    color: var(--text-3);
  }
  .row:hover {
    background: var(--raised);
    color: var(--text);
  }
  .name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta {
    flex: none;
    max-width: 6rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .add {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    margin: 0.6rem 0.55rem 0;
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .add:hover {
    color: var(--accent);
  }
  .error {
    margin: 0.4rem 0.55rem;
    font-size: var(--fs-xs);
    color: var(--danger);
  }
</style>
