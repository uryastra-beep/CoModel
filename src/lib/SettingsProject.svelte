<script lang="ts">
  import { removeCardsOfProject } from "./canvas.svelte";
  import Curtain from "./Curtain.svelte";
  import Icon from "./Icon.svelte";
  import { deleteProject, editProject, projects } from "./projects.svelte";
  import RelayEditor from "./RelayEditor.svelte";

  let openId = $state<string | null>(null);
  let confirmId = $state<string | null>(null);
  let error = $state("");

  async function run(action: () => Promise<void>) {
    error = "";
    try {
      await action();
    } catch (e) {
      error = String(e);
    }
  }

  const folder = (p: string) => p.replace(/[\\/]+$/, "").split(/[\\/]/).pop() || p;
</script>

<h2 class="s-title">Proyectos</h2>
<p class="s-lead">Un proyecto es una carpeta tuya en la que trabajan las IAs, con su propia memoria compartida y su orden de relevo. Se crea al elegir una carpeta debajo del cuadro de texto de un chat.</p>

{#if projects.list.length === 0}
  <p class="empty">Todavía no hay proyectos. Abre un chat y elige una carpeta en «Sin carpeta».</p>
{:else}
  <ul class="list s-rows">
    {#each projects.list as p (p.id)}
      {@const open = openId === p.id}
      <li>
        <button class="head" aria-expanded={open} aria-controls="proj-{p.id}" onclick={() => (openId = open ? null : p.id)}>
          <Icon name="folder" size="1rem" />
          <span class="name">{p.name}</span>
          <span class="path" title={p.path}>{folder(p.path)}</span>
          <span class="chev" class:open aria-hidden="true"><Icon name="down" size="0.9rem" /></span>
        </button>

        <Curtain {open} id="proj-{p.id}">
          <div class="body">
            <label class="field">
              <span class="lbl">Nombre</span>
              <input class="input" value={p.name} maxlength="60" onchange={(e) => run(() => editProject(p.id, e.currentTarget.value, null))} />
            </label>

            <div class="field">
              <span class="lbl">Carpeta</span>
              <code class="full" title={p.path}>{p.path}</code>
              <span class="hint">La memoria compartida está en <code>.comodel/MEMORY.md</code>.</span>
            </div>

            <fieldset>
              <legend>IAs y orden de relevo</legend>
              <p class="hint">Cambiar esto reinicia las conversaciones abiertas del proyecto, sin perder su historial.</p>
              <RelayEditor relay={p.relay} onchange={(r) => run(() => editProject(p.id, null, r))} />
            </fieldset>

            <div class="danger">
              {#if confirmId !== p.id}
                <button class="btn sm danger" onclick={() => (confirmId = p.id)}>Quitar de CoModel</button>
              {:else}
                <p class="hint">Se quita de la lista y se cierran sus chats abiertos. No se borra ningún archivo de la carpeta.</p>
                <div class="row">
                  <button
                    class="btn sm danger"
                    onclick={() =>
                      run(async () => {
                        removeCardsOfProject(p.id);
                        await deleteProject(p.id);
                        confirmId = null;
                        openId = null;
                      })}>Quitar proyecto</button
                  >
                  <button class="btn sm ghost" onclick={() => (confirmId = null)}>Cancelar</button>
                </div>
              {/if}
            </div>
          </div>
        </Curtain>
      </li>
    {/each}
  </ul>
{/if}

{#if error}<p class="error" role="alert">{error}</p>{/if}

<style>
  .empty {
    color: var(--text-3);
  }
  .list {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  li + li {
    border-top: 1px solid var(--line-soft);
  }
  .head {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) minmax(0, 14rem) auto;
    align-items: center;
    gap: 0.7rem;
    width: 100%;
    min-height: 3rem;
    padding: 0.5rem 0.95rem;
    border: 0;
    background: none;
    text-align: left;
    color: var(--text-3);
    transition: background-color var(--dur-fast) var(--ease);
  }
  li:first-child .head {
    border-radius: var(--radius-lg) var(--radius-lg) 0 0;
  }
  li:last-child .head[aria-expanded="false"] {
    border-radius: 0 0 var(--radius-lg) var(--radius-lg);
  }
  .head:hover {
    background: var(--raised);
  }
  .name {
    font-weight: 500;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--mono);
    font-size: var(--fs-xs);
    color: var(--text-3);
    text-align: right;
  }
  .chev {
    display: grid;
    color: var(--text-3);
    transition: transform var(--dur-base) var(--ease);
  }
  .chev.open {
    transform: rotate(180deg);
  }
  .body {
    display: grid;
    gap: 1rem;
    padding: 0.5rem 0.95rem 1.1rem 2.65rem;
  }
  .field {
    display: grid;
    gap: 0.35rem;
    max-width: 32rem;
  }
  .lbl,
  legend {
    padding: 0;
    font-size: var(--fs-xs);
    font-weight: 600;
    color: var(--text-2);
  }
  .full {
    padding: 0.4rem 0.6rem;
    background: var(--surface);
    border: 1px solid var(--line-soft);
    border-radius: var(--radius);
    font-family: var(--mono);
    font-size: var(--fs-xs);
    color: var(--text-2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .hint {
    margin: 0;
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .hint code {
    font-family: var(--mono);
  }
  fieldset {
    display: grid;
    gap: 0.4rem;
    max-width: 32rem;
    margin: 0;
    padding: 0;
    border: 0;
  }
  .error {
    color: var(--danger);
    font-size: var(--fs-sm);
  }
  .danger {
    max-width: 32rem;
    padding-top: 0.9rem;
    border-top: 1px solid var(--line-soft);
  }
  .row {
    display: flex;
    gap: 0.5rem;
    margin-top: 0.5rem;
  }
</style>
