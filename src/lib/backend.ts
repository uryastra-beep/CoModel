import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

// Espejo de los eventos del núcleo (core/src/events.rs y orchestrator.rs).
export type ProviderEvent =
  | { type: "Started"; data: { session_id: string | null; model: string | null; mode: string | null } }
  | { type: "Text"; data: string }
  | { type: "ToolUse"; data: { name: string; input: unknown } }
  | { type: "FileTouched"; data: { path: string } }
  | {
      type: "Usage";
      data: { input_tokens: number; output_tokens: number; context_window: number | null; cost_usd: number | null };
    }
  | { type: "Quota"; data: { window: string; used_fraction: number; resets_at: number | null } }
  | { type: "LimitReached"; data: { reason: string; resets_at: number | null } }
  | {
      type: "Permission";
      data: { id: string; tool: string; input: Record<string, unknown>; description: string | null; can_accept_edits: boolean };
    }
  | { type: "Question"; data: { id: string; questions: AskQuestion[] } }
  | { type: "Error"; data: string }
  | { type: "Finished" };

export interface AskQuestion {
  question: string;
  header: string;
  options: { label: string; description: string }[];
  multiSelect: boolean;
}

/** Lo que responde el usuario a un permiso o a una pregunta de la IA. */
export type Answer =
  | { kind: "allow"; always: boolean }
  | { kind: "deny"; message?: string }
  | { kind: "answers"; answers: Record<string, string> };

/** Un archivo que la IA entregó en su mensaje (core/src/outputs.rs). `path` es relativa a la carpeta de trabajo. */
export interface OutFile {
  path: string;
  name: string;
  mime: string;
  size: number;
  /** `image` si se puede mostrar; `file` para el resto. */
  kind: "image" | "file";
  /** `provider`: la IA lo envió directamente. `workspace`: lo escribió en la carpeta de trabajo. */
  origin: "provider" | "workspace";
}

export type UiEvent =
  | { kind: "Provider"; provider: string; event: ProviderEvent }
  | { kind: "Handoff"; from: string; to: string; reason: string }
  | { kind: "Outputs"; provider: string; files: OutFile[]; rejected: string[] };

export type ChatEvent = UiEvent & { chat_id: string };

export interface SendOptions {
  /** IA elegida en el chat: empieza ella y el resto del relevo queda detrás. */
  provider?: string | null;
  model: string | null;
  effort: string | null;
  mode: string | null;
  /** Nombre del chat, para el resumen de uso. */
  title: string | null;
  /** Rutas absolutas de los archivos adjuntos. */
  attachments: string[];
  /** El texto es el foco de un /compact: la IA resume la conversación en vez de contestarla. */
  compact?: boolean;
  sharedChats?: { chatId: string; projectId: string | null; title: string }[];
}

export interface ProviderStatus {
  id: string;
  /** Hay un adaptador en CoModel para este proveedor. */
  available: boolean;
  /** Listo para usarse: CLI instalado o clave guardada. */
  ready: boolean;
  detail: string;
}

export interface UsageSummary {
  days: number;
  totals: { turns: number; input_tokens: number; output_tokens: number; cost_usd: number };
  by_day: { day: string; turns: number; tokens: number }[];
  by_model: { model: string; provider: string; turns: number; tokens: number; cost_usd: number }[];
  by_chat: { chat: string; title: string; turns: number; tokens: number; cost_usd: number }[];
}

export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

type Listener = (ev: ChatEvent) => void;
const mockListeners = new Set<Listener>();
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

// En el navegador (sin Tauri) se simula un proveedor para poder ver y probar la interfaz.
const mockWaiting = new Map<string, (a: Answer) => void>();
const mockStop = new Set<string>();
const ask = (id: string) => new Promise<Answer>((resolve) => mockWaiting.set(id, resolve));

async function mockCompact(chatId: string): Promise<string> {
  const text = "Resumen simulado: se hablaba del diseño de la pantalla de inicio. Queda pendiente probar el panel de archivos.";
  mockListeners.forEach((l) => l({ chat_id: chatId, kind: "Provider", provider: "claude", event: { type: "Text", data: text } }));
  await sleep(600);
  return text;
}

async function mockSend(chatId: string, text: string, chosen = "claude", model: string | null = null): Promise<string> {
  const emit = (provider: string, event: ProviderEvent) =>
    mockListeners.forEach((l) => l({ chat_id: chatId, kind: "Provider", provider: provider === "claude" ? chosen : provider, event }));
  const reply = (p: string) =>
    `Respuesta simulada de ${p}.\n\nTu mensaje fue: **${text}**\n\n- Primer punto con \`código\`\n- Segundo punto\n\n\`\`\`ts\nconst listo = true;\n\`\`\`` +
    (/bloques/i.test(text) ? '\n\nListo, aquí tienes los archivos:\n\n```html file="web/index.html"\n<h1>Hola</h1>\n```\n\n```css file="web/estilo.css"\nh1 { color: rebeccapurple; }\n```' : "");

  emit("claude", { type: "Started", data: { session_id: "mock", model: chosen === "codex" ? null : (model ?? "claude-haiku-5-5"), mode: "default" } });
  await sleep(500);
  if (/permiso/i.test(text)) {
    const id = `perm-${Date.now()}`;
    emit("claude", {
      type: "Permission",
      data: { id, tool: "Edit", input: { file_path: "src/ejemplo.ts", old_string: "const a = 1;\nconst b = 2;", new_string: "const a = 1;\nconst b = 3;\nconst c = 4;" }, description: "src/ejemplo.ts", can_accept_edits: true },
    });
    const a = await ask(id);
    if (mockStop.delete(chatId)) throw "Detenido.";
    return a.kind === "allow" ? reply("Claude") : "Rechazado";
  }
  if (/pregunta/i.test(text)) {
    const id = `q-${Date.now()}`;
    emit("claude", {
      type: "Question",
      data: {
        id,
        questions: [
          { question: "¿Qué estilo prefieres?", header: "Estilo", multiSelect: false, options: [{ label: "Sobrio", description: "Pocos colores" }, { label: "Colorido", description: "Más contraste" }] },
          { question: "¿Qué secciones incluimos?", header: "Secciones", multiSelect: true, options: [{ label: "Inicio", description: "" }, { label: "Precios", description: "" }, { label: "Contacto", description: "" }] },
        ],
      },
    });
    await ask(id);
    return reply("Claude");
  }
  if (/archivo|adjunto|zip/i.test(text)) {
    // Una imagen de muestra: en el navegador la ruta es una URL de datos para poder verla sin la app.
    const canvas = document.createElement("canvas");
    canvas.width = 240; canvas.height = 140;
    const g = canvas.getContext("2d")!;
    const grad = g.createLinearGradient(0, 0, 240, 140);
    grad.addColorStop(0, "#7c5cff"); grad.addColorStop(1, "#ff8a4c");
    g.fillStyle = grad; g.fillRect(0, 0, 240, 140);
    const png = canvas.toDataURL("image/png");
    emit("claude", { type: "Text", data: "Te dejo la gráfica y el paquete con todo." });
    mockListeners.forEach((l) =>
      l({
        chat_id: chatId,
        kind: "Outputs",
        provider: chosen,
        files: [
          { path: png, name: "grafica.png", mime: "image/png", size: 4096, kind: "image", origin: "provider" },
          { path: "entrega.zip", name: "entrega.zip", mime: "application/zip", size: 2457600, kind: "file", origin: "workspace" },
        ],
        rejected: ["secreto.txt: está fuera de la carpeta de trabajo"],
      }),
    );
    emit("claude", { type: "Finished" });
    return reply("Claude");
  }
  if (/l[ií]mite/i.test(text)) {
    emit("claude", { type: "LimitReached", data: { reason: "límite simulado", resets_at: null } });
    mockListeners.forEach((l) =>
      l({ chat_id: chatId, kind: "Handoff", from: "claude", to: "openrouter", reason: "límite simulado" }),
    );
    await sleep(400);
    emit("openrouter", { type: "Started", data: { session_id: null, model: "openai/gpt-4o-mini", mode: null } });
    emit("openrouter", { type: "Text", data: reply("OpenRouter") });
    emit("openrouter", { type: "Usage", data: { input_tokens: 5200, output_tokens: 90, context_window: 128000, cost_usd: 0.0004 } });
    emit("openrouter", { type: "Quota", data: { window: "credits", used_fraction: 0.25, resets_at: null } });
    emit("openrouter", { type: "Finished" });
    return reply("OpenRouter");
  }
  emit("claude", { type: "ToolUse", data: { name: "Read", input: {} } });
  await sleep(400);
  emit("claude", { type: "FileTouched", data: { path: "src/lib/ejemplo.ts" } });
  emit("claude", { type: "Text", data: reply("Claude") });
  emit("claude", { type: "Usage", data: { input_tokens: 48000, output_tokens: 320, context_window: 1000000, cost_usd: 0.004 } });
  emit("claude", { type: "Quota", data: { window: "five_hour", used_fraction: 0.27, resets_at: Math.floor(Date.now() / 1000) + 7200 } });
  emit("claude", { type: "Quota", data: { window: "seven_day", used_fraction: 0.43, resets_at: null } });
  emit("claude", { type: "Finished" });
  return reply("Claude");
}

export async function sendMessage(projectId: string | null, chatId: string, text: string, options: SendOptions): Promise<string> {
  if (!inTauri) return options.compact ? mockCompact(chatId) : mockSend(chatId, text, options.provider ?? "claude", options.model);
  return invoke<string>("send_message", { projectId, chatId, text, options });
}

/** /clear: lo anterior deja de contar como contexto; el historial guardado se conserva. */
export async function clearChat(projectId: string | null, chatId: string): Promise<void> {
  if (inTauri) await invoke("clear_chat", { projectId, chatId });
}

export interface StoredMessage {
  role: "user" | "assistant";
  content: string;
  provider: string | null;
  attachments?: string[];
  /** Archivos que la IA entregó en este mensaje (solo asistentes). */
  outputs?: OutFile[];
}

/** La conversación guardada de un chat, para volver a mostrarla al reabrir. */
export async function chatHistory(projectId: string | null, chatId: string): Promise<StoredMessage[]> {
  if (!inTauri) return [];
  return invoke<StoredMessage[]>("chat_history", { projectId, chatId });
}

/** Abre un archivo entregado por la IA con la aplicación del sistema, o lo muestra en su carpeta. */
export async function openOutput(projectId: string | null, file: OutFile, reveal = false): Promise<void> {
  if (!inTauri) throw "Abrir archivos está disponible en la app de escritorio.";
  await invoke("open_output", { projectId, path: file.path, reveal });
}

/** Guarda una copia del archivo donde elija el usuario. Devuelve `false` si cancela. */
export async function downloadOutput(projectId: string | null, file: OutFile): Promise<boolean> {
  if (!inTauri) {
    if (!file.path.startsWith("data:")) throw "Descargar archivos está disponible en la app de escritorio.";
    const a = document.createElement("a");
    a.href = file.path;
    a.download = file.name;
    a.click();
    return true;
  }
  const { save } = await import("@tauri-apps/plugin-dialog");
  const dest = await save({ title: "Guardar archivo", defaultPath: file.name });
  if (!dest) return false;
  await invoke("save_output", { projectId, path: file.path, dest });
  return true;
}

export async function answerInteraction(requestId: string, answer: Answer): Promise<void> {
  if (!inTauri) {
    mockWaiting.get(requestId)?.(answer);
    mockWaiting.delete(requestId);
    return;
  }
  await invoke("answer_interaction", { requestId, answer });
}

/** Para el mensaje en curso de un chat. */
export async function stopMessage(chatId: string): Promise<void> {
  if (!inTauri) {
    mockStop.add(chatId);
    return;
  }
  await invoke("stop_message", { chatId });
}

export async function closeChat(chatId: string): Promise<void> {
  if (inTauri) await invoke("close_chat", { chatId });
}

export async function onChatEvent(cb: Listener): Promise<() => void> {
  if (!inTauri) {
    mockListeners.add(cb);
    return () => mockListeners.delete(cb);
  }
  return listen<ChatEvent>("comodel://event", (e) => cb(e.payload));
}

// ---------- Memoria importada de otras IAs ----------

export interface MemoryImport {
  id: string;
  label: string;
  chars: number;
  /** Segundos desde 1970. */
  updated: number;
}

export interface MemoryState {
  imports: MemoryImport[];
  /** Tamaño del texto compartido que leen las IAs, si existe. */
  shared_chars: number | null;
  /** Está instalado el programa que junta la memoria (`ai-memory-sync`). */
  sync_available: boolean;
}

export interface SyncReport {
  available: boolean;
  ok: boolean;
  message: string;
}

// En el navegador la memoria importada se guarda en localStorage para poder probar el flujo.
const MOCK_MEMORY = "comodel.mock.memory";
const mockMemory = (): Record<string, { text: string; updated: number }> => {
  try {
    return JSON.parse(localStorage.getItem(MOCK_MEMORY) ?? "{}");
  } catch {
    return {};
  }
};
const mockLabel = (id: string) => ({ chatgpt: "ChatGPT", gemini: "Gemini" } as Record<string, string>)[id] ?? id.charAt(0).toUpperCase() + id.slice(1);

export async function memoryState(): Promise<MemoryState> {
  if (!inTauri) {
    const m = mockMemory();
    return { imports: Object.entries(m).map(([id, v]) => ({ id, label: mockLabel(id), chars: v.text.length, updated: v.updated })), shared_chars: 16000, sync_available: true };
  }
  return invoke<MemoryState>("memory_imports");
}

export async function memoryImportText(id: string): Promise<string> {
  if (!inTauri) return mockMemory()[id]?.text ?? "";
  return invoke<string>("memory_import_get", { id });
}

export async function importMemory(label: string, text: string): Promise<{ info: MemoryImport; sync: SyncReport }> {
  if (!inTauri) {
    const id = label.trim().toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
    if (!id) throw new Error("Ponle un nombre a la fuente (por ejemplo ChatGPT).");
    if (!text.trim()) throw new Error("Pega primero lo que te dio la IA.");
    const m = mockMemory();
    m[id] = { text: text.trim(), updated: Math.floor(Date.now() / 1000) };
    localStorage.setItem(MOCK_MEMORY, JSON.stringify(m));
    return { info: { id, label: mockLabel(id), chars: text.trim().length, updated: m[id].updated }, sync: { available: true, ok: true, message: "shared.md: 16000 bytes (simulado)" } };
  }
  return invoke("memory_import_save", { label, text });
}

export async function removeMemoryImport(id: string): Promise<SyncReport> {
  if (!inTauri) {
    const m = mockMemory();
    delete m[id];
    localStorage.setItem(MOCK_MEMORY, JSON.stringify(m));
    return { available: true, ok: true, message: "" };
  }
  return invoke<SyncReport>("memory_import_delete", { id });
}

export async function syncMemory(): Promise<SyncReport> {
  if (!inTauri) return { available: true, ok: true, message: "shared.md: 16000 bytes (simulado)" };
  return invoke<SyncReport>("memory_sync");
}

// ---------- Títulos automáticos ----------

/** Un título corto para el primer mensaje de un chat. Falla si no hay ninguna IA que lo genere. */
export async function suggestTitle(text: string, provider: string | null): Promise<string> {
  if (!inTauri) {
    await sleep(600);
    return quickTitle(text).split(" ").slice(0, 3).join(" ") + " (IA)";
  }
  return invoke<string>("suggest_title", { text, provider });
}

/** Título provisional y gratis: las primeras palabras del mensaje, sin formato. */
export function quickTitle(text: string): string {
  const clean = text
    .replace(/```[\s\S]*?```/g, " ")
    .replace(/https?:\/\/\S+/g, " ")
    .replace(/[`*_#>~\[\]()]/g, " ")
    .split("\n")
    .map((l) => l.trim())
    .find((l) => l.length > 0) ?? "";
  const words = clean.split(/\s+/).filter(Boolean).slice(0, 6);
  let t = words.join(" ");
  if (t.length > 40) t = t.slice(0, 40).replace(/\s+\S*$/, "");
  t = t.replace(/[.,:;!?¡¿\s]+$/, "").replace(/^[¡¿\s]+/, "");
  return t ? t.charAt(0).toUpperCase() + t.slice(1) : "";
}

// ---------- Archivos que entrega una IA ----------

export interface GeneratedFile {
  path: string;
  content: string;
}

/** Guarda un zip con los archivos donde el usuario elija. Devuelve la ruta, o `null` si cancela. */
export async function saveGeneratedZip(files: GeneratedFile[], name = "archivos.zip"): Promise<string | null> {
  if (!inTauri) throw new Error("El zip solo se puede crear en la app de escritorio.");
  const { save } = await import("@tauri-apps/plugin-dialog");
  const path = await save({ title: "Guardar el zip", defaultPath: name, filters: [{ name: "Zip", extensions: ["zip"] }] });
  if (!path) return null;
  await invoke("write_generated_zip", { path, files });
  return path;
}

export interface SaveReport {
  written: string[];
  skipped: string[];
}

/** Guarda los archivos en la carpeta del proyecto sin sobrescribir los que ya existen. */
export async function saveGeneratedFiles(projectId: string | null, files: GeneratedFile[]): Promise<SaveReport> {
  if (!inTauri) throw new Error("Guardar en la carpeta solo funciona en la app de escritorio.");
  return invoke<SaveReport>("save_generated_files", { projectId, files });
}

// ---------- Terminal ----------
// Solo existe en la app de escritorio: un navegador no puede abrir un shell.

export const terminalAvailable = inTauri;

export async function termOpen(id: string, projectId: string | null, cols: number, rows: number): Promise<void> {
  await invoke("term_open", { id, projectId, cols, rows });
}
export async function termWrite(id: string, data: string): Promise<void> {
  await invoke("term_write", { id, data });
}
export async function termResize(id: string, cols: number, rows: number): Promise<void> {
  await invoke("term_resize", { id, cols, rows });
}
export async function termClose(id: string): Promise<void> {
  if (inTauri) await invoke("term_close", { id });
}

export interface TermEvent {
  id: string;
  /** Salida del shell en base64. */
  data: string | null;
  exit: boolean;
}

export async function onTermEvent(cb: (e: TermEvent) => void): Promise<() => void> {
  if (!inTauri) return () => {};
  return listen<TermEvent>("comodel://term", (e) => cb(e.payload));
}

// ---------- Conexiones y uso ----------
// En el navegador (sin Tauri) estas funciones devuelven datos de ejemplo para poder ver y probar la interfaz.

let mockKey = false;

export async function providerStatus(): Promise<ProviderStatus[]> {
  if (!inTauri) {
    return [
      { id: "claude", available: true, ready: true, detail: "2.1.310 (Claude Code)" },
      { id: "openrouter", available: true, ready: mockKey, detail: mockKey ? "Clave guardada" : "Sin clave" },
      { id: "codex", available: true, ready: true, detail: "Logged in using ChatGPT" },
      { id: "gemini", available: false, ready: false, detail: "Próximamente" },
    ];
  }
  return invoke<ProviderStatus[]>("provider_status");
}

/** La clave viaja una sola vez al llavero del sistema; la interfaz nunca la vuelve a recibir. */
export async function setApiKey(provider: string, key: string): Promise<void> {
  if (!inTauri) {
    if (!key.trim()) throw "La clave no parece válida. Pégala completa, sin espacios.";
    mockKey = true;
    return;
  }
  await invoke("set_api_key", { provider, key });
}

export async function removeApiKey(provider: string): Promise<void> {
  if (!inTauri) {
    mockKey = false;
    return;
  }
  await invoke("remove_api_key", { provider });
}

export async function testConnection(provider: string): Promise<string> {
  if (!inTauri) return provider === "openrouter" ? "Conexión correcta. Gastado 0.42 de 10.00 dólares." : provider === "codex" ? "Codex listo. Logged in using ChatGPT." : "2.1.310 (Claude Code) instalado. Usa tu sesión de Claude Code.";
  return invoke<string>("test_connection", { provider });
}

export async function usageSummary(days = 7): Promise<UsageSummary> {
  if (!inTauri) return mockUsage(days);
  return invoke<UsageSummary>("usage_summary", { days });
}

function mockUsage(days: number): UsageSummary {
  const tokens = [18_400, 52_000, 0, 131_000, 88_500, 240_000, 61_300];
  const by_day = Array.from({ length: days }, (_, i) => {
    const d = new Date();
    d.setDate(d.getDate() - (days - 1 - i));
    const t = tokens[i % tokens.length];
    return { day: `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`, turns: t ? Math.ceil(t / 21_000) : 0, tokens: t };
  });
  return {
    days,
    totals: { turns: 33, input_tokens: 570_000, output_tokens: 21_200, cost_usd: 3.84 },
    by_day,
    by_model: [
      { model: "claude-sonnet-5-5", provider: "claude", turns: 14, tokens: 301_000, cost_usd: 1.9 },
      { model: "claude-opus-5-5", provider: "claude", turns: 6, tokens: 212_000, cost_usd: 1.7 },
      { model: "claude-haiku-5-5", provider: "claude", turns: 11, tokens: 61_000, cost_usd: 0.2 },
      { model: "openai/gpt-4o-mini", provider: "openrouter", turns: 2, tokens: 17_200, cost_usd: 0.04 },
    ],
    by_chat: [
      { chat: "c1", title: "Refactor del núcleo", turns: 13, tokens: 340_000, cost_usd: 2.1 },
      { chat: "c2", title: "Pruebas de la interfaz", turns: 9, tokens: 160_000, cost_usd: 1.2 },
      { chat: "c3", title: "Chat 3", turns: 11, tokens: 91_200, cost_usd: 0.54 },
    ],
  };
}

const baseName = (p: string) => p.replace(/[\\/]+$/, "").split(/[\\/]/).pop() || p;

// ---------- Proyectos ----------

export interface Project {
  id: string;
  name: string;
  path: string;
  /** Ids de las IAs, en orden de relevo. */
  relay: string[];
}

export interface ProjectsState {
  projects: Project[];
  /** Orden de relevo de los chats sin carpeta. */
  default_relay: string[];
}

// En el navegador los proyectos se guardan en localStorage para poder probar los flujos.
const MOCK_KEY = "comodel.mock.projects";

function mockLoad(): ProjectsState {
  try {
    const s = JSON.parse(localStorage.getItem(MOCK_KEY) ?? "") as ProjectsState;
    return { projects: s.projects ?? [], default_relay: s.default_relay ?? ["claude"] };
  } catch {
    return { projects: [], default_relay: ["claude"] };
  }
}

function mockSave(s: ProjectsState) {
  try {
    localStorage.setItem(MOCK_KEY, JSON.stringify(s));
  } catch {
    // sin almacenamiento
  }
}

export async function listProjects(): Promise<ProjectsState> {
  if (!inTauri) return mockLoad();
  return invoke<ProjectsState>("list_projects");
}

export async function createProject(name: string, path: string, relay: string[]): Promise<Project> {
  if (!inTauri) {
    const s = mockLoad();
    if (!name.trim()) throw "Ponle un nombre al proyecto.";
    if (!path.trim()) throw "Esa carpeta no existe o no se puede abrir.";
    if (!relay.length) throw "Elige al menos una IA para el proyecto.";
    if (s.projects.some((p) => p.path === path)) throw "Esa carpeta ya es un proyecto.";
    const p = { id: `p${Date.now().toString(36)}`, name: name.trim(), path, relay };
    s.projects.push(p);
    mockSave(s);
    return p;
  }
  return invoke<Project>("create_project", { name, path, relay });
}

/** Abre una carpeta suelta como proyecto (o devuelve el que ya existe para esa carpeta). */
export async function openFolder(path: string): Promise<Project> {
  if (!inTauri) {
    const s = mockLoad();
    const found = s.projects.find((p) => p.path === path);
    if (found) return found;
    return createProject(baseName(path), path, s.default_relay);
  }
  return invoke<Project>("open_folder", { path });
}

export async function updateProject(id: string, name: string | null, relay: string[] | null): Promise<Project> {
  if (!inTauri) {
    const s = mockLoad();
    const p = s.projects.find((p) => p.id === id);
    if (!p) throw "No se encontró el proyecto.";
    if (name !== null) p.name = name.trim() || p.name;
    if (relay !== null) p.relay = relay;
    mockSave(s);
    return p;
  }
  return invoke<Project>("update_project", { id, name, relay });
}

export async function removeProject(id: string): Promise<void> {
  if (!inTauri) {
    const s = mockLoad();
    s.projects = s.projects.filter((p) => p.id !== id);
    mockSave(s);
    return;
  }
  await invoke("remove_project", { id });
}

export async function setDefaultRelay(relay: string[]): Promise<void> {
  if (!inTauri) {
    const s = mockLoad();
    s.default_relay = relay;
    mockSave(s);
    return;
  }
  await invoke("set_default_relay", { relay });
}

/** Selector de carpeta nativo del sistema. `null` si se cancela (o en el navegador, donde se escribe la ruta). */
export async function pickFolder(): Promise<string | null> {
  if (!inTauri) return null;
  const { open } = await import("@tauri-apps/plugin-dialog");
  const r = await open({ directory: true, multiple: false, title: "Elige la carpeta del proyecto" });
  return typeof r === "string" ? r : null;
}

// ---------- Adjuntos ----------

export interface PickedFile {
  name: string;
  path: string;
}

/** Selector de archivos nativo (varios a la vez). En el navegador se usa el selector del propio navegador. */
export async function pickFiles(): Promise<PickedFile[]> {
  if (!inTauri) {
    return new Promise((resolve) => {
      const input = document.createElement("input");
      input.type = "file";
      input.multiple = true;
      input.onchange = () => resolve([...(input.files ?? [])].map((f) => ({ name: f.name, path: `/ejemplo/${f.name}` })));
      input.oncancel = () => resolve([]);
      input.click();
    });
  }
  const { open } = await import("@tauri-apps/plugin-dialog");
  const r = await open({ directory: false, multiple: true, title: "Adjuntar archivos" });
  const list = Array.isArray(r) ? r : r ? [r] : [];
  return list.map((path) => ({ name: baseName(path), path }));
}

/** Guarda una imagen pegada dentro del proyecto y devuelve su ruta. */
export async function savePastedImage(projectId: string | null, name: string, base64: string): Promise<string> {
  if (!inTauri) return `/ejemplo/${name}`;
  return invoke<string>("save_pasted_image", { projectId, name, data: base64 });
}

export interface DropEvent {
  paths: string[];
  /** Posición del puntero en píxeles de la ventana (ya convertida a coordenadas CSS). */
  x: number;
  y: number;
  /** `over` mientras se arrastra encima, `drop` al soltar, `leave` si se cancela. */
  phase: "over" | "drop" | "leave";
}

/** Archivos arrastrados desde el sistema a la ventana. Solo existe dentro de Tauri. */
export async function onFileDrop(cb: (e: DropEvent) => void): Promise<() => void> {
  if (!inTauri) return () => {};
  const { getCurrentWebview } = await import("@tauri-apps/api/webview");
  const dpr = window.devicePixelRatio || 1;
  return getCurrentWebview().onDragDropEvent((ev) => {
    const p = ev.payload;
    if (p.type === "leave") return cb({ paths: [], x: 0, y: 0, phase: "leave" });
    const pos = p.position;
    cb({ paths: p.type === "drop" || p.type === "enter" ? p.paths : [], x: pos.x / dpr, y: pos.y / dpr, phase: p.type === "drop" ? "drop" : "over" });
  });
}

// ---------- Archivos del proyecto ----------

export interface DirEntry {
  name: string;
  is_dir: boolean;
  size: number;
}

export type FileContent =
  | { kind: "text"; content: string; size: number; mtime: number }
  | { kind: "image"; data: string; mime: string; size: number; mtime: number }
  | { kind: "unsupported"; reason: string; size: number; mtime: number };

export interface GitChange {
  path: string;
  /** M modificado, A nuevo, ? sin seguimiento, D borrado, R renombrado. */
  status: string;
}

// Archivos de ejemplo para probar la interfaz en el navegador.
const mockFs: Record<string, { content: string; mtime: number }> = {
  "README.md": { content: "# Mi app\n\nUna app de ejemplo para probar CoModel.\n", mtime: 1 },
  "package.json": { content: '{\n  "name": "mi-app",\n  "version": "1.0.0"\n}\n', mtime: 1 },
  "src/main.rs": { content: 'fn main() {\n    println!("hola");\n}\n', mtime: 1 },
  "src/lib/app.ts": { content: 'export const saludo = (n: string) => `Hola, ${n}`;\n', mtime: 1 },
  "docs/guia.md": { content: "# Guía\n\nPasos para empezar.\n", mtime: 1 },
};

export async function listDir(projectId: string, path: string): Promise<DirEntry[]> {
  if (!inTauri) {
    const prefix = path ? `${path}/` : "";
    const seen = new Map<string, DirEntry>();
    for (const [p, f] of Object.entries(mockFs)) {
      if (!p.startsWith(prefix)) continue;
      const rest = p.slice(prefix.length);
      const [first, ...more] = rest.split("/");
      if (!seen.has(first)) seen.set(first, { name: first, is_dir: more.length > 0, size: more.length ? 0 : f.content.length });
    }
    return [...seen.values()].sort((a, b) => Number(b.is_dir) - Number(a.is_dir) || a.name.localeCompare(b.name));
  }
  return invoke<DirEntry[]>("list_dir", { projectId, path });
}

export async function readFile(projectId: string, path: string): Promise<FileContent> {
  if (!inTauri) {
    const f = mockFs[path];
    if (!f) throw "No se encontró el archivo.";
    return { kind: "text", content: f.content, size: f.content.length, mtime: f.mtime };
  }
  return invoke<FileContent>("read_file", { projectId, path });
}

export async function fileMtime(projectId: string, path: string): Promise<number> {
  if (!inTauri) {
    const f = mockFs[path];
    if (!f) throw "No se encontró el archivo.";
    return f.mtime;
  }
  return invoke<number>("file_mtime", { projectId, path });
}

/** Guarda el archivo. Lanza "CONFLICT" si cambió en disco desde que se abrió (salvo que `expectedMtime` sea null). */
export async function writeFile(projectId: string, path: string, content: string, expectedMtime: number | null): Promise<number> {
  if (!inTauri) {
    const f = mockFs[path];
    if (f && expectedMtime !== null && f.mtime !== expectedMtime) throw "CONFLICT";
    const mtime = Math.max(Date.now(), (f?.mtime ?? 0) + 1);
    mockFs[path] = { content, mtime };
    return mtime;
  }
  return invoke<number>("write_file", { projectId, path, content, expectedMtime });
}

/** Solo para probar el aviso de "cambió en disco" en el navegador. */
export function mockExternalEdit(path: string, content: string) {
  mockFs[path] = { content, mtime: Date.now() + 5 };
}

// Gancho de pruebas del navegador: no existe dentro de la app real.
if (!inTauri && typeof window !== "undefined") (window as unknown as { __mockExternalEdit: typeof mockExternalEdit }).__mockExternalEdit = mockExternalEdit;

export async function gitStatus(projectId: string): Promise<GitChange[]> {
  if (!inTauri) return [{ path: "src/main.rs", status: "M" }, { path: "docs/guia.md", status: "?" }];
  return invoke<GitChange[]>("git_status", { projectId });
}

export async function gitDiff(projectId: string, path: string): Promise<string | null> {
  if (!inTauri) {
    return path === "src/main.rs" ? '@@ -1,3 +1,4 @@ fn main() {\n fn main() {\n-    println!("adiós");\n+    println!("hola");\n+    println!("de nuevo");\n }\n' : "";
  }
  return invoke<string | null>("git_diff", { projectId, path });
}

// ---------- Cuenta ----------

export interface Account {
  email: string;
  name: string | null;
  avatar: string | null;
  provider: "email" | "google" | "github";
}

export type SignUpResult = { kind: "signed_in"; account: Account } | { kind: "confirm_email" };

let mockAccount: Account | null = null;
const mockUser = (email: string, provider: Account["provider"]): Account => ({ email, name: provider === "email" ? null : "Ury", avatar: null, provider });

/** Si el inicio de sesión está configurado en esta compilación, y la cuenta con sesión si la hay. */
export async function accountStatus(): Promise<{ configured: boolean; account: Account | null }> {
  if (!inTauri) return { configured: true, account: mockAccount };
  const [configured, account] = await invoke<[boolean, Account | null]>("account_status");
  return { configured, account };
}

export async function accountSignIn(email: string, password: string): Promise<Account> {
  if (!inTauri) {
    if (password !== "12345678") throw "Correo o contraseña incorrectos.";
    return (mockAccount = mockUser(email, "email"));
  }
  return invoke<Account>("account_sign_in", { email, password });
}

export async function accountSignUp(email: string, password: string): Promise<SignUpResult> {
  if (!inTauri) {
    if (password.length < 8) throw "La contraseña es demasiado corta o débil. Usa al menos 8 caracteres.";
    return { kind: "confirm_email" };
  }
  return invoke<SignUpResult>("account_sign_up", { email, password });
}

/** Abre el navegador del sistema y espera a que el usuario termine allí. */
export async function accountSignInOAuth(provider: "google" | "github"): Promise<Account> {
  if (!inTauri) {
    await sleep(900);
    return (mockAccount = mockUser("ury@example.com", provider));
  }
  return invoke<Account>("account_sign_in_oauth", { provider });
}

export async function accountSignOut(): Promise<void> {
  mockAccount = null;
  if (inTauri) await invoke("account_sign_out");
}

// ---------- Importar chats ----------

/** Descarga un chat compartido (ChatGPT o Claude) y lo guarda como historial de `chatId`. */
export async function importChat(projectId: string | null, chatId: string, url: string): Promise<{ title: string; count: number }> {
  if (!inTauri) {
    await sleep(700);
    if (!/^https:\/\/(chatgpt\.com|chat\.openai\.com|claude\.ai)\/(share|s)\//.test(url.trim())) throw "Ese enlace no parece un chat compartido de ChatGPT (chatgpt.com/share/... o chatgpt.com/s/...) ni de Claude (claude.ai/share/...).";
    return { title: "Chat importado", count: 6 };
  }
  return invoke("import_chat", { projectId, chatId, url });
}

/** Importa una sesión de Claude Code desde su archivo .jsonl. */
export async function importChatFile(projectId: string | null, chatId: string, path: string): Promise<{ title: string; count: number }> {
  if (!inTauri) {
    await sleep(700);
    return { title: "Sesión importada", count: 12 };
  }
  return invoke("import_chat_file", { projectId, chatId, path });
}

/** Elige el archivo de una sesión exportada. Devuelve null si se cancela. */
export async function pickSessionFile(): Promise<string | null> {
  if (!inTauri) return "/ejemplo/sesion.jsonl";
  const { open } = await import("@tauri-apps/plugin-dialog");
  const r = await open({ directory: false, multiple: false, title: "Elegir sesión de Claude Code", filters: [{ name: "Sesión de Claude Code", extensions: ["jsonl"] }] });
  return typeof r === "string" ? r : null;
}

// ---------- Modelos de cada IA ----------

export interface CodexModel {
  id: string;
  label: string;
  description: string;
  /** (nivel, explicación en inglés) tal como los anuncia Codex. */
  efforts: [string, string][];
  default_effort: string | null;
}

export interface CodexCatalog {
  models: CodexModel[];
  default_model: string | null;
  default_effort: string | null;
}

/** Modelos de la cuenta de Codex del usuario (los mantiene el propio Codex). */
export async function codexModels(): Promise<CodexCatalog> {
  if (!inTauri) {
    const e = (ids: string[]): [string, string][] => ids.map((x) => [x, ""]);
    return {
      models: [
        { id: "gpt-6.1-sol", label: "GPT-6.1-Sol", description: "", efforts: e(["low", "medium", "high", "xhigh", "max", "ultra"]), default_effort: "low" },
        { id: "gpt-6-astra", label: "GPT-6-Astra", description: "", efforts: e(["low", "medium", "high", "xhigh", "max", "ultra"]), default_effort: "low" },
        { id: "gpt-6-luna", label: "GPT-6-Luna", description: "", efforts: e(["low", "medium", "high", "xhigh", "max"]), default_effort: "medium" },
      ],
      default_model: "gpt-6-luna",
      default_effort: "high",
    };
  }
  return invoke<CodexCatalog>("codex_models");
}

export interface OpenRouterModel {
  id: string;
  name: string;
  /** Admite elegir el esfuerzo de razonamiento. */
  reasoning: boolean;
  context: number | null;
}

/** Modelos que ofrece OpenRouter ahora mismo (lista pública). */
export async function openrouterModels(): Promise<OpenRouterModel[]> {
  if (!inTauri) {
    await sleep(300);
    return [
      { id: "openai/gpt-6-luna", name: "OpenAI: GPT-6 Luna", reasoning: true, context: 400000 },
      { id: "openai/gpt-6.1-sol", name: "OpenAI: GPT-6.1 Sol", reasoning: true, context: 400000 },
      { id: "anthropic/claude-sonnet-5.5", name: "Anthropic: Claude Sonnet 5.5", reasoning: true, context: 1000000 },
      { id: "google/gemini-3.8-flash", name: "Google: Gemini 3.8 Flash", reasoning: true, context: 1000000 },
      { id: "deepseek/deepseek-v4.1-flash", name: "DeepSeek: DeepSeek V4.1 Flash", reasoning: true, context: 128000 },
      { id: "meta-llama/llama-3.3-70b-instruct", name: "Meta: Llama 3.3 70B Instruct", reasoning: false, context: 128000 },
      { id: "openai/gpt-4o-mini", name: "OpenAI: GPT-4o-mini", reasoning: false, context: 128000 },
    ];
  }
  return invoke<OpenRouterModel[]>("openrouter_models");
}

// Guardar en una ubicación elegida por el usuario, con confirmación nativa de sobrescritura.
export async function saveTextFile(name: string, content: string): Promise<string | null> {
  if (!inTauri) {
    const url = URL.createObjectURL(new Blob([content], { type: "text/plain;charset=utf-8" }));
    const a = document.createElement("a"); a.href = url; a.download = name; a.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
    return name;
  }
  const { save } = await import("@tauri-apps/plugin-dialog");
  const path = await save({ title: "Guardar archivo", defaultPath: name });
  if (!path) return null;
  await invoke("write_document", { path, content });
  return path;
}

export async function imageSource(projectId: string | null, path: string): Promise<string> {
  if (/^https?:\/\//i.test(path) || /^data:image\/(?:png|jpeg|gif|webp);base64,/i.test(path) || path.startsWith("blob:")) return path;
  if (!inTauri) throw "La vista previa de archivos locales está disponible en la app de escritorio.";
  try { path = decodeURIComponent(path); } catch { /* La ruta puede contener un porcentaje literal. */ }
  return invoke<string>("read_image", { projectId, path });
}

/** Cambia la carpeta sin perder el historial; el original queda conservado en su base de datos. */
export async function moveChatWorkspace(chatId: string, fromProjectId: string | null, toProjectId: string | null): Promise<void> {
  if (inTauri) await invoke("move_chat_workspace", { chatId, fromProjectId, toProjectId });
}

/** Workspace propio por defecto dentro de CoModel/Chats. Es estable para el id del chat. */
export async function createChatWorkspace(chatId: string, title: string): Promise<Project> {
  if (inTauri) return invoke<Project>("create_chat_workspace", { chatId, title });
  return openFolder(`/CoModel/Chats/Workspace #${Date.now()} - ${title.replace(/[\\/]/g, "_")}`);
}

export interface ChatGPTAccount { id: string; label: string; active: boolean; connected: boolean }
export interface ChatGPTModel { id: string; name: string }
export async function chatgptAccounts(): Promise<ChatGPTAccount[]> { return inTauri ? invoke("chatgpt_accounts") : []; }
export async function chatgptSignIn(accountId: string | null): Promise<string> {
  if (!inTauri) throw "Conecta ChatGPT desde la app de escritorio.";
  return invoke("chatgpt_sign_in", { accountId });
}
export async function chatgptCancelSignIn(): Promise<void> { if (inTauri) await invoke("chatgpt_cancel_sign_in"); }
export async function chatgptSelectAccount(id: string): Promise<void> { if (inTauri) await invoke("chatgpt_select_account", { id }); }
export async function chatgptSignOut(): Promise<string> { return inTauri ? invoke("chatgpt_sign_out") : ""; }
export async function chatgptModels(): Promise<ChatGPTModel[]> { if (!inTauri) return []; return invoke("chatgpt_models"); }
