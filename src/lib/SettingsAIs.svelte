<script lang="ts">
  import { onMount } from "svelte";
  import { removeApiKey, setApiKey, testConnection } from "./backend";
  import { connections, refreshConnections, statusOf } from "./connections.svelte";
  import Icon from "./Icon.svelte";
  import ChatGPTConnection from "./ChatGPTConnection.svelte";
  import { changeDefaultRelay, projects } from "./projects.svelte";
  import { RELAY } from "./providers";
  import RelayEditor from "./RelayEditor.svelte";

  type Result = { ok: boolean; text: string };
  let results = $state<Record<string, Result>>({});
  let busy = $state<string | null>(null);
  let key = $state("");

  onMount(() => {
    refreshConnections();
  });

  async function run(id: string, action: () => Promise<string | void>) {
    busy = id;
    try {
      const text = await action();
      results[id] = { ok: true, text: text ?? "" };
    } catch (e) {
      results[id] = { ok: false, text: String(e) };
    } finally {
      busy = null;
      await refreshConnections();
    }
  }

  const test = (id: string) => run(id, () => testConnection(id));

  const save = () =>
    run("openrouter", async () => {
      await setApiKey("openrouter", key);
      key = ""; // la clave no se queda en pantalla
      return "Clave guardada en el llavero del sistema.";
    });

  const remove = () =>
    run("openrouter", async () => {
      await removeApiKey("openrouter");
      return "Clave quitada.";
    });

  const DESC: Record<string, string> = {
    claude: "Usa tu sesión de Claude Code, sin clave. Trabaja con tus archivos y pide permiso dentro del chat.",
    chatgpt: "Conversa, analiza imágenes y lee PDF, documentos, hojas de cálculo y texto adjuntos. Usa tu plan de ChatGPT con autorización propia; no ejecuta comandos ni cambia archivos.",
    codex: "Usa la sesión de ChatGPT de la app de Codex, sin clave. Sus modelos y esfuerzos son los de tu cuenta.",
    openrouter: "Una sola clave para cientos de modelos (OpenAI, Google, DeepSeek, Meta…). Conversa, pero no abre archivos.",
    gemini: "Todavía no está disponible.",
  };
</script>

<h2 class="s-title">IAs</h2>
<p class="s-lead">Conecta las IAs que quieras usar. En cada chat eliges cuál responde; si se queda sin uso, sigue la siguiente del relevo.</p>

<section class="s-group">
  <div class="s-rows">
    {#each RELAY as r (r.id)}
      {@const st = statusOf(r.id)}
      {@const state = !connections.loaded ? "checking" : st && !st.available ? "soon" : st?.ready ? "ready" : "off"}
      <div class="prov" class:soon={state === "soon"}>
        <div class="phead">
          <span class="pdot" data-p={r.id} aria-hidden="true"></span>
          <span class="name">{r.label}</span>
          <span class="via">{r.via}</span>
          <span class="badge {state}">
            {#if state === "ready"}<Icon name="check" size="0.75rem" />Conectada{:else if state === "checking"}Comprobando{:else if state === "soon"}Próximamente{:else}Sin conectar{/if}
          </span>
          <span class="grow"></span>
          {#if r.id === "claude" || r.id === "codex"}
            <button class="btn sm" onclick={() => test(r.id)} disabled={busy === r.id}>{busy === r.id ? "Probando…" : "Probar"}</button>
          {:else if r.id === "openrouter" && st?.ready}
            <button class="btn sm" onclick={() => test("openrouter")} disabled={busy === "openrouter"}>Probar</button>
            <button class="btn ghost sm danger" onclick={remove} disabled={busy === "openrouter"}>Quitar clave</button>
          {/if}
        </div>
        <p class="desc">
          {DESC[r.id] ?? ""}
          {#if state === "off" && st?.detail && r.id !== "openrouter"}<span class="warn">{st.detail}</span>{/if}
          {#if state === "ready" && r.id === "claude" && st?.detail}<span class="ver">Versión {st.detail}.</span>{/if}
        </p>

        {#if r.id === "chatgpt"}
          <ChatGPTConnection />
        {:else if r.id === "openrouter"}
          <form
            class="key"
            onsubmit={(e) => {
              e.preventDefault();
              save();
            }}
          >
            <label class="sr-only" for="or-key">Clave de API de OpenRouter</label>
            <input
              id="or-key"
              class="input mono"
              type="password"
              bind:value={key}
              placeholder={st?.ready ? "Clave guardada. Pega otra para reemplazarla" : "Pega tu clave de OpenRouter (sk-or-…)"}
              autocomplete="off"
              spellcheck="false"
              maxlength="400"
            />
            <button class="btn primary" type="submit" disabled={!key.trim() || busy === "openrouter"}>Guardar</button>
          </form>
          <p class="note"><Icon name="shield" size="0.8rem" />Se guarda en el llavero del sistema, nunca en archivos, y la app no puede volver a mostrarla.</p>
        {:else if r.id === "codex"}
          <p class="note"><Icon name="shield" size="0.8rem" />En un proyecto escribe solo dentro de su carpeta; en modo «Solo leer» o sin carpeta, solo lee. No pide permisos a mitad del mensaje.</p>
        {/if}

        {#if results[r.id]}
          <p class="s-result" class:bad={!results[r.id].ok} role="status">{results[r.id].text}</p>
        {/if}
      </div>
    {/each}
  </div>
</section>

<section class="s-group">
  <h3 class="s-group-title">Relevo de los chats sin carpeta</h3>
  <p class="s-group-desc">Si la IA de un chat se queda sin uso, sigue la primera disponible de esta lista. Los chats con carpeta usan el orden de su proyecto.</p>
  <RelayEditor relay={projects.defaultRelay} onchange={(r) => run("default", async () => { await changeDefaultRelay(r); })} />
  {#if results.default && !results.default.ok}<p class="s-result bad" role="alert">{results.default.text}</p>{/if}
</section>

<style>
  .prov {
    padding: 0.85rem 0.95rem 0.9rem;
  }
  .prov + .prov {
    border-top: 1px solid var(--line-soft);
  }
  .prov.soon {
    opacity: 0.6;
  }
  .phead {
    display: flex;
    align-items: center;
    gap: 0.55rem;
    min-height: var(--control-sm);
  }
  .name {
    font-weight: 600;
    font-size: var(--fs-md);
  }
  .via {
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .badge {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    margin-left: 0.25rem;
    padding: 0.05rem 0.45rem;
    border-radius: 1rem;
    border: 1px solid var(--line-soft);
    font-size: 0.6875rem;
    font-weight: 500;
    color: var(--text-3);
  }
  .badge.ready {
    color: var(--ok);
    border-color: color-mix(in oklch, var(--ok) 35%, var(--line-soft));
    background: color-mix(in oklch, var(--ok) 8%, transparent);
  }
  .badge.off {
    color: var(--warn);
    border-color: color-mix(in oklch, var(--warn) 35%, var(--line-soft));
  }
  .grow {
    flex: 1;
  }
  .desc {
    margin: 0.35rem 0 0;
    max-width: 62ch;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .desc .warn {
    display: block;
    margin-top: 0.2rem;
    color: var(--warn);
  }
  .ver {
    color: var(--text-3);
  }
  .key {
    display: flex;
    gap: 0.5rem;
    max-width: 34rem;
    margin-top: 0.7rem;
  }
  .note {
    display: flex;
    align-items: flex-start;
    gap: 0.4rem;
    margin: 0.5rem 0 0;
    max-width: 62ch;
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .note :global(.icon) {
    margin-top: 0.12rem;
  }
</style>
