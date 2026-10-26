<script lang="ts">
  import type { Answer, AskQuestion } from "./backend";
  import Icon from "./Icon.svelte";

  export interface QuestionItem {
    id: string;
    questions: AskQuestion[];
    state: "pending" | "answered" | "denied" | "cancelled";
    answers?: Record<string, string>;
  }

  interface Props {
    item: QuestionItem;
    /** Nombre de la IA que pregunta. */
    provider?: string;
    onanswer: (answer: Answer, answers?: Record<string, string>) => void;
  }
  let { item, provider = "La IA", onanswer }: Props = $props();

  const uid = $props.id();
  // Lo elegido por pregunta (varias opciones si es de selección múltiple) y el texto libre de "Otra respuesta".
  let picked = $state<Record<string, string[]>>({});
  let other = $state<Record<string, string>>({});

  function toggle(q: AskQuestion, label: string, on: boolean) {
    const cur = picked[q.question] ?? [];
    picked[q.question] = q.multiSelect ? (on ? [...cur, label] : cur.filter((x) => x !== label)) : [label];
  }

  const valueOf = (q: AskQuestion) => [...(picked[q.question] ?? []), ...(other[q.question]?.trim() ? [other[q.question].trim()] : [])];
  const complete = $derived(item.questions.every((q) => valueOf(q).length > 0));

  function submit() {
    const answers: Record<string, string> = {};
    for (const q of item.questions) answers[q.question] = valueOf(q).join(", ");
    onanswer({ kind: "answers", answers }, answers);
  }
</script>

{#if item.state === "pending"}
  <section class="ask" role="group" aria-label="Pregunta de {provider}">
    <p class="who"><Icon name="question" size="0.9rem" />{provider} te pregunta</p>
    {#each item.questions as q, qi (q.question)}
      <fieldset>
        <legend>{#if q.header}<span class="chip">{q.header}</span>{/if}{q.question}</legend>
        {#each q.options as o}
          <label class="opt">
            <input
              type={q.multiSelect ? "checkbox" : "radio"}
              name="{uid}-{qi}"
              checked={picked[q.question]?.includes(o.label) ?? false}
              onchange={(e) => toggle(q, o.label, e.currentTarget.checked)}
            />
            <span class="t"><span class="l">{o.label}</span>{#if o.description}<span class="d">{o.description}</span>{/if}</span>
          </label>
        {/each}
        <input class="input other" placeholder="Otra respuesta" bind:value={other[q.question]} aria-label="Otra respuesta para: {q.question}" />
      </fieldset>
    {/each}
    <div class="actions">
      <button class="btn primary sm" disabled={!complete} onclick={submit}>Responder</button>
      <button class="btn ghost sm" onclick={() => onanswer({ kind: "deny", message: "El usuario prefirió no responder." })}>Prefiero no responder</button>
    </div>
  </section>
{:else if item.state === "answered" && item.answers}
  <div class="done"><Icon name="check" size="0.8rem" /><span>Respondiste: {Object.values(item.answers).join(" · ")}</span></div>
{:else}
  <div class="done bad"><Icon name="x" size="0.8rem" /><span>{item.state === "denied" ? "No respondiste la pregunta." : "Pregunta sin respuesta."}</span></div>
{/if}

<style>
  .ask {
    display: grid;
    gap: 0.8rem;
    margin: 0.5rem 0.9rem;
    padding: 0.75rem 0.85rem 0.8rem;
    border: 1px solid color-mix(in oklch, var(--accent) 45%, var(--line));
    border-radius: var(--radius-lg);
    background: color-mix(in oklch, var(--accent) 5%, var(--surface));
  }
  .who {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    margin: 0;
    font-size: var(--fs-xs);
    font-weight: 500;
    color: var(--accent);
  }
  fieldset {
    display: grid;
    gap: 0.2rem;
    margin: 0;
    padding: 0;
    border: 0;
  }
  legend {
    padding: 0;
    margin-bottom: 0.35rem;
    font-weight: 600;
  }
  .chip {
    margin-right: 0.5rem;
    padding: 0.05rem 0.45rem;
    border-radius: var(--radius-sm);
    background: var(--accent-tint);
    color: var(--accent);
    font-size: var(--fs-xs);
    font-weight: 500;
  }
  .opt {
    display: flex;
    gap: 0.6rem;
    align-items: flex-start;
    padding: 0.35rem 0.45rem;
    border-radius: var(--radius);
    cursor: pointer;
    transition: background-color var(--dur-fast) var(--ease);
  }
  .opt:hover {
    background: var(--raised);
  }
  .opt input {
    margin-top: 0.22rem;
    accent-color: var(--accent-solid);
  }
  .t {
    display: grid;
  }
  .d {
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .other {
    margin-top: 0.25rem;
    height: var(--control-sm);
  }
  .actions {
    display: flex;
    gap: 0.4rem;
  }
  .done {
    display: flex;
    align-items: flex-start;
    gap: 0.5rem;
    padding: 0.12rem 1.4rem;
    font-size: var(--fs-xs);
    color: var(--text-3);
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
