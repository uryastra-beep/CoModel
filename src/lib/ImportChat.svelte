<script lang="ts">
  import { importChat, importChatFile, pickSessionFile } from "./backend";
  import { addCard } from "./canvas.svelte";
  import Dialog from "./Dialog.svelte";
  import Icon from "./Icon.svelte";
  import FolderMenu from "./FolderMenu.svelte";
  import { canvas } from "./canvas.svelte";
  import { showCanvas, ui } from "./ui.svelte";

  let url = $state("");
  let projectId = $state<string | null>(canvas.cards.find((c) => c.id === canvas.focusId)?.projectId ?? null);
  let busy = $state(false);
  let error = $state("");

  async function run(close: () => void, fromFile = false) {
    const link = url.trim();
    if (busy || (!fromFile && !link)) return;
    busy = true;
    error = "";
    // El chat se crea ya con su id para que el historial importado esté guardado antes de que se abra.
    const id = `c${Date.now().toString(36)}i`;
    try {
      let r;
      if (fromFile) {
        const path = await pickSessionFile();
        if (!path) return;
        r = await importChatFile(projectId, id, path);
      } else {
        r = await importChat(projectId, id, link);
      }
      showCanvas();
      addCard(undefined, projectId, { id, title: r.title });
      close();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<Dialog title="Importar un chat" subtitle="Trae una conversación entera a un chat nuevo." onclose={() => (ui.importOpen = false)} width="min(28rem, 94vw)">
  {#snippet children(close)}
    <form class="body" onsubmit={(e) => { e.preventDefault(); run(close); }}>
      <p class="lead">Pega el enlace de un chat compartido de ChatGPT, Codex o Claude. Después puedes seguir la conversación con cualquier IA.</p>
      <label>
        <span>Enlace</span>
        <!-- svelte-ignore a11y_autofocus -->
        <input class="input" bind:value={url} placeholder="https://chatgpt.com/share/… o /s/…" autofocus spellcheck="false" autocomplete="off" />
      </label>
      <div class="workspace">
        <span>Carpeta de trabajo del chat</span>
        <FolderMenu {projectId} locked={busy} up={false} onchange={(id) => (projectId = id)} />
        <p class="lead">La conversación usará esta carpeta. Si no eliges una, se creará su workspace dentro de CoModel/Chats al enviar el primer mensaje.</p>
      </div>
      {#if error}<p class="error" role="alert">{error}</p>{/if}
      <button class="btn primary lg" type="submit" disabled={!url.trim() || busy}>{busy ? "Importando…" : "Importar"}</button>
      <div class="or" aria-hidden="true"><span>o</span></div>
      <button type="button" class="btn lg" onclick={() => run(close, true)} disabled={busy}><Icon name="file" size="0.95rem" />Elegir una sesión de Claude Code (.jsonl)</button>
    </form>
  {/snippet}
</Dialog>

<style>
  .body {
    display: grid;
    gap: 0.85rem;
    padding: 1rem 1.35rem 1.2rem;
  }
  .lead {
    margin: 0;
    font-size: var(--fs-sm);
    color: var(--text-3);
  }
  label {
    display: grid;
    gap: 0.3rem;
  }
  label span {
    font-size: var(--fs-xs);
    font-weight: 600;
    color: var(--text-2);
  }
  .workspace { display: grid; gap: 0.35rem; }
  .workspace > span { font-size: var(--fs-xs); color: var(--text-2); font-weight: 600; }
  .error {
    margin: 0;
    font-size: var(--fs-sm);
    color: var(--danger);
  }
  .or {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .or::before,
  .or::after {
    content: "";
    flex: 1;
    height: 1px;
    background: var(--line-soft);
  }
</style>
