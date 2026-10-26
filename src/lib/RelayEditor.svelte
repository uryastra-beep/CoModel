<script lang="ts">
  import { connections, statusOf } from "./connections.svelte";
  import Icon from "./Icon.svelte";
  import { PROVIDER_INFO, SELECTABLE } from "./providers";

  interface Props {
    /** IAs elegidas, en orden de relevo. */
    relay: string[];
    onchange: (relay: string[]) => void;
  }
  let { relay, onchange }: Props = $props();

  // Primero las elegidas, en su orden; después las que no se usan.
  const rows = $derived([...relay, ...SELECTABLE.filter((id) => !relay.includes(id))]);

  const toggle = (id: string, on: boolean) => onchange(on ? [...relay, id] : relay.filter((x) => x !== id));

  function move(id: string, delta: number) {
    const i = relay.indexOf(id);
    const j = i + delta;
    if (i < 0 || j < 0 || j >= relay.length) return;
    const next = [...relay];
    [next[i], next[j]] = [next[j], next[i]];
    onchange(next);
  }

  function note(id: string): string {
    const st = statusOf(id);
    if (!connections.loaded || !st || st.ready) return "";
    return id === "openrouter" ? "Sin clave: no se usará hasta que la guardes arriba." : id === "codex" ? "Codex no está listo: revisa su estado arriba." : "No se encontró en este equipo.";
  }
</script>

<ol class="relay s-rows" aria-label="IAs en orden de relevo">
  {#each rows as id (id)}
    {@const on = relay.includes(id)}
    {@const i = relay.indexOf(id)}
    <li class:off={!on}>
      <span class="num" aria-hidden="true">{on ? i + 1 : "–"}</span>
      <label class="pick">
        <input type="checkbox" checked={on} disabled={on && relay.length === 1} onchange={(e) => toggle(id, e.currentTarget.checked)} />
        <span class="pdot" data-p={id} aria-hidden="true"></span>
        <span class="name">{PROVIDER_INFO[id]?.label ?? id}</span>
        <span class="via">{PROVIDER_INFO[id]?.via}</span>
      </label>
      {#if on}
        <span class="moves">
          <button type="button" class="icon-btn sm" onclick={() => move(id, -1)} disabled={i === 0} aria-label="Subir {PROVIDER_INFO[id]?.label}" title="Subir"><Icon name="up" size="0.95rem" /></button>
          <button type="button" class="icon-btn sm" onclick={() => move(id, 1)} disabled={i === relay.length - 1} aria-label="Bajar {PROVIDER_INFO[id]?.label}" title="Bajar"><Icon name="down" size="0.95rem" /></button>
        </span>
      {/if}
      {#if note(id)}<span class="note">{note(id)}</span>{/if}
    </li>
  {/each}
</ol>

<style>
  .relay {
    margin: 0;
    padding: 0.25rem;
    list-style: none;
  }
  li {
    display: grid;
    grid-template-columns: 1.4rem minmax(0, 1fr) auto;
    align-items: center;
    gap: 0.4rem;
    min-height: 2.4rem;
    padding: 0.2rem 0.4rem;
    border-radius: var(--radius);
  }
  li:hover {
    background: var(--raised);
  }
  .num {
    font-family: var(--mono);
    font-size: var(--fs-xs);
    color: var(--text-2);
    text-align: center;
    font-variant-numeric: tabular-nums;
  }
  li.off .num {
    color: var(--text-3);
  }
  .pick {
    display: flex;
    align-items: center;
    gap: 0.55rem;
    min-width: 0;
    cursor: pointer;
  }
  .pick input {
    accent-color: var(--accent-solid);
  }
  .name {
    font-weight: 500;
  }
  li.off .name {
    color: var(--text-3);
    font-weight: 400;
  }
  li.off .pdot {
    opacity: 0.45;
  }
  .via {
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .moves {
    display: flex;
    gap: 0.1rem;
  }
  .note {
    grid-column: 2 / -1;
    margin-top: -0.2rem;
    padding-bottom: 0.2rem;
    font-size: var(--fs-xs);
    color: var(--warn);
  }
</style>
