// Lo que la cabecera de cada chat ofrece por proveedor, en lenguaje llano.
// Más adelante cada adaptador declarará esto él mismo.

export interface Choice {
  id: string;
  label: string;
  hint?: string;
  /** Texto corto a la derecha (estado, atajo). */
  note?: string;
  /** Punto de identidad de una IA delante del nombre. */
  dot?: string;
}

export interface Capabilities {
  label: string;
  modes: Choice[];
  models: Choice[];
  /** Modelos anteriores, tras "Ver más modelos". */
  moreModels?: Choice[];
  efforts: Choice[];
}

export const CLAUDE: Capabilities = {
  label: "Claude",
  // Los ids son los valores de `--permission-mode` de Claude Code.
  modes: [
    { id: "auto", label: "Auto", hint: "Trabaja sola y te avisa al terminar" },
    { id: "plan", label: "Plan", hint: "Propone un plan; no cambia archivos" },
    { id: "default", label: "Preguntar", hint: "Te pide permiso antes de cambiar archivos o ejecutar comandos" },
  ],
  // Los ids son los nombres exactos de `--model`. Los principales van primero.
  models: [
    { id: "claude-opus-5-5", label: "Opus 5.5" },
    { id: "claude-sonnet-5-5", label: "Sonnet 5.5" },
    { id: "claude-haiku-5-5", label: "Haiku 5.5" },
    { id: "claude-fable-5-1", label: "Fable 5.1" },
  ],
  // Versiones anteriores, para quien todavía las usa. Pueden no estar disponibles en todas las cuentas.
  moreModels: [
    { id: "claude-fable-5", label: "Fable 5" },
    { id: "claude-opus-5", label: "Opus 5" },
    { id: "claude-sonnet-5", label: "Sonnet 5" },
    { id: "claude-opus-4-8", label: "Opus 4.8" },
    { id: "claude-opus-4-7", label: "Opus 4.7" },
    { id: "claude-opus-4-6", label: "Opus 4.6" },
    { id: "claude-sonnet-4-6", label: "Sonnet 4.6" },
    { id: "claude-opus-4-5", label: "Opus 4.5" },
    { id: "claude-sonnet-4-5", label: "Sonnet 4.5" },
    { id: "claude-haiku-4-5", label: "Haiku 4.5" },
  ],
  efforts: [
    { id: "low", label: "Bajo", hint: "Piensa poco, responde rápido" },
    { id: "medium", label: "Medio", hint: "Para la mayoría de tareas" },
    { id: "high", label: "Alto", hint: "Razona más antes de responder" },
    { id: "xhigh", label: "Extra", hint: "Para problemas difíciles" },
    { id: "max", label: "Máximo", hint: "Usa más cuota; solo si hace falta" },
  ],
};

const PROVIDER_NAMES: Record<string, string> = { claude: "Claude", openrouter: "OpenRouter", chatgpt: "ChatGPT", codex: "Codex", gemini: "Gemini" };

export function providerLabel(id: string): string {
  return PROVIDER_NAMES[id] ?? id;
}

const TOOL_NAMES: Record<string, string> = {
  Read: "Leyó un archivo",
  Grep: "Buscó en los archivos",
  Glob: "Buscó archivos",
  Bash: "Ejecutó un comando",
  WebSearch: "Buscó en la web",
  WebFetch: "Abrió una página web",
};

/** Las dos últimas partes de una ruta, para que la línea diga qué archivo sin ocupar todo el ancho. */
const shortPath = (p: string) => p.split("/").filter(Boolean).slice(-2).join("/");

export function toolLabel(name: string, input?: Record<string, unknown>): string {
  if (name === "WorkspaceFiles") {
    const files = Array.isArray(input?.files) ? input.files.length : 0;
    const skipped = Array.isArray(input?.skipped) ? input.skipped : [];
    const excluded = typeof input?.excluded === "number" ? input.excluded : 0;
    return `Preparó ${files} archivos del workspace para ChatGPT${excluded ? `; ${excluded} entradas internas, dependencias, credenciales o enlaces excluidos` : ""}${skipped.length ? `. No se adjuntaron ${skipped.length}: ${skipped.slice(0, 5).join("; ")}${skipped.length > 5 ? "; y otros" : ""}` : ""}.`;
  }
  const path = typeof input?.file_path === "string" ? input.file_path : typeof input?.path === "string" ? input.path : null;
  if (name === "Bash" && typeof input?.command === "string") {
    const first = input.command.split("\n")[0].slice(0, 80);
    return `Ejecutó: ${first}`;
  }
  if (name === "Read" && path) return `Leyó ${shortPath(path)}`;
  if ((name === "Grep" || name === "Glob") && typeof input?.pattern === "string") return `Buscó «${input.pattern.slice(0, 40)}»`;
  if (name === "WebFetch" && typeof input?.url === "string") return `Abrió ${input.url.slice(0, 60)}`;
  return TOOL_NAMES[name] ?? `Usó ${name}`;
}

export function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(n % 1_000_000 === 0 ? 0 : 1)} M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(n >= 100_000 ? 0 : 1)} k`;
  return String(n);
}

export function windowLabel(w: string): string {
  return ({ five_hour: "5 h", seven_day: "7 d", credits: "Créditos" } as Record<string, string>)[w] ?? w;
}

export function windowTitle(w: string): string {
  return (
    ({ five_hour: "Uso de las últimas 5 horas", seven_day: "Uso de los últimos 7 días", credits: "Créditos gastados" } as Record<string, string>)[w] ?? w
  );
}

export type RelayState = "ready" | "setup" | "soon";

export interface RelayEntry {
  id: string;
  label: string;
  via: string;
  state: RelayState;
}

/** Orden de relevo del proyecto. Más adelante lo define el usuario al crear el proyecto. */
export const RELAY: RelayEntry[] = [
  { id: "claude", label: "Claude", via: "Claude Code", state: "ready" },
  { id: "codex", label: "Codex", via: "Cuenta de ChatGPT", state: "setup" },
  { id: "chatgpt", label: "ChatGPT", via: "Plan de ChatGPT", state: "setup" },
  { id: "openrouter", label: "OpenRouter", via: "Clave de API", state: "setup" },
  { id: "gemini", label: "Gemini", via: "Clave de API", state: "soon" },
];

export const RELAY_STATE_LABEL: Record<RelayState, string> = { ready: "Lista", setup: "Sin configurar", soon: "Próximamente" };

/** Nombre y vía de conexión de cada IA que CoModel sabe usar. */
export const PROVIDER_INFO: Record<string, { label: string; via: string }> = {
  claude: { label: "Claude", via: "Claude Code" },
  openrouter: { label: "OpenRouter", via: "Clave de API" },
  codex: { label: "Codex", via: "Cuenta de ChatGPT" },
  chatgpt: { label: "ChatGPT", via: "Plan de ChatGPT" },
  gemini: { label: "Gemini", via: "Clave de API" },
};

/** Las IAs que se pueden elegir para un proyecto hoy, en el orden por defecto. */
export const SELECTABLE = ["claude", "codex", "openrouter"];
