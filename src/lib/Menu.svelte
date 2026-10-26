<script lang="ts">
  import { tick } from "svelte";
  import Curtain from "./Curtain.svelte";
  import Icon, { type IconName } from "./Icon.svelte";
  import { popIn, popOut } from "./motion";
  import type { Choice } from "./providers";

  interface Props {
    /** Nombre del control, p. ej. "Modelo". Se lee en voz alta y encabeza la lista. */
    label: string;
    value: string;
    options: Choice[];
    /** Opciones menos habituales, plegadas tras "Ver más". */
    more?: Choice[];
    moreLabel?: string;
    /** La lista de "Ver más" se filtra escribiendo (listas largas). */
    searchable?: boolean;
    onchange: (id: string) => void;
    /** Abre hacia arriba (por defecto, para controles pegados al borde inferior). */
    up?: boolean;
    /** Alinea la lista al borde derecho del control. */
    alignEnd?: boolean;
    /** Texto del botón cuando el valor no está en la lista. */
    fallback?: string;
    /** Explicación al pasar el puntero. */
    title?: string;
    /** Icono delante del valor, para que se sepa qué se elige sin leer la etiqueta. */
    icon?: IconName;
  }
  let { label, value, options, more = [], moreLabel = "Ver más modelos", searchable = false, onchange, up = true, alignEnd = false, fallback, title, icon }: Props = $props();

  const uid = $props.id();
  let open = $state(false);
  let active = $state(0);
  let expanded = $state(false);
  let query = $state("");
  let root: HTMLDivElement;
  let button: HTMLButtonElement;
  let search = $state<HTMLInputElement>();

  const all = $derived([...options, ...more]);
  const current = $derived(all.find((o) => o.id === value));
  const MAX_SHOWN = 60;
  const filtered = $derived.by(() => {
    const q = query.trim().toLowerCase();
    const list = q ? more.filter((o) => `${o.label} ${o.id}`.toLowerCase().includes(q)) : more;
    return list.slice(0, MAX_SHOWN);
  });
  const hiddenCount = $derived(Math.max(0, (query.trim() ? more.filter((o) => `${o.label} ${o.id}`.toLowerCase().includes(query.trim().toLowerCase())).length : more.length) - MAX_SHOWN));

  // Filas navegables: las principales, el botón de "ver más" y, si está abierto, las demás.
  type Row = { kind: "option"; choice: Choice; id: string } | { kind: "more"; id: string };
  const rows = $derived.by<Row[]>(() => {
    const r: Row[] = options.map((choice) => ({ kind: "option", choice, id: `${uid}-${choice.id}` }));
    if (more.length) {
      r.push({ kind: "more", id: `${uid}-more` });
      if (expanded) for (const choice of filtered) r.push({ kind: "option", choice, id: `${uid}-${choice.id}` });
    }
    return r;
  });

  function toggle() {
    open = !open;
    if (open) {
      query = "";
      // Si el valor actual es uno de los de "ver más", la lista ya se abre desplegada.
      expanded = more.some((o) => o.id === value);
      const i = rows.findIndex((r) => r.kind === "option" && r.choice.id === value);
      active = Math.max(0, i);
    }
  }

  async function pick(i: number) {
    const r = rows[i];
    if (!r) return;
    if (r.kind === "more") {
      expanded = !expanded;
      if (expanded && searchable) {
        await tick();
        search?.focus();
      }
      return;
    }
    onchange(r.choice.id);
    open = false;
    button.focus();
  }

  function onkeydown(e: KeyboardEvent) {
    if (!open) {
      if (e.key === "ArrowDown" || e.key === "ArrowUp" || e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        toggle();
      }
      return;
    }
    navigate(e);
  }

  function navigate(e: KeyboardEvent) {
    const n = rows.length;
    const inSearch = e.target === search;
    if (e.key === "ArrowDown") active = (active + 1) % n;
    else if (e.key === "ArrowUp") active = (active - 1 + n) % n;
    else if (e.key === "Home" && !inSearch) active = 0;
    else if (e.key === "End" && !inSearch) active = n - 1;
    else if (e.key === "Enter" || (e.key === " " && !inSearch)) pick(active);
    else if (e.key === "Escape") {
      e.stopPropagation(); // que no cierre también el diálogo o la tarjeta de debajo
      open = false;
      button.focus();
    } else if (e.key === "Tab") open = false;
    else return;
    if (e.key !== "Tab") e.preventDefault();
  }

  // Al escribir en la búsqueda, la primera coincidencia queda activa.
  $effect(() => {
    void query;
    if (!open || !expanded) return;
    const first = rows.findIndex((r, i) => i > options.length && r.kind === "option");
    if (first > 0) active = first;
  });

  // La fila activa siempre queda a la vista dentro de la lista con scroll.
  $effect(() => {
    if (!open) return;
    document.getElementById(rows[active]?.id)?.scrollIntoView({ block: "nearest" });
  });

  $effect(() => {
    if (!open) return;
    const close = (e: PointerEvent) => {
      if (!root.contains(e.target as Node)) open = false;
    };
    window.addEventListener("pointerdown", close, true);
    return () => window.removeEventListener("pointerdown", close, true);
  });
</script>

<div class="menu" bind:this={root}>
  <button
    bind:this={button}
    type="button"
    class="trigger"
    role="combobox"
    aria-haspopup="listbox"
    aria-expanded={open}
    aria-label="{label}: {current?.label ?? fallback ?? ''}"
    aria-controls="{uid}-list"
    aria-activedescendant={open ? rows[active]?.id : undefined}
    title={title ?? label}
    onclick={toggle}
    {onkeydown}
  >
    {#if current?.dot}<span class="pdot" data-p={current.dot} aria-hidden="true"></span>{/if}
    {#if icon}<span class="ico" aria-hidden="true"><Icon name={icon} size="0.9rem" /></span>{/if}
    <span class="value">{current?.label ?? fallback ?? "—"}</span>
    <span class="caret" aria-hidden="true"><Icon name="down" size="0.85rem" /></span>
  </button>

  {#if open}
    <div
      id="{uid}-list"
      class="list popover"
      class:down={!up}
      class:end={alignEnd}
      role="listbox"
      aria-label={label}
      tabindex="-1"
      in:popIn={{ y: up ? 6 : -6 }}
      out:popOut={{ y: up ? 4 : -4 }}
    >
      <div class="popover-label" aria-hidden="true">{label}</div>
      {#each options as o, i (o.id)}
        {@render option(o, i)}
      {/each}

      {#if more.length}
        <div
          id="{uid}-more"
          class="row more"
          class:active={rows[active]?.kind === "more"}
          role="option"
          tabindex="-1"
          aria-selected="false"
          aria-label="{moreLabel}, {expanded ? 'abierto' : 'cerrado'}"
          onpointermove={() => (active = options.length)}
          onclick={() => pick(options.length)}
          onkeydown={() => {}}
        >
          <span class="check"></span>
          <span class="name">{moreLabel}</span>
          <span class="caret small" class:openchev={expanded} aria-hidden="true"><Icon name="down" size="0.85rem" /></span>
        </div>
        <Curtain open={expanded}>
          <div class="extra" role="group" aria-label={moreLabel}>
            {#if searchable}
              <div class="search">
                <Icon name="search" size="0.9rem" />
                <input
                  bind:this={search}
                  bind:value={query}
                  placeholder="Buscar modelo"
                  aria-label="Buscar en {moreLabel}"
                  aria-controls="{uid}-list"
                  aria-activedescendant={rows[active]?.id}
                  spellcheck="false"
                  onkeydown={navigate}
                />
              </div>
            {/if}
            {#each filtered as o, j (o.id)}
              {@render option(o, options.length + 1 + j)}
            {/each}
            {#if filtered.length === 0}
              <p class="empty">{query ? "Ningún modelo coincide." : "Cargando la lista…"}</p>
            {:else if hiddenCount > 0}
              <p class="empty">{hiddenCount} más. Escribe para afinar la búsqueda.</p>
            {/if}
          </div>
        </Curtain>
      {/if}
    </div>
  {/if}
</div>

{#snippet option(o: Choice, i: number)}
  <div
    id="{uid}-{o.id}"
    class="row"
    class:active={rows[active]?.id === `${uid}-${o.id}`}
    class:selected={o.id === value}
    role="option"
    tabindex="-1"
    aria-selected={o.id === value}
    onpointermove={() => (active = i)}
    onclick={() => pick(i)}
    onkeydown={() => {}}
  >
    <span class="check" aria-hidden="true">{#if o.id === value}<Icon name="check" size="0.85rem" />{/if}</span>
    <span class="text">
      <span class="name">{#if o.dot}<span class="pdot" data-p={o.dot}></span>{/if}{o.label}</span>
      {#if o.hint}<span class="hint">{o.hint}</span>{/if}
    </span>
    {#if o.note}<span class="note">{o.note}</span>{/if}
  </div>
{/snippet}

<style>
  .menu {
    position: relative;
    display: inline-block;
    min-width: 0;
  }
  .trigger {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    max-width: 100%;
    height: var(--control-sm);
    padding: 0 0.4rem 0 0.5rem;
    border: 0;
    border-radius: var(--radius);
    background: transparent;
    color: var(--text-2);
    font-size: var(--fs-xs);
    font-weight: 500;
    transition:
      background-color var(--dur-fast) var(--ease),
      color var(--dur-fast) var(--ease);
  }
  .trigger:hover,
  .trigger[aria-expanded="true"] {
    background: var(--raised-2);
    color: var(--text);
  }
  .ico {
    display: grid;
    color: var(--text-3);
  }
  .trigger:hover .ico,
  .trigger[aria-expanded="true"] .ico {
    color: var(--text-2);
  }
  .value {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .caret {
    display: grid;
    opacity: 0.6;
    transition: transform var(--dur-base) var(--ease);
  }
  .trigger[aria-expanded="true"] .caret {
    transform: rotate(180deg);
  }
  .caret.small.openchev {
    transform: rotate(180deg);
  }
  .list {
    position: absolute;
    bottom: calc(100% + 0.35rem);
    left: 0;
    min-width: 15.5rem;
    max-height: min(28rem, 70vh);
    overflow-y: auto;
    overscroll-behavior: contain;
  }
  .list:focus-visible {
    outline: none;
  }
  .list.down {
    bottom: auto;
    top: calc(100% + 0.35rem);
  }
  .list.end {
    left: auto;
    right: 0;
  }
  .row {
    display: grid;
    grid-template-columns: 1rem minmax(0, 1fr) auto;
    align-items: start;
    gap: 0.45rem;
    padding: 0.38rem 0.55rem 0.38rem 0.45rem;
    border-radius: var(--radius);
    cursor: pointer;
  }
  .row.active {
    background: var(--raised-2);
  }
  .check {
    display: grid;
    place-items: center;
    height: 1.25rem;
    color: var(--accent);
  }
  .text {
    display: grid;
    min-width: 0;
  }
  .name {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    font-size: var(--fs-sm);
    line-height: 1.25rem;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .hint {
    font-size: var(--fs-xs);
    color: var(--text-3);
    line-height: 1.35;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .note {
    padding-top: 0.15rem;
    font-size: var(--fs-xs);
    color: var(--text-3);
    white-space: nowrap;
  }
  .row.more {
    grid-template-columns: 1rem minmax(0, 1fr) auto;
    align-items: center;
    margin-top: 0.25rem;
    padding-top: 0.45rem;
    border-top: 1px solid var(--line-soft);
    border-radius: 0 0 var(--radius) var(--radius);
  }
  .row.more .name {
    color: var(--text-2);
  }
  .extra {
    padding-top: 0.15rem;
  }
  .search {
    position: sticky;
    top: 0;
    z-index: 1;
    display: flex;
    align-items: center;
    gap: 0.4rem;
    margin: 0.15rem 0 0.25rem;
    padding: 0 0.55rem;
    height: var(--control-sm);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--bg);
    color: var(--text-3);
  }
  .search:focus-within {
    border-color: var(--accent);
  }
  .search input {
    flex: 1;
    min-width: 0;
    border: 0;
    background: none;
    font-size: var(--fs-sm);
    color: var(--text);
  }
  .search input:focus {
    outline: none;
  }
  .search input::placeholder {
    color: var(--text-3);
  }
  .empty {
    margin: 0.3rem 0.55rem 0.2rem;
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
</style>
