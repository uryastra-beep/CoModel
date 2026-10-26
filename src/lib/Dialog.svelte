<script lang="ts">
  import { onMount, type Snippet } from "svelte";
  import Icon from "./Icon.svelte";
  import { ms } from "./motion";

  interface Props {
    title: string;
    /** Se llama cuando el diálogo ya se cerró (por la ×, Escape, el fondo o `close()`). */
    onclose: () => void;
    width?: string;
    height?: string;
    /** Recibe una función para cerrar con animación desde dentro. */
    children: Snippet<[() => void]>;
    /** Texto bajo el título. */
    subtitle?: string;
    /** Sin cabecera propia (el contenido trae la suya). */
    bare?: boolean;
  }
  let { title, onclose, width = "min(34rem, 94vw)", height = "fit-content", children, subtitle, bare = false }: Props = $props();

  const uid = $props.id();
  let dialog: HTMLDialogElement;
  let closing = $state(false);

  // <dialog> modal: el navegador se encarga del foco, de Escape y de bloquear lo de detrás.
  onMount(() => {
    dialog.showModal();
    // El foco entra al diálogo, no al primer botón (que se vería con anillo al abrir).
    dialog.focus();
    return () => {
      if (dialog.open) dialog.close();
    };
  });

  // Cerrar = reproducir la salida y, al terminar, cerrar el <dialog> de verdad.
  function requestClose() {
    if (closing) return;
    closing = true;
    // Seguro: si el navegador no avisa del fin de la animación, se cierra igualmente.
    setTimeout(() => {
      if (dialog?.open) dialog.close();
    }, ms(150) + 80);
  }

  function onAnimationEnd(e: AnimationEvent) {
    if (closing && e.target === dialog) dialog.close();
  }

  // Un clic en el fondo (fuera del contenido) cierra el diálogo.
  function onBackdrop(e: MouseEvent) {
    if (e.target === dialog) requestClose();
  }
</script>

<dialog
  bind:this={dialog}
  class:closing
  style:--w={width}
  style:--h={height}
  aria-labelledby="{uid}-title"
  tabindex="-1"
  {onclose}
  onclick={onBackdrop}
  oncancel={(e) => {
    e.preventDefault();
    requestClose();
  }}
  onanimationend={onAnimationEnd}
>
  {#if bare}
    <h1 id="{uid}-title" class="sr-only">{title}</h1>
  {:else}
    <header>
      <div class="titles">
        <h1 id="{uid}-title">{title}</h1>
        {#if subtitle}<p class="subtitle">{subtitle}</p>{/if}
      </div>
      <button class="icon-btn sm" onclick={requestClose} aria-label="Cerrar {title.toLowerCase()}" title="Cerrar (Esc)"><Icon name="x" size="1rem" /></button>
    </header>
  {/if}
  {@render children(requestClose)}
</dialog>

<style>
  dialog {
    width: var(--w);
    height: var(--h);
    max-width: none;
    max-height: 90vh;
    padding: 0;
    color: var(--text);
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: var(--radius-xl);
    box-shadow: var(--lift);
  }
  dialog:focus-visible {
    outline: none;
  }
  dialog[open] {
    display: flex;
    flex-direction: column;
    animation: pop-in var(--dur-base) var(--ease) both;
  }
  dialog::backdrop {
    background: var(--scrim);
  }
  dialog[open]::backdrop {
    animation: fade-in var(--dur-base) var(--ease) both;
  }
  /* Salida: más corta que la entrada y con aceleración. */
  dialog.closing[open] {
    animation: pop-out var(--dur-out) var(--ease-in) both;
  }
  dialog.closing[open]::backdrop {
    animation: fade-out var(--dur-out) var(--ease-in) both;
  }
  @keyframes pop-in {
    from {
      opacity: 0;
      transform: translateY(0.6rem) scale(0.98);
    }
  }
  @keyframes pop-out {
    to {
      opacity: 0;
      transform: translateY(0.4rem) scale(0.98);
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

  header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 1rem;
    flex: none;
    padding: 1rem 0.8rem 0.9rem 1.35rem;
    border-bottom: 1px solid var(--line-soft);
  }
  .titles {
    display: grid;
    gap: 0.15rem;
    padding-top: 0.15rem;
  }
  h1 {
    margin: 0;
    font-size: var(--fs-lg);
    font-weight: 600;
    letter-spacing: -0.01em;
    line-height: 1.3;
  }
  .subtitle {
    margin: 0;
    font-size: var(--fs-sm);
    color: var(--text-3);
  }
</style>
