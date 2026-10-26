<script lang="ts">
  import { canvas, here } from "./canvas.svelte";

  let box: HTMLDivElement;

  // Zona visible del lienzo, en coordenadas del mundo.
  const view = $derived({
    x: -canvas.view.x / canvas.view.k,
    y: -canvas.view.y / canvas.view.k,
    w: canvas.size.w / canvas.view.k,
    h: canvas.size.h / canvas.view.k,
  });

  // El mapa es el espacio de trabajo, no la pantalla: una zona base de 3 x 3 pantallas, que crece
  // si hay tarjetas o la cámara se va más lejos. Así la cámara (en color) muestra dónde estás.
  const bounds = $derived.by(() => {
    const pad = Math.max(canvas.size.w, canvas.size.h) * 0.05;
    let x0 = Math.min(canvas.size.w / 2 - canvas.size.w * 1.5, view.x);
    let y0 = Math.min(canvas.size.h / 2 - canvas.size.h * 1.5, view.y);
    let x1 = Math.max(canvas.size.w / 2 + canvas.size.w * 1.5, view.x + view.w);
    let y1 = Math.max(canvas.size.h / 2 + canvas.size.h * 1.5, view.y + view.h);
    for (const c of here()) {
      x0 = Math.min(x0, c.x);
      y0 = Math.min(y0, c.y);
      x1 = Math.max(x1, c.x + c.w);
      y1 = Math.max(y1, c.y + c.h);
    }
    return { x: x0 - pad, y: y0 - pad, w: x1 - x0 + pad * 2, h: y1 - y0 + pad * 2 };
  });

  function goTo(e: PointerEvent) {
    const r = box.getBoundingClientRect();
    const s = Math.min(r.width / bounds.w, r.height / bounds.h);
    const ox = (r.width - bounds.w * s) / 2;
    const oy = (r.height - bounds.h * s) / 2;
    const wx = bounds.x + (e.clientX - r.left - ox) / s;
    const wy = bounds.y + (e.clientY - r.top - oy) / s;
    canvas.view.x = canvas.size.w / 2 - wx * canvas.view.k;
    canvas.view.y = canvas.size.h / 2 - wy * canvas.view.k;
  }

  function onpointerdown(e: PointerEvent) {
    if (e.button !== 0) return;
    box.setPointerCapture(e.pointerId);
    goTo(e);
    const move = (ev: PointerEvent) => goTo(ev);
    const up = () => {
      box.removeEventListener("pointermove", move);
      box.removeEventListener("pointerup", up);
      box.removeEventListener("pointercancel", up);
    };
    box.addEventListener("pointermove", move);
    box.addEventListener("pointerup", up);
    box.addEventListener("pointercancel", up);
  }

  function onkeydown(e: KeyboardEvent) {
    const step = (e.shiftKey ? 240 : 80) * canvas.view.k;
    const d: Record<string, [number, number]> = { ArrowLeft: [step, 0], ArrowRight: [-step, 0], ArrowUp: [0, step], ArrowDown: [0, -step] };
    const v = d[e.key];
    if (v) {
      e.preventDefault();
      canvas.view.x += v[0];
      canvas.view.y += v[1];
    }
  }
</script>

<!-- Control de desplazamiento en dos ejes: no existe un rol ARIA exacto, así que se declara como aplicación con etiqueta y teclado. -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div
  bind:this={box}
  class="minimap"
  role="application"
  tabindex="0"
  aria-label="Minimapa del lienzo. Flechas para desplazarte."
  style:aspect-ratio="{canvas.size.w} / {canvas.size.h}"
  {onpointerdown}
  {onkeydown}
>
  <svg viewBox="{bounds.x} {bounds.y} {bounds.w} {bounds.h}" preserveAspectRatio="xMidYMid meet" aria-hidden="true">
    {#each here() as c (c.id)}
      <rect class="card" class:focus={canvas.focusId === c.id} class:busy={c.busy} x={c.x} y={c.y} width={c.w} height={c.h} rx="16" />
    {/each}
    <rect class="view" x={view.x} y={view.y} width={view.w} height={view.h} />
  </svg>
</div>

<style>
  .minimap {
    position: absolute;
    right: 0.75rem;
    bottom: 0.75rem;
    width: 11rem;
    max-height: 11rem;
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-card);
    overflow: hidden;
    cursor: crosshair;
    touch-action: none;
  }
  svg {
    display: block;
    width: 100%;
    height: 100%;
  }
  /* Las tarjetas son neutras; solo la cámara lleva color. */
  .card {
    fill: var(--raised-2);
    stroke: var(--line);
    vector-effect: non-scaling-stroke;
    stroke-width: 1;
  }
  .card.focus {
    stroke: var(--text-2);
  }
  .card.busy {
    stroke: var(--accent);
  }
  .view {
    fill: color-mix(in oklch, var(--accent) 28%, transparent);
    stroke: var(--accent);
    vector-effect: non-scaling-stroke;
    stroke-width: 1.5;
  }
</style>
