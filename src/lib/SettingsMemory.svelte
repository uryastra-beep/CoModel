<script lang="ts">
  // Trae a CoModel lo que otras IAs recuerdan de ti: se copia un prompt, se pega la respuesta y listo.
  import { onMount } from "svelte";
  import { importMemory, memoryImportText, memoryState, removeMemoryImport, syncMemory, type MemoryState } from "./backend";
  import { copyText } from "./clipboard";
  import Icon from "./Icon.svelte";

  const PROMPT = `Quiero exportar todo lo que sabes de mí para pasárselo a otra IA. Hazlo así:

1. Revisa tu memoria guardada (Ajustes > Personalización > Memoria) y cualquier instrucción personalizada que tenga.
2. Devuélveme UN solo bloque de texto en español, dentro de un bloque de código, con estas secciones:
   - Quién soy (trabajo, intereses, contexto personal que ya sepas)
   - Cómo quiero que me hables y me respondas (tono, longitud, formato, idioma)
   - Preferencias y reglas que te he dado
   - Proyectos y cosas en curso (una línea cada una, con el estado)
   - Herramientas, equipo y software que uso
   - Cosas que NO quiero que hagas
3. Copia mis datos tal cual los tienes. No inventes nada ni rellenes huecos; si algo no lo sabes, no lo pongas.
4. Una línea por dato, sin explicaciones ni introducción. Máximo 3500 caracteres: si no cabe todo, deja lo más importante y útil para trabajar conmigo.
5. No incluyas contraseñas, claves, tokens ni datos bancarios, aunque los tengas.
6. No incluyas reglas que sean propias de una herramienta o plugin tuyo (por ejemplo, preguntas que debas hacer al empezar un chat).`;

  const SOURCES = ["ChatGPT", "Gemini", "Otra IA"];

  let mem = $state<MemoryState | null>(null);
  let source = $state("ChatGPT");
  let custom = $state("");
  let text = $state("");
  let busy = $state(false);
  let copied = $state(false);
  let note = $state<{ ok: boolean; text: string } | null>(null);
  let viewing = $state<{ id: string; text: string } | null>(null);

  const label = $derived(source === "Otra IA" ? custom.trim() : source);

  async function load() {
    try {
      mem = await memoryState();
    } catch (e) {
      note = { ok: false, text: errText(e) };
    }
  }
  onMount(load);

  const errText = (e: unknown) => (e instanceof Error ? e.message : String(e));

  async function copyPrompt() {
    if (await copyText(PROMPT)) {
      copied = true;
      setTimeout(() => (copied = false), 1600);
    }
  }

  async function run() {
    busy = true;
    note = null;
    try {
      const r = await importMemory(label, text);
      text = "";
      note = {
        ok: r.sync.ok || !r.sync.available,
        text: `Memoria de ${r.info.label} importada (${r.info.chars.toLocaleString("es")} caracteres). ${r.sync.available ? (r.sync.ok ? "Claude, Codex y el resto ya la leen." : `No se pudo actualizar la memoria compartida: ${r.sync.message}`) : r.sync.message}`,
      };
    } catch (e) {
      note = { ok: false, text: errText(e) };
    } finally {
      busy = false;
      await load();
    }
  }

  async function view(id: string) {
    if (viewing?.id === id) {
      viewing = null;
      return;
    }
    try {
      viewing = { id, text: await memoryImportText(id) };
    } catch (e) {
      note = { ok: false, text: errText(e) };
    }
  }

  async function remove(id: string) {
    busy = true;
    try {
      await removeMemoryImport(id);
      if (viewing?.id === id) viewing = null;
      note = { ok: true, text: "Memoria quitada." };
    } catch (e) {
      note = { ok: false, text: errText(e) };
    } finally {
      busy = false;
      await load();
    }
  }

  async function refresh() {
    busy = true;
    note = null;
    try {
      const r = await syncMemory();
      note = { ok: r.ok, text: r.ok ? `Memoria compartida al día. ${r.message}` : r.message };
    } catch (e) {
      note = { ok: false, text: errText(e) };
    } finally {
      busy = false;
      await load();
    }
  }

  const when = (s: number) => new Date(s * 1000).toLocaleDateString("es", { day: "numeric", month: "short", year: "numeric" });
</script>

<h2 class="s-title">Memoria</h2>
<p class="s-lead">Cada IA recuerda cosas de ti por su cuenta. Aquí traes lo que otra IA sabe de ti para que todas las que uses en CoModel lo conozcan.</p>

<section class="s-group">
  <h3 class="s-group-title">1. Copia este prompt y pégalo en la otra IA</h3>
  <p class="s-group-desc">Abre un chat nuevo en ChatGPT, Gemini u otra y pégalo. Te devolverá un texto con lo que sabe de ti.</p>
  <div class="prompt">
    <div class="row end">
      <span class="count">Prompt</span>
      <button class="btn sm" onclick={copyPrompt}>
        {#if copied}<Icon name="check" size="0.85rem" />Copiado{:else}<Icon name="copy" size="0.85rem" />Copiar prompt{/if}
      </button>
    </div>
    <pre>{PROMPT}</pre>
  </div>
</section>

<section class="s-group">
  <h3 class="s-group-title">2. Pega aquí lo que te contestó</h3>
  <p class="s-group-desc">Revísalo antes: lo que importes lo leerán todas tus IAs. Si importas otra vez de la misma fuente, reemplaza lo anterior.</p>
  <div class="row">
    <label class="sr-only" for="mem-source">De qué IA viene</label>
    <select id="mem-source" class="input" style:width="9rem" bind:value={source}>
      {#each SOURCES as s (s)}<option value={s}>{s}</option>{/each}
    </select>
    {#if source === "Otra IA"}
      <label class="sr-only" for="mem-custom">Nombre de la IA</label>
      <input id="mem-custom" class="input" style:width="12rem" bind:value={custom} placeholder="Nombre de la IA" maxlength="30" />
    {/if}
  </div>
  <label class="sr-only" for="mem-text">Texto que te dio la IA</label>
  <textarea id="mem-text" class="input paste" bind:value={text} rows="8" placeholder="Pega aquí el texto con tu memoria…" spellcheck="false"></textarea>
  <div class="row end">
    <span class="count">{text.length ? `${text.length.toLocaleString("es")} caracteres` : ""}</span>
    <button class="btn primary" onclick={run} disabled={busy || !text.trim() || !label}>Importar memoria</button>
  </div>
  {#if note}<p class="s-result" class:bad={!note.ok} role={note.ok ? "status" : "alert"}>{note.text}</p>{/if}
</section>

<section class="s-group">
  <h3 class="s-group-title">Memorias importadas</h3>
  {#if !mem}
    <p class="s-group-desc">Cargando…</p>
  {:else if mem.imports.length === 0}
    <p class="s-group-desc">Todavía no has importado ninguna.</p>
  {:else}
    <div class="s-rows">
      {#each mem.imports as m (m.id)}
        <div class="s-row">
          <span class="s-row-text">
            <span class="s-label">{m.label}</span>
            <span class="s-desc">{m.chars.toLocaleString("es")} caracteres · {when(m.updated)}</span>
          </span>
          <span class="s-control">
            <button class="btn sm" onclick={() => view(m.id)}>{viewing?.id === m.id ? "Ocultar" : "Ver"}</button>
            <button class="btn ghost sm danger" onclick={() => remove(m.id)} disabled={busy}>Quitar</button>
          </span>
        </div>
        {#if viewing?.id === m.id}<pre class="view">{viewing.text}</pre>{/if}
      {/each}
    </div>
  {/if}
</section>

<section class="s-group">
  <h3 class="s-group-title">Memoria compartida</h3>
  <p class="s-group-desc">
    Junta lo de Claude, Codex y estas fuentes en un solo texto que leen todas tus IAs.
    {#if mem?.shared_chars}Ahora ocupa unos {mem.shared_chars.toLocaleString("es")} caracteres.{/if}
    {#if mem && !mem.sync_available}<span class="warn"> No se encontró <code>ai-memory-sync</code>: lo importado se guarda, pero Claude y Codex no lo leerán hasta instalarlo.</span>{/if}
  </p>
  <button class="btn" onclick={refresh} disabled={busy || (mem !== null && !mem.sync_available)}>Actualizar ahora</button>
</section>

<style>
  .prompt pre,
  .view {
    margin: 0;
    padding: 0.8rem 0.9rem;
    max-height: 14rem;
    overflow: auto;
    border: 1px solid var(--line-soft);
    border-radius: var(--radius-lg);
    background: var(--bg);
    font-family: var(--mono);
    font-size: var(--fs-xs);
    line-height: 1.55;
    color: var(--text-2);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .view {
    margin: 0 0.6rem 0.7rem;
    max-height: 16rem;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-bottom: 0.5rem;
  }
  .row.end {
    justify-content: space-between;
    margin: 0.5rem 0 0;
  }
  .prompt .row.end {
    margin: 0 0 0.4rem;
  }
  .paste {
    height: auto;
    padding: 0.6rem 0.7rem;
    line-height: 1.5;
    resize: vertical;
    font-family: var(--mono);
    font-size: var(--fs-xs);
  }
  .count {
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .warn {
    color: var(--warn);
  }
</style>
