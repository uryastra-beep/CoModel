<script lang="ts" module>
  import type { IconName } from "./Icon.svelte";

  export interface Action {
    label: string;
    icon?: IconName;
    keys?: string;
    danger?: boolean;
    disabled?: boolean;
    /** Separador antes de esta acción. */
    sep?: boolean;
    run: () => void;
  }
</script>

<script lang="ts">
  // Menú de acciones (los tres puntos). Teclado: flechas, Enter, Escape.
  import { tick } from "svelte";
  import Icon from "./Icon.svelte";
  import { popIn, popOut } from "./motion";

  interface Props {
    label: string;
    actions: Action[];
    /** Abre hacia abajo (por defecto) o hacia arriba. */
    up?: boolean;
  }
  let { label, actions, up = false }: Props = $props();

  const uid = $props.id();
  let open = $state(false);
  let active = $state(0);
  let root: HTMLDivElement;
  let button: HTMLButtonElement;
  let list = $state<HTMLDivElement>();

  async function toggle() {
    open = !open;
    if (open) {
      active = actions.findIndex((a) => !a.disabled);
      await tick();
      list?.focus();
    }
  }

  function run(i: number) {
    const a = actions[i];
    if (!a || a.disabled) return;
    open = false;
    button.focus();
    a.run();
  }

  function step(dir: 1 | -1) {
    const n = actions.length;
    for (let k = 1; k <= n; k++) {
      const i = (active + dir * k + n) % n;
      if (!actions[i].disabled) {
        active = i;
        return;
      }
    }
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "ArrowDown") step(1);
    else if (e.key === "ArrowUp") step(-1);
    else if (e.key === "Enter" || e.key === " ") run(active);
    else if (e.key === "Escape") {
      e.stopPropagation();
      open = false;
      button.focus();
    } else if (e.key === "Tab") open = false;
    else return;
    if (e.key !== "Tab") e.preventDefault();
  }

  $effect(() => {
    if (!open) return;
    const close = (e: PointerEvent) => {
      if (!root.contains(e.target as Node)) open = false;
    };
    window.addEventListener("pointerdown", close, true);
    return () => window.removeEventListener("pointerdown", close, true);
  });
</script>

<div class="wrap" bind:this={root}>
  <button
    bind:this={button}
    class="icon-btn sm"
    aria-label={label}
    title={label}
    aria-haspopup="menu"
    aria-expanded={open}
    aria-controls="{uid}-menu"
    onclick={toggle}
  >
    <Icon name="more" size="1rem" />
  </button>
  {#if open}
    <div
      bind:this={list}
      id="{uid}-menu"
      class="menu popover"
      class:up
      role="menu"
      aria-label={label}
      tabindex="-1"
      aria-activedescendant="{uid}-{active}"
      {onkeydown}
      in:popIn={{ y: up ? 6 : -6 }}
      out:popOut={{ y: up ? 4 : -4 }}
    >
      {#each actions as a, i (a.label)}
        {#if a.sep}<div class="popover-sep" role="separator"></div>{/if}
        <div
          id="{uid}-{i}"
          class="item"
          class:active={i === active}
          class:danger={a.danger}
          class:disabled={a.disabled}
          role="menuitem"
          tabindex="-1"
          aria-disabled={a.disabled}
          onpointermove={() => !a.disabled && (active = i)}
          onclick={() => run(i)}
          onkeydown={() => {}}
        >
          <span class="ico" aria-hidden="true">{#if a.icon}<Icon name={a.icon} size="0.95rem" />{/if}</span>
          <span class="label">{a.label}</span>
          {#if a.keys}<kbd>{a.keys}</kbd>{/if}
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .wrap {
    position: relative;
  }
  .menu {
    position: absolute;
    top: calc(100% + 0.3rem);
    right: 0;
    min-width: 14rem;
  }
  .menu.up {
    top: auto;
    bottom: calc(100% + 0.3rem);
  }
  .menu:focus-visible {
    outline: none;
  }
  .item {
    display: grid;
    grid-template-columns: 1.1rem minmax(0, 1fr) auto;
    align-items: center;
    gap: 0.55rem;
    height: 2rem;
    padding: 0 0.6rem 0 0.5rem;
    border-radius: var(--radius);
    font-size: var(--fs-sm);
    color: var(--text);
    cursor: pointer;
    white-space: nowrap;
  }
  .item.active {
    background: var(--raised-2);
  }
  .item.disabled {
    color: var(--text-3);
    cursor: default;
  }
  .item.danger {
    color: var(--danger);
  }
  .ico {
    display: grid;
    place-items: center;
    color: var(--text-3);
  }
  .item.danger .ico {
    color: inherit;
  }
</style>
