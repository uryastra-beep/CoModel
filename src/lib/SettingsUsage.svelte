<script lang="ts">
  import { onMount } from "svelte";
  import { usageSummary, type UsageSummary } from "./backend";
  import { modelLabel as aiModelLabel } from "./ai.svelte";
  import Icon from "./Icon.svelte";
  import { formatTokens, providerLabel, windowLabel, windowTitle } from "./providers";
  import { resetsIn, usage } from "./usage.svelte";

  let data = $state<UsageSummary | null>(null);
  let failed = $state(false);
  let table = $state(false);
  let hover = $state<number | null>(null);

  async function load() {
    try {
      data = await usageSummary(7);
      failed = false;
    } catch {
      failed = true;
    }
  }

  onMount(load);

  const modelLabel = (id: string, provider: string) => aiModelLabel(provider, id) || id;

  const max = $derived(Math.max(1, ...(data?.by_day.map((d) => d.tokens) ?? [0])));
  const maxModel = $derived(Math.max(1, ...(data?.by_model.map((m) => m.tokens) ?? [0])));
  const maxChat = $derived(Math.max(1, ...(data?.by_chat.map((c) => c.tokens) ?? [0])));
  const totalTokens = $derived((data?.totals.input_tokens ?? 0) + (data?.totals.output_tokens ?? 0));

  const dayDate = (d: string) => new Date(`${d}T12:00:00`);
  const dayShort = (d: string) => dayDate(d).toLocaleDateString("es", { weekday: "short" }).replace(".", "");
  const dayLong = (d: string) => dayDate(d).toLocaleDateString("es", { weekday: "long", day: "numeric", month: "long" });
  const money = (n: number) => (n > 0 && n < 0.01 ? "menos de 0,01" : n.toLocaleString("es", { minimumFractionDigits: 2, maximumFractionDigits: 2 }));
  const quotas = $derived(Object.entries(usage.byProvider));
</script>

<h2 class="s-title">Uso</h2>
<div class="bar">
  <p class="s-lead">Lo que se ha usado desde CoModel en los últimos 7 días, en todos tus proyectos y chats.</p>
  <button class="btn sm" onclick={load}><Icon name="refresh" size="0.85rem" />Actualizar</button>
</div>

{#if failed}
  <p class="empty" role="alert">No se pudo leer el uso. Inténtalo de nuevo.</p>
{:else if !data}
  <p class="empty">Cargando</p>
{:else if data.totals.turns === 0}
  <p class="empty">Todavía no hay uso esta semana. Aparecerá aquí cuando mandes mensajes desde un chat.</p>
{:else}
  <dl class="totals">
    <div><dt>Mensajes</dt><dd>{data.totals.turns}</dd></div>
    <div><dt>Tokens</dt><dd>{formatTokens(totalTokens)}</dd></div>
    <div>
      <dt>Coste equivalente</dt>
      <dd>{money(data.totals.cost_usd)} <span class="unit">USD aprox.</span></dd>
    </div>
  </dl>

  <section aria-labelledby="u-days">
    <div class="bar sub">
      <h3 id="u-days">Por día</h3>
      <button class="link" onclick={() => (table = !table)}>{table ? "Ver gráfico" : "Ver como tabla"}</button>
    </div>

    {#if table}
      <table>
        <thead><tr><th>Día</th><th class="n">Mensajes</th><th class="n">Tokens</th></tr></thead>
        <tbody>
          {#each data.by_day as d}
            <tr><td>{dayLong(d.day)}</td><td class="n">{d.turns}</td><td class="n">{d.tokens.toLocaleString("es")}</td></tr>
          {/each}
        </tbody>
      </table>
    {:else}
      <div class="chart" role="group" aria-label="Tokens por día, últimos 7 días">
        {#each data.by_day as d, i (d.day)}
          {@const h = (d.tokens / max) * 100}
          <!-- Cada columna se enfoca con el teclado para ver su detalle; el aria-label ya lo dice completo. -->
          <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
          <div
            class="col"
            role="img"
            tabindex="0"
            aria-label="{dayLong(d.day)}: {d.tokens.toLocaleString('es')} tokens en {d.turns} mensajes"
            onpointerenter={() => (hover = i)}
            onpointerleave={() => (hover = null)}
            onfocus={() => (hover = i)}
            onblur={() => (hover = null)}
          >
            <div class="plot">
              {#if d.tokens === max}<span class="top">{formatTokens(d.tokens)}</span>{/if}
              <span class="mark" style:height="{Math.max(h, d.tokens ? 2 : 0)}%"></span>
            </div>
            <span class="x">{dayShort(d.day)}</span>
            {#if hover === i}
              <span class="tip" role="presentation">
                <b>{dayLong(d.day)}</b>
                {d.tokens.toLocaleString("es")} tokens, {d.turns} {d.turns === 1 ? "mensaje" : "mensajes"}
              </span>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  </section>

  <section aria-labelledby="u-models">
    <h3 id="u-models">Modelos más usados</h3>
    <ol class="rank">
      {#each data.by_model as m (m.model + m.provider)}
        <li>
          <span class="rname" title={m.model}><span class="pdot" data-p={m.provider} aria-hidden="true"></span>{modelLabel(m.model, m.provider)} <span class="who">{providerLabel(m.provider)}</span></span>
          <span class="track"><span class="fill" style:width="{(m.tokens / maxModel) * 100}%"></span></span>
          <span class="val">{formatTokens(m.tokens)}<span class="sub">{m.turns} {m.turns === 1 ? "mensaje" : "mensajes"}</span></span>
        </li>
      {/each}
    </ol>
  </section>

  <section aria-labelledby="u-chats">
    <h3 id="u-chats">Sesiones más activas</h3>
    <ol class="rank">
      {#each data.by_chat as c (c.chat)}
        <li>
          <span class="rname" title={c.title}>{c.title}</span>
          <span class="track"><span class="fill" style:width="{(c.tokens / maxChat) * 100}%"></span></span>
          <span class="val">{formatTokens(c.tokens)}<span class="sub">{c.turns} {c.turns === 1 ? "mensaje" : "mensajes"}</span></span>
        </li>
      {/each}
    </ol>
  </section>

  <p class="foot">Los tokens incluyen lo que la IA lee en cada mensaje. El coste de Claude es el equivalente a precio de lista; con suscripción no se cobra aparte.</p>
{/if}

<section aria-labelledby="u-limits">
  <h3 id="u-limits">Límites de tus cuentas</h3>
  {#if quotas.length === 0}
    <p class="empty small">Aparecen cuando mandas el primer mensaje.</p>
  {:else}
    <ul class="limits">
      {#each quotas as [id, windows] (id)}
        {#each Object.entries(windows) as [w, q] (id + w)}
          <li title={resetsIn(q.resetsAt)}>
            <span class="lname"><span class="pdot" data-p={id} aria-hidden="true"></span>{providerLabel(id)} <span class="who">{windowTitle(w).toLowerCase()}</span></span>
            <span class="track"><span class="fill" class:warn={q.fraction >= 0.7} class:danger={q.fraction >= 0.9} style:width="{Math.round(q.fraction * 100)}%"></span></span>
            <span class="val">{Math.round(q.fraction * 100)}%<span class="sub">{windowLabel(w)}</span></span>
          </li>
        {/each}
      {/each}
    </ul>
  {/if}
</section>

<style>
  .bar {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 1rem;
  }
  .bar.sub {
    margin-bottom: 0.5rem;
  }
  h3 {
    margin: 0;
    font-size: var(--fs-sm);
    font-weight: 600;
    color: var(--text);
  }
  section {
    margin-top: 1.6rem;
  }
  section > h3 {
    margin-bottom: 0.55rem;
  }
  .bar.sub .link {
    font-size: var(--fs-xs);
  }
  .empty {
    margin: 0.5rem 0;
    max-width: 52ch;
    color: var(--text-3);
  }
  .empty.small {
    font-size: var(--fs-sm);
  }

  .totals {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    margin: 0;
    border: 1px solid var(--line-soft);
    border-radius: var(--radius-lg);
    background: var(--bg);
  }
  .totals > div {
    padding: 0.8rem 1rem;
  }
  .totals > div + div {
    border-left: 1px solid var(--line-soft);
  }
  .totals dt {
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .totals dd {
    margin: 0;
    font-size: var(--fs-lg);
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
  .unit {
    font-size: var(--fs-xs);
    font-weight: 400;
    color: var(--text-3);
  }

  /* Gráfico de columnas: marcas finas (máx. 24 px), extremo superior redondeado, base recta. */
  .chart {
    display: grid;
    grid-template-columns: repeat(7, minmax(0, 1fr));
    gap: 0.25rem;
    padding-top: 1.1rem;
    border-bottom: 1px solid var(--line);
  }
  .col {
    position: relative;
    display: grid;
    justify-items: center;
    gap: 0.3rem;
    padding: 0 0.15rem 0.35rem;
    border-radius: var(--radius);
    cursor: default;
  }
  .col:hover,
  .col:focus-visible {
    background: var(--raised);
  }
  .plot {
    position: relative;
    display: flex;
    align-items: flex-end;
    justify-content: center;
    width: 100%;
    height: 7rem;
  }
  .mark {
    display: block;
    width: min(100%, 1.5rem);
    border-radius: 4px 4px 0 0;
    background: var(--accent-solid);
    transform-origin: bottom;
    animation: grow var(--dur-slow) var(--ease) both;
  }
  @keyframes grow {
    from {
      transform: scaleY(0);
    }
  }
  .top {
    position: absolute;
    top: -1.15rem;
    left: 50%;
    transform: translateX(-50%);
    font-size: var(--fs-xs);
    color: var(--text-2);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .x {
    font-size: var(--fs-xs);
    color: var(--text-3);
    text-transform: capitalize;
  }
  .tip {
    position: absolute;
    bottom: calc(100% + 0.2rem);
    left: 50%;
    transform: translateX(-50%);
    z-index: 5;
    display: grid;
    gap: 0.1rem;
    padding: 0.4rem 0.6rem;
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    box-shadow: var(--lift);
    font-size: var(--fs-xs);
    color: var(--text-2);
    white-space: nowrap;
    pointer-events: none;
  }
  .tip b {
    color: var(--text);
    font-weight: 600;
    text-transform: capitalize;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--fs-sm);
  }
  th,
  td {
    padding: 0.4rem 0.5rem;
    text-align: left;
    border-bottom: 1px solid var(--line-soft);
  }
  th {
    font-weight: 600;
    color: var(--text-3);
    font-size: var(--fs-xs);
  }
  .n {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  .rank,
  .limits {
    display: grid;
    gap: 0.5rem;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .rank li,
  .limits li {
    display: grid;
    grid-template-columns: minmax(8rem, 13rem) minmax(0, 1fr) 6.5rem;
    align-items: center;
    gap: 0.75rem;
  }
  .rname,
  .lname {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .who {
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .track {
    height: 0.5rem;
    border-radius: 0.25rem;
    background: var(--raised-2);
    overflow: hidden;
  }
  .fill {
    display: block;
    height: 100%;
    border-radius: 0 4px 4px 0;
    background: var(--accent-solid);
    transform-origin: left;
    animation: sweep var(--dur-slow) var(--ease) both;
  }
  .fill.warn {
    background: var(--warn);
  }
  .fill.danger {
    background: var(--danger);
  }
  @keyframes sweep {
    from {
      transform: scaleX(0);
    }
  }
  .val {
    display: grid;
    justify-items: end;
    font-variant-numeric: tabular-nums;
    line-height: 1.25;
  }
  .sub {
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .foot {
    margin: 1.4rem 0 0;
    max-width: 60ch;
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
</style>
