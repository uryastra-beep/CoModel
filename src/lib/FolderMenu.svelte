<script lang="ts">
  import { inTauri, pickFolder } from "./backend";
  import Icon from "./Icon.svelte";
  import { popIn, popOut } from "./motion";
  import { openFolder, projectById, projects } from "./projects.svelte";

  interface Props {
    projectId: string | null;
    /** Con la conversación empezada la carpeta ya no se cambia. */
    locked: boolean;
    onchange: (projectId: string | null) => void;
    /** Abre hacia arriba (por defecto) o hacia abajo. */
    up?: boolean;
  }
  let { projectId, locked, onchange, up = true }: Props = $props();

  let open = $state(false);
  let error = $state("");
  let root: HTMLDivElement;
  let trigger: HTMLButtonElement;

  const current = $derived(projectById(projectId));
  const folder = (p: string) => p.replace(/[\\/]+$/, "").split(/[\\/]/).pop() || p;

  $effect(() => {
    if (!open) return;
    const close = (e: PointerEvent) => {
      if (!root.contains(e.target as Node)) open = false;
    };
    const esc = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        open = false;
        trigger.focus();
      }
    };
    window.addEventListener("pointerdown", close, true);
    window.addEventListener("keydown", esc, true);
    return () => {
      window.removeEventListener("pointerdown", close, true);
      window.removeEventListener("keydown", esc, true);
    };
  });

  function choose(id: string | null) {
    open = false;
    error = "";
    onchange(id);
  }

  /** Selector nativo del sistema. La carpeta pasa a ser un proyecto con su nombre y el relevo por defecto. */
  async function pick() {
    error = "";
    try {
      // En el navegador (sin selector nativo) se pide la ruta con un cuadro simple, solo para poder probar.
      const path = inTauri ? await pickFolder() : window.prompt("Ruta de la carpeta");
      if (!path) return;
      const p = await openFolder(path.trim());
      open = false;
      onchange(p.id);
    } catch (e) {
      error = String(e);
    }
  }
</script>

<div class="wrap" bind:this={root}>
  <button
    bind:this={trigger}
    type="button"
    class="trigger"
    class:set={!!current}
    disabled={locked}
    aria-haspopup="menu"
    aria-expanded={open}
    title={locked ? "La carpeta se elige antes del primer mensaje. Abre un chat nuevo para usar otra." : current ? current.path : "Elige una carpeta para que la IA trabaje sobre tus archivos"}
    onclick={() => (open = !open)}
  >
    <Icon name="folder" size="0.95rem" />
    <span class="label">{current?.name ?? "Carpeta automática"}</span>
    {#if !locked}<span class="caret" aria-hidden="true"><Icon name="down" size="0.8rem" /></span>{/if}
  </button>

  {#if open}
    <div class="menu popover" class:down={!up} role="menu" aria-label="Carpeta del chat" in:popIn={{ y: up ? 6 : -6 }} out:popOut={{ y: up ? 4 : -4 }}>
      <div class="popover-label">Dónde trabaja la IA</div>
      <button role="menuitemradio" aria-checked={projectId === null} class="item" onclick={() => choose(null)}>
        <span class="check" aria-hidden="true">{#if projectId === null}<Icon name="check" size="0.85rem" />{/if}</span>
        <span class="t"><span class="n">Carpeta automática</span><span class="f">Se crea en CoModel/Chats al enviar el primer mensaje</span></span>
      </button>
      {#each projects.list as p (p.id)}
        <button role="menuitemradio" aria-checked={p.id === projectId} class="item" onclick={() => choose(p.id)}>
          <span class="check" aria-hidden="true">{#if p.id === projectId}<Icon name="check" size="0.85rem" />{/if}</span>
          <span class="t"><span class="n">{p.name}</span><span class="f" title={p.path}>{folder(p.path)}</span></span>
        </button>
      {/each}
      <div class="popover-sep"></div>
      <button role="menuitem" class="item" onclick={pick}>
        <span class="check" aria-hidden="true"><Icon name="plus" size="0.85rem" /></span>
        <span class="t"><span class="n">Elegir otra carpeta…</span></span>
      </button>
      {#if error}<p class="error" role="alert">{error}</p>{/if}
    </div>
  {/if}
</div>

<style>
  .wrap {
    position: relative;
    display: inline-block;
    min-width: 0;
  }
  .trigger {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    max-width: 16rem;
    height: var(--control-sm);
    padding: 0 0.45rem;
    border: 0;
    border-radius: var(--radius);
    background: none;
    font-size: var(--fs-xs);
    font-weight: 500;
    color: var(--text-3);
    transition:
      background-color var(--dur-fast) var(--ease),
      color var(--dur-fast) var(--ease);
  }
  .trigger.set {
    color: var(--text-2);
  }
  .trigger:hover:not(:disabled),
  .trigger[aria-expanded="true"] {
    background: var(--raised-2);
    color: var(--text);
  }
  .label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .caret {
    display: grid;
    opacity: 0.6;
  }
  .menu {
    position: absolute;
    bottom: calc(100% + 0.35rem);
    left: 0;
    display: grid;
    min-width: 18rem;
    max-height: 20rem;
    overflow-y: auto;
  }
  .menu.down {
    bottom: auto;
    top: calc(100% + 0.35rem);
  }
  .item {
    display: grid;
    grid-template-columns: 1rem minmax(0, 1fr);
    align-items: start;
    gap: 0.45rem;
    padding: 0.38rem 0.55rem 0.38rem 0.45rem;
    border: 0;
    border-radius: var(--radius);
    background: none;
    text-align: left;
    transition: background-color var(--dur-fast) var(--ease);
  }
  .item:hover,
  .item:focus-visible {
    background: var(--raised-2);
    outline: none;
  }
  .check {
    display: grid;
    place-items: center;
    height: 1.25rem;
    color: var(--accent);
  }
  .t {
    display: grid;
    min-width: 0;
  }
  .n {
    font-size: var(--fs-sm);
    line-height: 1.25rem;
    color: var(--text);
  }
  .f {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .error {
    margin: 0.3rem 0.55rem 0.2rem;
    font-size: var(--fs-xs);
    color: var(--danger);
  }
</style>
