<script lang="ts">
  import type { Answer } from "./backend";
  import DiffView from "./DiffView.svelte";
  import { isEditTool, toolDiff } from "./diff";
  import Icon from "./Icon.svelte";

  export interface PermissionItem {
    id: string;
    tool: string;
    input: Record<string, unknown>;
    description: string | null;
    canAcceptEdits: boolean;
    state: "pending" | "allowed" | "always" | "denied" | "cancelled";
  }

  interface Props {
    item: PermissionItem;
    /** Nombre de la IA que pide permiso. */
    provider?: string;
    onanswer: (answer: Answer) => void;
  }
  let { item, provider = "La IA", onanswer }: Props = $props();

  const diff = $derived(isEditTool(item.tool) ? toolDiff(item.tool, item.input) : null);
  const path = $derived((item.input.file_path ?? item.input.notebook_path ?? item.input.path) as string | undefined);
  const command = $derived(item.tool === "Bash" && typeof item.input.command === "string" ? item.input.command : null);

  const title = $derived.by(() => {
    if (item.tool === "Write") return diff?.created ? `Crear ${path}` : `Reescribir ${path}`;
    if (item.tool === "Edit" || item.tool === "MultiEdit") return `Editar ${path}`;
    if (item.tool === "Bash") return "Ejecutar un comando";
    if (item.tool === "Read") return `Leer ${path}`;
    if (item.tool === "WebFetch" && typeof item.input.url === "string") return `Abrir ${item.input.url}`;
    if (item.tool === "WebSearch" && typeof item.input.query === "string") return `Buscar en la web: ${item.input.query}`;
    return `Usar la herramienta ${item.tool}`;
  });

  const resolved = $derived(
    { allowed: "Permitiste", always: "Permitiste (y las ediciones siguientes)", denied: "Rechazaste", cancelled: "Sin respuesta" }[item.state as "allowed" | "always" | "denied" | "cancelled"],
  );
</script>

{#if item.state === "pending"}
  <section class="ask" role="group" aria-label="Permiso: {title}">
    <p class="who"><Icon name="shield" size="0.9rem" />{provider} pide permiso</p>
    <p class="title">{title}</p>
    {#if diff && diff.rows.length}
      <DiffView rows={diff.rows} />
    {:else if command}
      <pre class="cmd">{command}</pre>
    {/if}
    <div class="actions">
      <button class="btn primary sm" onclick={() => onanswer({ kind: "allow", always: false })}>Permitir</button>
      {#if item.canAcceptEdits && isEditTool(item.tool)}
        <button class="btn sm" onclick={() => onanswer({ kind: "allow", always: true })}>Permitir todas las ediciones de este chat</button>
      {/if}
      <button class="btn ghost sm" onclick={() => onanswer({ kind: "deny" })}>Rechazar</button>
    </div>
  </section>
{:else}
  <div class="done" class:bad={item.state === "denied"}><Icon name={item.state === "denied" ? "x" : "check"} size="0.8rem" /><span>{resolved}: {title}</span></div>
{/if}

<style>
  .ask {
    display: grid;
    gap: 0.5rem;
    margin: 0.5rem 0.9rem;
    padding: 0.75rem 0.85rem 0.8rem;
    border: 1px solid color-mix(in oklch, var(--warn) 45%, var(--line));
    border-radius: var(--radius-lg);
    background: color-mix(in oklch, var(--warn) 6%, var(--surface));
  }
  .who {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    margin: 0;
    font-size: var(--fs-xs);
    font-weight: 500;
    color: var(--warn);
  }
  .title {
    margin: -0.2rem 0 0;
    font-weight: 600;
    overflow-wrap: anywhere;
  }
  .cmd {
    margin: 0;
    padding: 0.5rem 0.65rem;
    overflow-x: auto;
    background: var(--bg);
    border: 1px solid var(--line-soft);
    border-radius: var(--radius);
    font-family: var(--mono);
    font-size: var(--fs-xs);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
    margin-top: 0.1rem;
  }
  .done {
    display: flex;
    align-items: flex-start;
    gap: 0.5rem;
    padding: 0.12rem 1.4rem;
    font-size: var(--fs-xs);
    color: var(--text-3);
    overflow-wrap: anywhere;
  }
  .done :global(.icon) {
    margin-top: 0.15rem;
    color: var(--ok);
  }
  .done.bad,
  .done.bad :global(.icon) {
    color: var(--warn);
  }
</style>
