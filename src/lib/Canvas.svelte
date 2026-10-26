<script lang="ts">
  import { onMount } from "svelte";
  import ChatCard from "./ChatCard.svelte";
  import FileCard from "./FileCard.svelte";
  import PanelCard from "./PanelCard.svelte";
  import Icon from "./Icon.svelte";
  import { ms } from "./motion";
  import { onFileDrop } from "./backend";
  import { fade } from "svelte/transition";
  import { addCard, canvas, fitAll, focusCard, moveCard, panBy, removeCard, renameCard, resetZoom, resizeCard, setBusy, setCardProject, zoomAt, zoomStep } from "./canvas.svelte";
  import Minimap from "./Minimap.svelte";
  import { startChat } from "./start";
  import { ui } from "./ui.svelte";

  let root: HTMLDivElement;
  let panning = $state(false);

  function worldPoint(e: MouseEvent) {
    const r = root.getBoundingClientRect();
    return { x: (e.clientX - r.left - canvas.view.x) / canvas.view.k, y: (e.clientY - r.top - canvas.view.y) / canvas.view.k };
  }

  function onPointerDown(e: PointerEvent) {
    if (e.button !== 0 || e.target !== root && !(e.target as HTMLElement).dataset.world) return;
    canvas.focusId = null;
    panning = true;
    root.setPointerCapture(e.pointerId);
    let lx = e.clientX;
    let ly = e.clientY;
    const move = (ev: PointerEvent) => {
      panBy(ev.clientX - lx, ev.clientY - ly);
      lx = ev.clientX;
      ly = ev.clientY;
    };
    const up = () => {
      panning = false;
      root.removeEventListener("pointermove", move);
      root.removeEventListener("pointerup", up);
      root.removeEventListener("pointercancel", up);
    };
    root.addEventListener("pointermove", move);
    root.addEventListener("pointerup", up);
    root.addEventListener("pointercancel", up);
  }

  /** Si algún contenedor entre el puntero y el lienzo puede desplazarse en esa dirección. */
  function scrollsItself(el: HTMLElement | null, dy: number, dx: number): boolean {
    for (let n = el; n && n !== root; n = n.parentElement) {
      const st = getComputedStyle(n);
      if (dy && /(auto|scroll)/.test(st.overflowY) && n.scrollHeight > n.clientHeight + 1) return true;
      if (dx && /(auto|scroll)/.test(st.overflowX) && n.scrollWidth > n.clientWidth + 1) return true;
    }
    return false;
  }

  function onDblClick(e: MouseEvent) {
    if (e.target !== root && !(e.target as HTMLElement).dataset.world) return;
    addCard(worldPoint(e));
  }

  onMount(() => {
    const ro = new ResizeObserver(([entry]) => {
      canvas.size.w = entry.contentRect.width;
      canvas.size.h = entry.contentRect.height;
    });
    ro.observe(root);

    // La rueda desplaza el lienzo; con Ctrl (o el gesto de pellizco) hace zoom.
    const onWheel = (e: WheelEvent) => {
      // Dentro de algo con su propio scroll (una conversación, un editor, una lista), la rueda es suya.
      if (!e.ctrlKey && !e.metaKey && scrollsItself(e.target as HTMLElement, e.deltaY, e.deltaX)) return;
      e.preventDefault();
      if (e.ctrlKey || e.metaKey) {
        const r = root.getBoundingClientRect();
        zoomAt(Math.exp(-e.deltaY * 0.01), e.clientX - r.left, e.clientY - r.top);
      } else {
        panBy(-e.deltaX, -e.deltaY);
      }
    };
    root.addEventListener("wheel", onWheel, { passive: false });

    // Archivos arrastrados desde el sistema: van a la tarjeta que hay bajo el puntero (o a la que tiene el foco).
    const cardAt = (x: number, y: number) => {
      const el = document.elementFromPoint(x, y)?.closest<HTMLElement>("[data-card-id]");
      return el?.dataset.cardId ?? canvas.focusId;
    };
    const markTarget = (id: string | null) => {
      document.querySelectorAll(".card.drop").forEach((n) => n.classList.remove("drop"));
      if (id) document.querySelector(`[data-card-id="${id}"]`)?.classList.add("drop");
    };
    let offDrop: (() => void) | undefined;
    onFileDrop((e) => {
      if (e.phase === "leave") return markTarget(null);
      const id = cardAt(e.x, e.y);
      if (e.phase === "over") return markTarget(id);
      markTarget(null);
      if (!id || e.paths.length === 0) return;
      const files = e.paths.map((path) => ({ path, name: path.split(/[\\/]/).pop() || path }));
      window.dispatchEvent(new CustomEvent("comodel:attach", { detail: { cardId: id, files } }));
    }).then((f) => (offDrop = f));

    return () => {
      offDrop?.();
      ro.disconnect();
      root.removeEventListener("wheel", onWheel);
    };
  });
</script>

<div
  bind:this={root}
  class="canvas"
  class:panning
  role="application"
  aria-label="Lienzo de chats"
  style:background-size="{24 * canvas.view.k}px {24 * canvas.view.k}px"
  style:background-position="{canvas.view.x}px {canvas.view.y}px"
  onpointerdown={onPointerDown}
  ondblclick={onDblClick}
>
  <div class="world" data-world="1" style:transform="translate({canvas.view.x}px, {canvas.view.y}px) scale({canvas.view.k})">
    <svg class="connections" aria-label="Conexiones de memoria compartida">
      {#each canvas.links as link}
        {@const a = canvas.cards.find((c) => c.id === link.a)}
        {@const b = canvas.cards.find((c) => c.id === link.b)}
        {#if a && b}<path d="M {a.x + a.w / 2} {a.y + 22} C {a.x + a.w / 2} {a.y - 65}, {b.x + b.w / 2} {b.y - 65}, {b.x + b.w / 2} {b.y + 22}" />{/if}
      {/each}
    </svg>
    {#each canvas.cards as card (card.id)}
      {#if card.kind === "file"}
        <FileCard
          {card}
          scale={canvas.view.k}
          focused={canvas.focusId === card.id}
          onfocus={() => focusCard(card.id)}
          onclose={() => removeCard(card.id)}
          onmove={(dx, dy) => moveCard(card.id, dx, dy)}
          onresize={(dw, dh) => resizeCard(card.id, dw, dh)}
        />
      {:else if card.kind === "files" || card.kind === "code"}
        <PanelCard {card} scale={canvas.view.k} focused={canvas.focusId === card.id}
          onfocus={() => focusCard(card.id)} onclose={() => removeCard(card.id)}
          onmove={(dx, dy) => moveCard(card.id, dx, dy)} onresize={(dw, dh) => resizeCard(card.id, dw, dh)} />
      {:else}
        <ChatCard
          {card}
          scale={canvas.view.k}
          focused={canvas.focusId === card.id}
          onfocus={() => focusCard(card.id)}
          onclose={() => removeCard(card.id)}
          onmove={(dx, dy) => moveCard(card.id, dx, dy)}
          onresize={(dw, dh) => resizeCard(card.id, dw, dh)}
          onrename={(t) => renameCard(card.id, t)}
          onbusy={(b) => setBusy(card.id, b)}
          onproject={(p) => setCardProject(card.id, p)}
        />
      {/if}
    {/each}
  </div>

  {#if canvas.cards.length === 0}
    <div class="empty" in:fade={{ duration: ms(220), delay: ms(80) }} out:fade={{ duration: ms(140) }}>
      <div class="intro">
        <h2>El lienzo está vacío</h2>
        <p class="lead">Cada chat es una tarjeta que puedes mover y agrandar. Pon varios uno al lado del otro para trabajar con distintas IAs a la vez.</p>
        <ul>
          <li><Icon name="chat" size="1rem" /><span><b>Abre un chat</b> con doble clic en el lienzo o con <kbd>N</kbd>.</span></li>
          <li><Icon name="memory" size="1rem" /><span><b>Comparten memoria</b>: en un proyecto, todas leen y escriben las mismas notas.</span></li>
          <li><Icon name="relay" size="1rem" /><span><b>Relevo automático</b>: si una se queda sin uso, sigue la siguiente.</span></li>
        </ul>
        <div class="acts">
          <button class="btn primary" onclick={() => startChat("", null)}><Icon name="plus" size="0.95rem" />Nuevo chat</button>
          <button class="btn" onclick={() => (ui.importOpen = true)}><Icon name="import" size="0.95rem" />Importar un chat</button>
        </div>
      </div>
    </div>
  {/if}

  <div class="controls" role="group" aria-label="Vista del lienzo">
    <button class="icon-btn sm" onclick={() => zoomStep(1 / 1.2)} aria-label="Alejar" title="Alejar (Ctrl+rueda)"><Icon name="minus" size="0.95rem" /></button>
    <button class="pct" onclick={resetZoom} title="Volver a 100%">{Math.round(canvas.view.k * 100)}%</button>
    <button class="icon-btn sm" onclick={() => zoomStep(1.2)} aria-label="Acercar" title="Acercar (Ctrl+rueda)"><Icon name="plus" size="0.95rem" /></button>
    <span class="vsep" aria-hidden="true"></span>
    <button class="icon-btn sm" onclick={fitAll} disabled={canvas.cards.length === 0} aria-label="Encuadrar todo" title="Encuadrar todos los chats"><Icon name="fit" size="0.95rem" /></button>
  </div>
  {#if canvas.cards.length > 0}<Minimap />{/if}
</div>

<style>
  .canvas {
    position: relative;
    height: 100%;
    overflow: hidden;
    background-color: var(--bg);
    background-image: radial-gradient(var(--dot) 1px, transparent 1.5px);
    touch-action: none;
  }
  .canvas.panning {
    cursor: grabbing;
  }
  .connections { position: absolute; overflow: visible; width: 1px; height: 1px; pointer-events: none; }
  .connections path { fill: none; stroke: var(--accent); stroke-width: 2; opacity: 0.65; }
  .world {
    position: absolute;
    left: 0;
    top: 0;
    width: 0;
    height: 0;
    transform-origin: 0 0;
  }
  .empty {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    pointer-events: none;
  }
  .intro {
    width: min(31rem, 86%);
    padding: 1.75rem 1.9rem 1.6rem;
    border: 1px solid var(--line-soft);
    border-radius: var(--radius-xl);
    background: color-mix(in oklch, var(--surface) 92%, transparent);
    pointer-events: auto;
  }
  h2 {
    margin: 0;
    font-size: var(--fs-lg);
    font-weight: 600;
    letter-spacing: -0.01em;
  }
  .lead {
    margin: 0.45rem 0 1.2rem;
    color: var(--text-2);
    font-size: var(--fs-sm);
    line-height: 1.55;
  }
  ul {
    display: grid;
    gap: 0.7rem;
    margin: 0 0 1.5rem;
    padding: 0;
    list-style: none;
  }
  li {
    display: grid;
    grid-template-columns: 1.1rem minmax(0, 1fr);
    gap: 0.7rem;
    align-items: start;
    font-size: var(--fs-sm);
    color: var(--text-3);
  }
  li :global(.icon) {
    margin-top: 0.12rem;
    color: var(--text-2);
  }
  li b {
    font-weight: 600;
    color: var(--text);
  }
  .acts {
    display: flex;
    gap: 0.5rem;
  }

  .controls {
    position: absolute;
    left: 0.75rem;
    bottom: 0.75rem;
    display: flex;
    align-items: center;
    gap: 0.1rem;
    padding: 0.2rem;
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-card);
  }
  .pct {
    min-width: 3.1rem;
    height: var(--control-sm);
    border: 0;
    border-radius: var(--radius);
    background: none;
    font-size: var(--fs-xs);
    color: var(--text-2);
    font-variant-numeric: tabular-nums;
  }
  .pct:hover {
    background: var(--raised-2);
    color: var(--text);
  }
  .vsep {
    width: 1px;
    height: 1rem;
    margin: 0 0.2rem;
    background: var(--line);
  }
</style>
