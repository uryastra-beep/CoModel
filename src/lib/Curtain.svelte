<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    open: boolean;
    id?: string;
    children: Snippet;
  }
  let { open, id, children }: Props = $props();
</script>

<!--
  Cortina: el contenido se despliega y se recoge hacia arriba. Se anima la fila de la rejilla (0fr a 1fr),
  que es la forma de animar una altura automática sin medirla. Cerrada, el contenido no recibe foco ni lo leen
  los lectores de pantalla.
-->
<div class="curtain" class:open {id} inert={!open} aria-hidden={!open}>
  <div class="inner">
    <div class="drop">{@render children()}</div>
  </div>
</div>

<style>
  .curtain {
    display: grid;
    grid-template-rows: 0fr;
    opacity: 0;
    /* Al cerrar, más rápida que al abrir. */
    transition:
      grid-template-rows var(--dur-out) var(--ease-in),
      opacity var(--dur-fast) var(--ease-in);
  }
  .curtain.open {
    grid-template-rows: 1fr;
    opacity: 1;
    transition:
      grid-template-rows var(--dur-slow) var(--ease),
      opacity var(--dur-base) var(--ease);
  }
  .inner {
    min-height: 0;
    overflow: hidden;
  }
  /* El contenido baja un poco al abrir, como la tela de una cortina. */
  .drop {
    transform: translateY(-0.5rem);
    transition: transform var(--dur-out) var(--ease-in);
  }
  .curtain.open .drop {
    transform: translateY(0);
    transition: transform var(--dur-slow) var(--ease);
  }
</style>
