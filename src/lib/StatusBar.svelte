<script lang="ts">
  import { onMount } from "svelte";
  import { canvas } from "./canvas.svelte";
  import Icon from "./Icon.svelte";
  import { providerLabel, windowLabel, windowTitle } from "./providers";
  import { openSettings } from "./ui.svelte";
  import { ago, handoffs, resetsIn, usage } from "./usage.svelte";

  const providers = $derived(Object.entries(usage.byProvider));
  const tone = (f: number) => (f >= 0.9 ? "danger" : f >= 0.7 ? "warn" : "");
  const working = $derived(canvas.cards.filter((c) => c.busy).length);

  // La hora relativa del último relevo se refresca sola.
  let now = $state(Date.now());
  onMount(() => {
    const t = setInterval(() => (now = Date.now()), 30_000);
    return () => clearInterval(t);
  });
</script>

<footer>
  <div class="group" aria-label="Estado de las IAs">
    <span class="state">
      <span class="activity" class:on={working > 0} aria-hidden="true"></span>
      {#if working > 0}{working === 1 ? "1 chat trabajando" : `${working} chats trabajando`}{:else}En reposo{/if}
    </span>

    {#each providers as [id, windows] (id)}
      <button class="prov" onclick={() => openSettings("uso")} title="Ver el uso en Ajustes">
        <span class="pdot" data-p={id} aria-hidden="true"></span>
        <span class="pname">{providerLabel(id)}</span>
        {#each Object.entries(windows) as [w, q] (w)}
          <span class="win" title="{windowTitle(w)}{resetsIn(q.resetsAt) ? `, ${resetsIn(q.resetsAt)}` : ''}">
            <span class="wl">{windowLabel(w)}</span>
            <span class="track" aria-hidden="true"><span class="fill {tone(q.fraction)}" style:width="{Math.round(q.fraction * 100)}%"></span></span>
            <b class={tone(q.fraction)}>{Math.round(q.fraction * 100)}%</b>
          </span>
        {/each}
      </button>
    {:else}
      <span class="muted">Aún sin datos de uso</span>
    {/each}
  </div>

  <span class="relevo" class:muted={!handoffs.last}>
    <Icon name="relay" size="0.8rem" />
    {#if handoffs.last}
      Último relevo: {providerLabel(handoffs.last.from)} a {providerLabel(handoffs.last.to)}, {ago(handoffs.last.at, now)}
    {:else}
      Sin relevos
    {/if}
  </span>
</footer>

<style>
  footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    height: 1.9rem;
    padding: 0 0.9rem 0 1rem;
    border-top: 1px solid var(--line-soft);
    background: var(--surface);
    font-size: var(--fs-xs);
    color: var(--text-2);
  }
  .group {
    display: flex;
    align-items: center;
    gap: 1.1rem;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
  }
  .state {
    display: inline-flex;
    align-items: center;
    gap: 0.45rem;
    color: var(--text);
  }
  .muted {
    color: var(--text-3);
  }
  .prov {
    display: inline-flex;
    align-items: center;
    gap: 0.6rem;
    height: 1.4rem;
    padding: 0 0.4rem;
    margin: 0 -0.4rem;
    border: 0;
    border-radius: var(--radius-sm);
    background: none;
    font-size: inherit;
    color: inherit;
    transition: background-color var(--dur-fast) var(--ease);
  }
  .prov:hover {
    background: var(--raised);
  }
  .pname {
    color: var(--text);
    font-weight: 500;
  }
  .win {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    font-variant-numeric: tabular-nums;
  }
  .wl {
    color: var(--text-3);
  }
  .track {
    width: 2.4rem;
    height: 0.25rem;
    border-radius: 1rem;
    background: var(--raised-2);
    overflow: hidden;
  }
  .fill {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: var(--text-3);
    transition: width var(--dur-slow) var(--ease);
  }
  .fill.warn {
    background: var(--warn);
  }
  .fill.danger {
    background: var(--danger);
  }
  b {
    min-width: 2.4ch;
    font-weight: 500;
    color: var(--text-2);
  }
  b.warn {
    color: var(--warn);
  }
  b.danger {
    color: var(--danger);
  }
  .relevo {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
</style>
