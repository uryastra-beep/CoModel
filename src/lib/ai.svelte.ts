// Qué IA usa cada chat y qué ofrece cada una: modelos, esfuerzos y modos.
// Claude tiene una lista fija; Codex y OpenRouter dan la suya (la cuenta del usuario y la lista pública).
import { chatgptModels, type ChatGPTModel, codexModels, openrouterModels, type CodexCatalog, type OpenRouterModel } from "./backend";
import { statusOf } from "./connections.svelte";
import { CLAUDE, type Choice } from "./providers";

export interface ChatAI {
  provider: string;
  /** Vacío: el modelo por defecto de la IA. */
  model: string;
  effort: string;
  mode: string;
}

/** Las IAs que se pueden elegir en un chat, en el orden en que se muestran. */
export const CHAT_PROVIDERS: { id: string; label: string; via: string }[] = [
  { id: "claude", label: "Claude", via: "Claude Code" },
  { id: "codex", label: "Codex", via: "Cuenta de ChatGPT" },
  { id: "chatgpt", label: "ChatGPT", via: "Conversación, imágenes y documentos" },
  { id: "openrouter", label: "OpenRouter", via: "Clave de API" },
];

export const catalog = $state<{
  codex: CodexCatalog | null;
  chatgpt: ChatGPTModel[] | null;
  chatgptError: string;
  openrouter: OpenRouterModel[] | null;
  openrouterError: string;
}>({ chatgpt: null, chatgptError: "", codex: null, openrouter: null, openrouterError: "" });

let codexLoading: Promise<void> | null = null;
let orLoading: Promise<void> | null = null;
let gptLoading: Promise<void> | null = null;
export function resetChatGPTCatalog() { catalog.chatgpt = null; catalog.chatgptError = ""; gptLoading = null; }

/** Pide la lista de modelos de una IA la primera vez que hace falta. */
export function loadCatalog(provider: string): Promise<void> {
  if (provider === "chatgpt") {
    gptLoading ??= chatgptModels().then((list) => { catalog.chatgpt = list; catalog.chatgptError = ""; }).catch((e) => { catalog.chatgptError = String(e); gptLoading = null; });
    return gptLoading;
  }
  if (provider === "codex") {
    codexLoading ??= codexModels()
      .then((c) => void (catalog.codex = c))
      .catch(() => void (catalog.codex = { models: [], default_model: null, default_effort: null }));
    return codexLoading;
  }
  if (provider === "openrouter") {
    orLoading ??= openrouterModels()
      .then((l) => void (catalog.openrouter = l))
      .catch((e) => {
        catalog.openrouterError = String(e);
        orLoading = null; // se puede reintentar
      });
    return orLoading;
  }
  return Promise.resolve();
}

// ---------- Esfuerzo ----------

const EFFORT: Record<string, { label: string; hint: string }> = {
  minimal: { label: "Mínimo", hint: "Casi sin razonar, lo más rápido" },
  low: { label: "Bajo", hint: "Piensa poco, responde rápido" },
  medium: { label: "Medio", hint: "Para la mayoría de tareas" },
  high: { label: "Alto", hint: "Razona más antes de responder" },
  xhigh: { label: "Extra", hint: "Para problemas difíciles" },
  max: { label: "Máximo", hint: "Usa más cuota; solo si hace falta" },
  ultra: { label: "Ultra", hint: "Máximo, y reparte el trabajo en subtareas" },
};
const effortChoice = (id: string): Choice => ({ id, label: EFFORT[id]?.label ?? id, hint: EFFORT[id]?.hint });

// ---------- OpenRouter ----------

/** Modelos recomendados de OpenRouter, si siguen en su lista. El resto se busca en "Ver más modelos". */
const OPENROUTER_PICKS = [
  "openai/gpt-6-luna",
  "openai/gpt-6.1-sol",
  "anthropic/claude-sonnet-5.5",
  "google/gemini-3.8-flash",
  "deepseek/deepseek-v4.1-flash",
  "openai/gpt-4o-mini",
];

/** "OpenAI: GPT-6 Luna" se muestra como "GPT-6 Luna"; el proveedor ya está en la pista. */
const shortName = (m: OpenRouterModel) => m.name.replace(/^[^:]+:\s*/, "");
const orChoice = (m: OpenRouterModel): Choice => ({ id: m.id, label: shortName(m) });

// ---------- Capacidades ----------

export interface ModelOptions {
  main: Choice[];
  more: Choice[];
  /** La lista "Ver más" es larga y se filtra escribiendo. */
  searchable: boolean;
}

export function modelsFor(provider: string): ModelOptions {
  if (provider === "chatgpt") {
    const models = catalog.chatgpt ?? [];
    return { main: models.length ? models.map((m) => ({ id: m.id, label: m.name })) : [{ id: "", label: "El de mi cuenta de ChatGPT" }], more: [], searchable: true };
  }
  if (provider === "codex") {
    const models = catalog.codex?.models ?? [];
    if (!models.length) return { main: [{ id: "", label: "El de Codex por defecto" }], more: [], searchable: false };
    return { main: models.map((m) => ({ id: m.id, label: m.label })), more: [], searchable: false };
  }
  if (provider === "openrouter") {
    const all = catalog.openrouter ?? [];
    const byId = new Map(all.map((m) => [m.id, m]));
    const picks = OPENROUTER_PICKS.filter((id) => !all.length || byId.has(id)).map((id) => (byId.has(id) ? orChoice(byId.get(id)!) : { id, label: id }));
    const more = all.filter((m) => !OPENROUTER_PICKS.includes(m.id) && !m.id.includes(":")).map(orChoice);
    return { main: picks, more, searchable: true };
  }
  return { main: CLAUDE.models, more: CLAUDE.moreModels ?? [], searchable: false };
}

export function effortsFor(provider: string, model: string): Choice[] {
  if (provider === "chatgpt") return [];
  if (provider === "codex") {
    const m = catalog.codex?.models.find((x) => x.id === model);
    const levels = m?.efforts.map(([e]) => e) ?? ["low", "medium", "high"];
    return levels.map(effortChoice);
  }
  if (provider === "openrouter") {
    const m = catalog.openrouter?.find((x) => x.id === model);
    // Sin la lista todavía no se sabe si razona: no se ofrece esfuerzo.
    return m?.reasoning ? ["low", "medium", "high"].map(effortChoice) : [];
  }
  return CLAUDE.efforts;
}

/** El modo solo importa con carpeta: sin ella ninguna IA toca archivos. */
export function modesFor(provider: string, hasFolder: boolean): Choice[] {
  if (!hasFolder) return [];
  if (provider === "codex")
    return [
      { id: "auto", label: "Editar", hint: "Cambia archivos dentro de la carpeta sin preguntar" },
      { id: "plan", label: "Solo leer", hint: "Lee y propone; no cambia nada" },
    ];
  if (provider === "openrouter" || provider === "chatgpt") return [];
  return CLAUDE.modes;
}

export function providerLabel(id: string) {
  return CHAT_PROVIDERS.find((p) => p.id === id)?.label ?? id;
}

/** Nombre corto del modelo para mostrar (cabecera, transcripción). */
export function modelLabel(provider: string, id: string | null | undefined): string {
  if (!id) return "";
  const { main, more } = modelsFor(provider);
  const hit = [...main, ...more].find((c) => c.id === id);
  if (hit) return hit.label;
  const claude = [...CLAUDE.models, ...(CLAUDE.moreModels ?? [])].find((c) => c.id === id);
  return claude?.label ?? id;
}

export function isReady(provider: string): boolean {
  return statusOf(provider)?.ready ?? provider === "claude";
}

// ---------- Elección por defecto ----------

const LAST_KEY = "comodel.ai.last";

function readLast(): Record<string, ChatAI> & { _provider?: string } {
  try {
    return JSON.parse(localStorage.getItem(LAST_KEY) ?? "{}");
  } catch {
    return {};
  }
}

/** Recuerda lo último elegido con cada IA, para que el siguiente chat empiece igual. */
export function remember(ai: ChatAI) {
  try {
    const last = readLast() as Record<string, unknown>;
    last[ai.provider] = ai;
    last._provider = ai.provider;
    localStorage.setItem(LAST_KEY, JSON.stringify(last));
  } catch {
    // sin almacenamiento: solo dura este chat
  }
}

function builtIn(provider: string): ChatAI {
  if (provider === "chatgpt") return { provider, model: catalog.chatgpt?.[0]?.id ?? "", effort: "", mode: "auto" };
  if (provider === "codex") {
    const c = catalog.codex;
    const model = c?.default_model && c.models.some((m) => m.id === c.default_model) ? c.default_model : (c?.models[0]?.id ?? "");
    const effort = c?.default_effort ?? c?.models.find((m) => m.id === model)?.default_effort ?? "medium";
    return { provider, model, effort, mode: "auto" };
  }
  if (provider === "openrouter") return { provider, model: OPENROUTER_PICKS[0], effort: "medium", mode: "auto" };
  return { provider: "claude", model: "claude-haiku-5-5", effort: "medium", mode: "auto" };
}

/** Lo que lleva un chat nuevo con `provider` (o con la última IA usada). */
export function defaultsFor(provider?: string): ChatAI {
  const last = readLast();
  const id = provider ?? (last._provider && CHAT_PROVIDERS.some((p) => p.id === last._provider) ? last._provider : "claude");
  const saved = last[id] as ChatAI | undefined;
  return saved?.provider === id ? { ...saved } : builtIn(id);
}

/** Corrige una elección que ya no vale (otro modelo, un esfuerzo que el modelo no admite, un modo sin carpeta). */
export function normalize(ai: ChatAI, hasFolder: boolean): ChatAI {
  const out = { ...ai };
  const { main, more } = modelsFor(out.provider);
  const known = [...main, ...more];
  const listLoaded = out.provider === "claude" || (out.provider === "codex" ? !!catalog.codex : out.provider === "chatgpt" ? !!catalog.chatgpt : !!catalog.openrouter);
  if (listLoaded && known.length && !known.some((c) => c.id === out.model)) out.model = builtIn(out.provider).model || known[0].id;
  const efforts = effortsFor(out.provider, out.model);
  if (efforts.length && !efforts.some((e) => e.id === out.effort)) {
    const def = out.provider === "codex" ? catalog.codex?.models.find((m) => m.id === out.model)?.default_effort : null;
    out.effort = def && efforts.some((e) => e.id === def) ? def : (efforts.find((e) => e.id === "medium") ?? efforts[0]).id;
  }
  const modes = modesFor(out.provider, hasFolder);
  if (modes.length && !modes.some((m) => m.id === out.mode)) out.mode = modes[0].id;
  return out;
}
