<script lang="ts">
  import { onMount } from "svelte";
  import AIPicker from "./AIPicker.svelte";
  import { defaultsFor, type ChatAI } from "./ai.svelte";
  import { inTauri, pickFolder } from "./backend";
  import { canvas } from "./canvas.svelte";
  import { connections, refreshConnections, statusOf } from "./connections.svelte";
  import Curtain from "./Curtain.svelte";
  import FolderMenu from "./FolderMenu.svelte";
  import Icon from "./Icon.svelte";
  import { ms } from "./motion";
  import { profile } from "./profile.svelte";
  import { openFolder, projects } from "./projects.svelte";
  import { startWorkspace } from "./start";
  import { openSettings, showCanvas, ui } from "./ui.svelte";
  import { fade } from "svelte/transition";

  const STEPS_KEY = "comodel.hideStart";
  const SENT_KEY = "comodel.sentFirst";
  const FOLD_KEY = "comodel.stepsFolded";

  const read = (k: string) => {
    try {
      return localStorage.getItem(k) === "1";
    } catch {
      return false;
    }
  };

  let text = $state("");
  let projectId = $state<string | null>(null);
  let ai = $state<ChatAI>(defaultsFor());
  let box: HTMLTextAreaElement;
  let error = $state("");
  let hideSteps = $state(read(STEPS_KEY));
  // Plegado por defecto solo si el usuario lo plegó antes.
  let stepsOpen = $state(!read(FOLD_KEY));
  const sentFirst = read(SENT_KEY);

  onMount(() => {
    if (!connections.loaded) refreshConnections();
    box?.focus();
  });

  const hour = new Date().getHours();
  const greeting = hour < 6 ? "Buenas noches" : hour < 13 ? "Buenos días" : hour < 20 ? "Buenas tardes" : "Buenas noches";
  const workspaces = $derived(canvas.workspaces.length);

  function submit() {
    if (!text.trim()) return;
    startWorkspace(text, projectId, ai);
    text = "";
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey && !e.isComposing) {
      e.preventDefault();
      submit();
    }
  }

  function resize() {
    box.style.height = "auto";
    box.style.height = `${Math.min(box.scrollHeight, 14 * 16)}px`;
  }

  /** Elegir una carpeta con el selector del sistema; queda elegida en la caja de arriba. */
  async function chooseFolder() {
    error = "";
    try {
      const path = inTauri ? await pickFolder() : window.prompt("Ruta de la carpeta");
      if (!path) return;
      const p = await openFolder(path.trim());
      projectId = p.id;
      box?.focus();
    } catch (e) {
      error = String(e);
    }
  }

  const claude = $derived(statusOf("claude"));
  const backup = $derived(statusOf("codex")?.ready || statusOf("openrouter")?.ready);
  const steps = $derived([
    { id: "claude", done: !!claude?.ready, label: "Conectar Claude Code", note: claude?.ready ? "Listo" : "Instálalo y abre sesión con él", action: () => openSettings("ias"), cta: "Ver estado" },
    { id: "folder", done: projects.list.length > 0, label: "Elegir una carpeta de proyecto", note: "Para que las IAs trabajen sobre tus archivos", action: chooseFolder, cta: "Elegir carpeta" },
    { id: "backup", done: !!backup, label: "Conectar una segunda IA", note: "Codex u OpenRouter: sigue el trabajo si la primera se queda sin uso", action: () => openSettings("ias"), cta: "Conectar" },
    { id: "first", done: sentFirst, label: "Mandar tu primer mensaje", note: "Escríbelo arriba y pulsa Enter", action: () => box.focus(), cta: "Escribir" },
  ]);
  const doneCount = $derived(steps.filter((s) => s.done).length);
  const showSteps = $derived(!hideSteps && connections.loaded && doneCount < steps.length);

  function toggleSteps() {
    stepsOpen = !stepsOpen;
    try {
      localStorage.setItem(FOLD_KEY, stepsOpen ? "0" : "1");
    } catch {
      // sin almacenamiento
    }
  }

  function dismiss() {
    hideSteps = true;
    try {
      localStorage.setItem(STEPS_KEY, "1");
    } catch {
      // sin almacenamiento
    }
  }
</script>

<section class="home" aria-label="Inicio" in:fade={{ duration: ms(200) }} out:fade={{ duration: ms(140) }}>
  <div class="col">
    <header>
      <h1>{greeting}, {profile.name}</h1>
      <p>¿En qué trabajamos hoy?</p>
    </header>

    <div class="compose">
      <label class="sr-only" for="home-box">Qué quieres hacer</label>
      <textarea
        id="home-box"
        bind:this={box}
        bind:value={text}
        oninput={resize}
        onkeydown={onKey}
        rows="3"
        placeholder="Pide algo: escribir un texto, revisar un proyecto, arreglar un error…"
      ></textarea>
      <div class="bar">
        <AIPicker {ai} hasFolder={!!projectId} up={false} onchange={(v) => (ai = v)} />
        <span class="grow"></span>
        <button class="send" onclick={submit} disabled={!text.trim()} aria-label="Empezar" title="Empezar (Enter)"><Icon name="send" size="1.05rem" /></button>
      </div>
    </div>
    <div class="under">
      <FolderMenu {projectId} locked={false} up={false} onchange={(id) => (projectId = id)} />
      <span class="grow"></span>
      {#if workspaces > 0}
        <button class="link quiet" onclick={showCanvas}>Volver a tus workspaces ({workspaces})</button>
      {/if}
      <button class="link quiet" onclick={() => (ui.importOpen = true)}>Importar un chat</button>
    </div>
    {#if error}<p class="error" role="alert">{error}</p>{/if}
  </div>

  {#if showSteps}
    <section aria-labelledby="h-steps" class="steps">
      <div class="steps-in">
        <button class="fold" aria-expanded={stepsOpen} aria-controls="steps-body" onclick={toggleSteps}>
          <h2 id="h-steps">Primeros pasos</h2>
          <span class="progress" aria-label="{doneCount} de {steps.length} hechos">
            <span class="track" aria-hidden="true"><span class="fill" style:width="{(doneCount / steps.length) * 100}%"></span></span>
            {doneCount} de {steps.length}
          </span>
          <span class="chev" class:open={stepsOpen} aria-hidden="true"><Icon name="up" size="0.9rem" /></span>
        </button>
        <Curtain open={stepsOpen} id="steps-body">
          <ol>
            {#each steps as s (s.id)}
              <li class:done={s.done}>
                <span class="mark" aria-hidden="true">{#if s.done}<Icon name="check" size="0.7rem" />{/if}</span>
                <span class="what"><span class="label">{s.label}</span><span class="note">{s.note}</span></span>
                {#if !s.done}<button class="btn sm" onclick={s.action}>{s.cta}</button>{:else}<span class="ok">Hecho</span>{/if}
              </li>
            {/each}
          </ol>
          <button class="link quiet hide" onclick={dismiss}>Ocultar esta guía</button>
        </Curtain>
      </div>
    </section>
  {/if}
</section>

<style>
  .home {
    position: absolute;
    inset: 0;
    z-index: 5;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    background: var(--bg);
  }
  .col {
    flex: 1 0 auto;
    display: flex;
    flex-direction: column;
    justify-content: center;
    width: min(42rem, calc(100% - 3rem));
    margin: 0 auto;
    padding: 2rem 0 4rem;
  }
  h1 {
    margin: 0;
    font-size: var(--fs-2xl);
    font-weight: 600;
    letter-spacing: -0.03em;
    line-height: 1.1;
  }
  header p {
    margin: 0.55rem 0 1.75rem;
    font-size: var(--fs-lg);
    color: var(--text-3);
  }

  .compose {
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: var(--radius-xl);
    padding: 0.7rem 0.7rem 0.55rem;
    box-shadow: var(--shadow-card);
    transition:
      border-color var(--dur-fast) var(--ease),
      box-shadow var(--dur-fast) var(--ease);
  }
  .compose:focus-within {
    border-color: color-mix(in oklch, var(--accent) 55%, var(--line));
    box-shadow:
      var(--shadow-card),
      0 0 0 3px color-mix(in oklch, var(--accent) 14%, transparent);
  }
  textarea {
    display: block;
    width: 100%;
    min-height: 4.5rem;
    resize: none;
    border: 0;
    background: transparent;
    padding: 0.25rem 0.4rem;
    font-size: var(--fs-md);
    line-height: 1.55;
  }
  textarea:focus-visible {
    outline: none;
  }
  textarea::placeholder {
    color: var(--text-3);
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    margin-top: 0.35rem;
  }
  .grow {
    flex: 1;
  }
  .send {
    display: grid;
    place-items: center;
    flex: none;
    width: 2rem;
    height: 2rem;
    border: 0;
    border-radius: var(--radius);
    background: var(--accent-solid);
    color: var(--on-accent);
    transition: background-color var(--dur-fast) var(--ease);
  }
  .send:hover:not(:disabled) {
    background: var(--accent-solid-hover);
  }
  .send:disabled {
    background: var(--raised-2);
    color: var(--text-3);
  }
  .under {
    display: flex;
    align-items: center;
    gap: 1rem;
    margin-top: 0.6rem;
    padding: 0 0.3rem;
  }
  .quiet {
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .quiet:hover {
    color: var(--accent);
  }
  .error {
    margin: 0.5rem 0.3rem 0;
    font-size: var(--fs-sm);
    color: var(--danger);
  }

  /* Primeros pasos: pegado al fondo de la pantalla, con el mismo ancho que la caja. */
  .steps {
    margin-top: auto;
    border-top: 1px solid var(--line-soft);
    background: var(--surface);
  }
  .steps-in {
    width: min(42rem, calc(100% - 3rem));
    margin: 0 auto;
    padding: 0.3rem 0 0.6rem;
  }
  .fold {
    display: flex;
    align-items: center;
    gap: 0.9rem;
    width: 100%;
    padding: 0.5rem 0.4rem;
    border: 0;
    border-radius: var(--radius);
    background: none;
    text-align: left;
    transition: background-color var(--dur-fast) var(--ease);
  }
  .fold:hover {
    background: var(--raised);
  }
  h2 {
    margin: 0;
    font-size: var(--fs-sm);
    font-weight: 600;
  }
  .progress {
    display: inline-flex;
    align-items: center;
    gap: 0.55rem;
    flex: 1;
    font-size: var(--fs-xs);
    color: var(--text-3);
    font-variant-numeric: tabular-nums;
  }
  .track {
    width: 4.5rem;
    height: 0.25rem;
    border-radius: 1rem;
    background: var(--raised-2);
    overflow: hidden;
  }
  .fill {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: var(--ok);
    transition: width var(--dur-slow) var(--ease);
  }
  .chev {
    display: grid;
    color: var(--text-3);
    transform: rotate(180deg);
    transition: transform var(--dur-base) var(--ease);
  }
  .chev.open {
    transform: rotate(0deg);
  }
  ol {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .steps li {
    display: grid;
    grid-template-columns: 1.1rem minmax(0, 1fr) auto;
    align-items: center;
    gap: 0.8rem;
    padding: 0.55rem 0.4rem;
    border-top: 1px solid var(--line-soft);
  }
  .mark {
    display: grid;
    place-items: center;
    width: 1rem;
    height: 1rem;
    border: 1.5px solid var(--line);
    border-radius: 50%;
    color: var(--bg);
  }
  .done .mark {
    border-color: var(--ok);
    background: var(--ok);
  }
  .what {
    display: grid;
  }
  .label {
    font-weight: 500;
  }
  .done .label {
    color: var(--text-3);
    font-weight: 400;
  }
  .note {
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .ok {
    font-size: var(--fs-xs);
    color: var(--ok);
  }
  .hide {
    margin: 0.3rem 0.4rem 0;
  }
</style>
