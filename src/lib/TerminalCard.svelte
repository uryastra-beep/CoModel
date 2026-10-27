<script lang="ts">
  // Terminal real: un shell del sistema en la carpeta del workspace, dibujado con xterm.js.
  import { onMount } from "svelte";
  import { FitAddon } from "@xterm/addon-fit";
  import { Terminal } from "@xterm/xterm";
  import "@xterm/xterm/css/xterm.css";
  import { onTermEvent, termOpen, termResize, termWrite, terminalAvailable } from "./backend";
  import type { Card } from "./canvas.svelte";
  import { trackPointer } from "./cardDrag";
  import Icon from "./Icon.svelte";
  import { cardIn, cardOut } from "./motion";
  import { projectById } from "./projects.svelte";

  let { card, scale, focused, onfocus, onclose, onmove, onresize }: {
    card: Card; scale: number; focused: boolean; onfocus: () => void; onclose: () => void;
    onmove: (dx: number, dy: number) => void; onresize: (dw: number, dh: number) => void;
  } = $props();

  let host = $state<HTMLDivElement>();
  let dragging = $state(false);
  let ended = $state(false);
  let error = $state("");
  let term: Terminal | undefined;
  let restart = $state<() => void>(() => {});
  const folder = $derived(projectById(card.projectId));

  /** Lee un color CSS (incluido oklch) tal como lo pinta el navegador, para dárselo a xterm. */
  const swatch = document.createElement("canvas").getContext("2d", { willReadFrequently: true })!;
  function rgb(css: string): string {
    swatch.clearRect(0, 0, 1, 1);
    swatch.fillStyle = "#000";
    swatch.fillStyle = css;
    swatch.fillRect(0, 0, 1, 1);
    const [r, g, b] = swatch.getImageData(0, 0, 1, 1).data;
    return `rgb(${r}, ${g}, ${b})`;
  }

  function theme() {
    const cs = getComputedStyle(document.documentElement);
    const v = (n: string) => rgb(cs.getPropertyValue(n).trim());
    const light = cs.colorScheme.includes("light") && !cs.colorScheme.includes("dark");
    // Colores ANSI legibles sobre el fondo del tema.
    const ansi = light
      ? ["#3b3b3b", "#b3261e", "#1e7a3c", "#8a6100", "#2457b5", "#8b3fa0", "#0f7482", "#6b6b6b", "#555555", "#d03a31", "#2a9150", "#a67800", "#3b6fd1", "#a352bb", "#1a8c9c", "#9a9a9a"]
      : ["#5c6370", "#e5786d", "#7fc58d", "#e0b862", "#7aa7ee", "#c28be0", "#6cc6d1", "#c9ccd2", "#7d8493", "#f09287", "#98d8a4", "#efcb7f", "#97bcf5", "#d6a4ee", "#8ad8e2", "#eef0f3"];
    const names = ["black", "red", "green", "yellow", "blue", "magenta", "cyan", "white", "brightBlack", "brightRed", "brightGreen", "brightYellow", "brightBlue", "brightMagenta", "brightCyan", "brightWhite"] as const;
    return {
      background: v("--bg"),
      foreground: v("--text"),
      cursor: v("--accent"),
      cursorAccent: v("--bg"),
      selectionBackground: v("--accent-tint"),
      ...Object.fromEntries(names.map((n, i) => [n, ansi[i]])),
    };
  }

  const fontSize = () => (parseFloat(getComputedStyle(document.documentElement).fontSize) || 16) * 0.8125;

  onMount(() => {
    if (!terminalAvailable) return;
    let alive = true;
    const t = new Terminal({
      fontFamily: '"Geist Mono Variable", ui-monospace, monospace',
      fontSize: fontSize(),
      lineHeight: 1.25,
      cursorBlink: true,
      scrollback: 5000,
      allowProposedApi: true,
      theme: theme(),
    });
    const f = new FitAddon();
    t.loadAddon(f);
    t.open(host!);
    term = t;

    const decode = (b64: string) => Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
    let offEvents: (() => void) | undefined;
    onTermEvent((e) => {
      if (e.id !== card.id) return;
      if (e.data) t.write(decode(e.data));
      if (e.exit) {
        ended = true;
        t.write("\r\n\x1b[2m[El proceso terminó]\x1b[0m\r\n");
      }
    }).then((off) => (alive ? (offEvents = off) : off()));

    const start = async () => {
      ended = false;
      error = "";
      try {
        f.fit();
        await termOpen(card.id, card.projectId, t.cols, t.rows);
      } catch (e) {
        error = String(e);
      }
    };
    restart = async () => {
      // El shell anterior ya terminó: se cierra su registro y se abre otro.
      const { termClose } = await import("./backend");
      await termClose(card.id);
      t.reset();
      await start();
    };
    void start();

    const input = t.onData((d) => void termWrite(card.id, d).catch(() => {}));
    const resized = t.onResize(({ cols, rows }) => void termResize(card.id, cols, rows).catch(() => {}));
    const ro = new ResizeObserver(() => {
      if (host && host.clientWidth > 20 && host.clientHeight > 20) f.fit();
    });
    ro.observe(host!);
    // Al cambiar de tema o de tamaño de texto, la terminal sigue el cambio.
    const mo = new MutationObserver(() => {
      t.options.theme = theme();
      t.options.fontSize = fontSize();
      f.fit();
    });
    mo.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme", "style", "data-motion"] });

    return () => {
      alive = false;
      offEvents?.();
      input.dispose();
      resized.dispose();
      ro.disconnect();
      mo.disconnect();
      t.dispose();
    };
  });

  function drag(e: PointerEvent) {
    if ((e.target as HTMLElement).closest("button, input")) return;
    trackPointer(e, scale, onfocus, onmove, (on) => (dragging = on));
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<article class="card" class:focused class:dragging data-card-id={card.id} style:left="{card.x}px" style:top="{card.y}px" style:width="{card.w}px" style:height="{card.h}px" style:z-index={card.z} onpointerdown={() => { onfocus(); term?.focus(); }} in:cardIn out:cardOut aria-label={card.title}>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <header onpointerdown={drag}>
    <Icon name="terminal" size="1rem" /><strong>{card.title}</strong>
    <span class="where" title={folder?.path ?? "Carpeta personal"}>{folder?.name ?? "Carpeta personal"}</span>
    <span class="spacer"></span>
    {#if ended}<button class="btn sm" onclick={restart}>Reiniciar</button>{/if}
    <button class="icon-btn sm" onclick={onclose} aria-label="Cerrar {card.title}" title="Cerrar la terminal (termina lo que corra en ella)"><Icon name="x" size="0.95rem" /></button>
  </header>
  {#if !terminalAvailable}
    <p class="notice">La terminal solo funciona en la app de escritorio.</p>
  {:else}
    {#if error}<p class="notice bad" role="alert">{error}</p>{/if}
    <div class="screen" bind:this={host}></div>
  {/if}
  <div class="grip" aria-hidden="true" onpointerdown={(e) => { e.stopPropagation(); trackPointer(e, scale, onfocus, onresize, (on) => (dragging = on)); }}></div>
</article>

<style>
  .card { position: absolute; display: flex; flex-direction: column; background: var(--bg); border: 1px solid var(--line); border-radius: var(--radius-xl); box-shadow: var(--shadow-card); transition: border-color var(--dur-base) var(--ease), box-shadow var(--dur-base) var(--ease); }
  .focused { border-color: var(--accent); }
  .dragging { box-shadow: var(--lift); }
  header { display: flex; gap: 0.5rem; align-items: center; padding: 0 0.4rem 0 0.85rem; height: 2.6rem; flex: none; background: var(--surface); border-bottom: 1px solid var(--line-soft); border-radius: var(--radius-xl) var(--radius-xl) 0 0; cursor: grab; touch-action: none; }
  strong { font-size: var(--fs-sm); white-space: nowrap; }
  .where { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: var(--fs-xs); color: var(--text-3); }
  .spacer { flex: 1; }
  .screen { flex: 1; min-height: 0; padding: 0.4rem 0 0.4rem 0.6rem; overflow: hidden; border-radius: 0 0 var(--radius-xl) var(--radius-xl); }
  .screen :global(.xterm) { height: 100%; }
  .screen :global(.xterm-viewport) { background: transparent !important; }
  .notice { margin: 0.6rem 0.85rem; font-size: var(--fs-xs); color: var(--text-3); overflow-wrap: anywhere; }
  .notice.bad { color: var(--danger); }
  .grip { position: absolute; right: -1px; bottom: -1px; width: 1rem; height: 1rem; cursor: nwse-resize; touch-action: none; border-bottom: 2px solid var(--line); border-right: 2px solid var(--line); border-radius: 0 0 var(--radius-xl) 0; }
</style>
