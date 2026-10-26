// Estado del lienzo: tarjetas y vista (desplazamiento y zoom). Es de la ventana entera, no de un componente.
import type { ChatAI } from "./ai.svelte";
import { closeChat, createChatWorkspace, moveChatWorkspace } from "./backend";
import { touchProject, projects } from "./projects.svelte";
import { reducedMotion } from "./theme";

export interface Card {
  id: string;
  title: string;
  x: number;
  y: number;
  w: number;
  h: number;
  z: number;
  busy: boolean;
  /** Carpeta del chat (un proyecto), o `null` si no tiene. En un archivo, su proyecto. */
  projectId: string | null;
  /** Un chat o un archivo abierto en el editor. */
  kind: "chat" | "file" | "files" | "code";
  workspacePending?: boolean;
  content?: string;
  language?: string;
  /** Ruta relativa del archivo (solo si `kind` es `file`). */
  path: string | null;
  /** IA, modelo, esfuerzo y modo elegidos en el chat. Sin ellos, el chat usa los últimos elegidos. */
  ai?: ChatAI;
}

export const MIN_ZOOM = 0.25;
export const MAX_ZOOM = 2;

export const canvas = $state({
  cards: [] as Card[],
  links: [] as { a: string; b: string }[],
  projectSync: false,
  projectError: "",
  view: { x: 0, y: 0, k: 1 },
  size: { w: 900, h: 600 },
  focusId: null as string | null,
  z: 1,
  n: 0,
});

const remPx = () => parseFloat(getComputedStyle(document.documentElement).fontSize) || 16;
const clamp = (v: number, a: number, b: number) => Math.min(b, Math.max(a, v));

/** Abre un chat. `at` es el centro deseado en coordenadas del lienzo; sin él, el centro de la vista. */
export function addCard(at?: { x: number; y: number }, projectId: string | null = null, init?: { id?: string; title?: string; ai?: ChatAI }): Card {
  canvas.n += 1;
  const w = Math.round(30 * remPx());
  // Que quepa entera en lo que se ve del lienzo.
  const h = Math.round(Math.min(40 * remPx(), Math.max(20 * remPx(), (canvas.size.h - 64) / canvas.view.k)));
  const cascade = (canvas.cards.length % 6) * 28;
  const cx = at?.x ?? (canvas.size.w / 2 - canvas.view.x) / canvas.view.k + cascade;
  const cy = at?.y ?? (canvas.size.h / 2 - canvas.view.y) / canvas.view.k + cascade;
  const card: Card = {
    id: init?.id ?? `c${Date.now().toString(36)}${canvas.n}`,
    title: init?.title ?? `Chat ${canvas.n}`,
    x: Math.round(cx - w / 2),
    y: Math.round(cy - h / 2),
    w,
    h,
    z: ++canvas.z,
    busy: false,
    projectId,
    kind: "chat",
    path: null,
    ai: init?.ai,
  };
  canvas.cards.push(card);
  canvas.focusId = card.id;
  return card;
}

export function setCardAI(id: string, ai: ChatAI) {
  const c = canvas.cards.find((c) => c.id === id);
  if (c) c.ai = ai;
}

/** Abre un archivo en una tarjeta de código. Si ya está abierto, se lleva la vista hasta él. */
export function openFileCard(projectId: string, path: string) {
  const existing = canvas.cards.find((c) => c.kind === "file" && c.projectId === projectId && c.path === path);
  if (existing) {
    centerOn(existing.id);
    return existing;
  }
  canvas.n += 1;
  const w = Math.round(38 * remPx());
  const h = Math.round(Math.min(32 * remPx(), Math.max(18 * remPx(), (canvas.size.h - 64) / canvas.view.k)));
  const cascade = (canvas.cards.length % 6) * 28;
  const cx = (canvas.size.w / 2 - canvas.view.x) / canvas.view.k + cascade;
  const cy = (canvas.size.h / 2 - canvas.view.y) / canvas.view.k + cascade;
  const card: Card = {
    id: `f${Date.now().toString(36)}${canvas.n}`,
    title: path.split("/").pop() || path,
    x: Math.round(cx - w / 2),
    y: Math.round(cy - h / 2),
    w,
    h,
    z: ++canvas.z,
    busy: false,
    projectId,
    kind: "file",
    path,
  };
  canvas.cards.push(card);
  canvas.focusId = card.id;
  return card;
}

export function removeCard(id: string) {
  const i = canvas.cards.findIndex((c) => c.id === id);
  if (i < 0) return;
  const kind = canvas.cards[i].kind;
  canvas.cards.splice(i, 1);
  canvas.links = canvas.links.filter((l) => l.a !== id && l.b !== id);
  if (canvas.focusId === id) canvas.focusId = canvas.cards[canvas.cards.length - 1]?.id ?? null;
  if (kind === "chat") void closeChat(id).catch(() => {});
}

export function focusCard(id: string) {
  const c = canvas.cards.find((c) => c.id === id);
  if (!c) return;
  c.z = ++canvas.z;
  canvas.focusId = id;
}

export function moveCard(id: string, dx: number, dy: number) {
  const c = canvas.cards.find((c) => c.id === id);
  if (c) {
    c.x += dx;
    c.y += dy;
  }
}

export function resizeCard(id: string, dw: number, dh: number) {
  const c = canvas.cards.find((c) => c.id === id);
  if (c) {
    const min = 20 * remPx();
    c.w = Math.max(min, c.w + dw);
    c.h = Math.max(min, c.h + dh);
  }
}

/** Aviso para refrescar el árbol de archivos y los archivos abiertos (una IA o el usuario cambió algo). */
export const filesChanged = $state({ version: 0 });
export const bumpFiles = () => (filesChanged.version += 1);

export function renameCard(id: string, title: string) {
  const c = canvas.cards.find((c) => c.id === id);
  const t = title.trim();
  if (c && t) c.title = t.slice(0, 40);
}

const workspaceJobs = new Map<string, Promise<void>>();

async function automaticProject(card: Card) {
  const p = await createChatWorkspace(card.id, card.title);
  if (!projects.list.some((x) => x.id === p.id)) projects.list = [...projects.list, p];
  return p;
}

/** La carpeta automática se prepara solo antes del primer mensaje enviado a una IA. */
export function ensureChatWorkspace(id: string): Promise<void> {
  const existing = workspaceJobs.get(id);
  if (existing) return existing;
  const card = canvas.cards.find((c) => c.id === id && c.kind === "chat");
  if (!card || card.projectId) return Promise.resolve();
  const members = [card, ...linkedChats(id)];
  members.forEach((c) => (c.workspacePending = true));
  const job = (async () => {
    const projectId = members.find((c) => c.projectId)?.projectId ?? (await automaticProject(card)).id;
    await assignWorkspace(members, projectId);
  })().finally(() => {
    members.forEach((c) => { c.workspacePending = false; workspaceJobs.delete(c.id); });
    saveLayout();
  });
  members.forEach((c) => workspaceJobs.set(c.id, job));
  return job;
}

async function assignWorkspace(cards: Card[], projectId: string | null) {
  if (cards.some((c) => c.busy)) throw new Error("Espera a que terminen los chats antes de cambiar su carpeta.");
  for (const card of cards) {
    if (card.projectId === projectId) continue;
    if (card.kind === "chat") await moveChatWorkspace(card.id, card.projectId, projectId);
    card.projectId = projectId;
  }
  touchProject(projectId);
}

export async function setCardProject(id: string, projectId: string | null) {
  const c = canvas.cards.find((c) => c.id === id);
  if (!c || canvas.projectSync) return;
  canvas.projectSync = true;
  canvas.projectError = "";
  try {
    await assignWorkspace(c.kind === "chat" ? [c, ...linkedChats(id)] : [c], projectId);
  }
  catch (e) { canvas.projectError = String(e); }
  finally { canvas.projectSync = false; saveLayout(); }
}

/** Cierra los chats y archivos de un proyecto (cuando se quita de CoModel). */
export function removeCardsOfProject(projectId: string) {
  for (const c of [...canvas.cards]) if (c.projectId === projectId) removeCard(c.id);
}

export function setBusy(id: string, busy: boolean) {
  const c = canvas.cards.find((c) => c.id === id);
  if (c) c.busy = busy;
}

let raf = 0;

/** Mueve la cámara hasta `to` con una curva suave. Sin movimiento (o con "reducir movimiento"), salta. */
export function tweenView(to: { x: number; y: number; k: number }, duration = 280) {
  cancelAnimationFrame(raf);
  const v = canvas.view;
  if (reducedMotion() || duration <= 0) {
    v.x = to.x;
    v.y = to.y;
    v.k = to.k;
    return;
  }
  const from = { x: v.x, y: v.y, k: v.k };
  const t0 = performance.now();
  const step = (now: number) => {
    const t = Math.min(1, (now - t0) / duration);
    const e = 1 - Math.pow(1 - t, 4); // quart out
    v.x = from.x + (to.x - from.x) * e;
    v.y = from.y + (to.y - from.y) * e;
    v.k = from.k + (to.k - from.k) * e;
    if (t < 1) raf = requestAnimationFrame(step);
  };
  raf = requestAnimationFrame(step);
}

/** Vista que resulta de hacer zoom `factor` alrededor del punto (cx, cy) de la pantalla. */
function zoomTarget(factor: number, cx: number, cy: number) {
  const v = canvas.view;
  const k = clamp(v.k * factor, MIN_ZOOM, MAX_ZOOM);
  const r = k / v.k;
  return { x: cx - (cx - v.x) * r, y: cy - (cy - v.y) * r, k };
}

/** Zoom inmediato (rueda y pellizco): sigue al dedo, sin curva. */
export function zoomAt(factor: number, cx = canvas.size.w / 2, cy = canvas.size.h / 2) {
  cancelAnimationFrame(raf);
  Object.assign(canvas.view, zoomTarget(factor, cx, cy));
}

/** Zoom de botón: con movimiento suave. */
export function zoomStep(factor: number) {
  tweenView(zoomTarget(factor, canvas.size.w / 2, canvas.size.h / 2), 220);
}

export function panBy(dx: number, dy: number) {
  cancelAnimationFrame(raf);
  canvas.view.x += dx;
  canvas.view.y += dy;
}

export function resetZoom() {
  zoomStep(1 / canvas.view.k);
}

/** Encuadra todas las tarjetas. Sin tarjetas, vuelve al origen. */
export function fitAll() {
  const cards = canvas.cards;
  if (!cards.length) {
    tweenView({ x: 0, y: 0, k: 1 });
    return;
  }
  const minX = Math.min(...cards.map((c) => c.x));
  const minY = Math.min(...cards.map((c) => c.y));
  const maxX = Math.max(...cards.map((c) => c.x + c.w));
  const maxY = Math.max(...cards.map((c) => c.y + c.h));
  const pad = 48;
  const k = clamp(Math.min(canvas.size.w / (maxX - minX + pad * 2), canvas.size.h / (maxY - minY + pad * 2), 1), MIN_ZOOM, MAX_ZOOM);
  tweenView({ x: canvas.size.w / 2 - ((minX + maxX) / 2) * k, y: canvas.size.h / 2 - ((minY + maxY) / 2) * k, k });
}

export function centerOn(id: string) {
  const c = canvas.cards.find((c) => c.id === id);
  if (!c) return;
  const k = canvas.view.k;
  tweenView({ x: canvas.size.w / 2 - (c.x + c.w / 2) * k, y: canvas.size.h / 2 - (c.y + c.h / 2) * k, k });
  focusCard(id);
}

/** Al cambiar el tamaño del texto, las tarjetas crecen o se encogen con él para que el contenido siga cabiendo. */
export function rescaleCards(ratio: number) {
  if (!Number.isFinite(ratio) || ratio <= 0 || ratio === 1) return;
  for (const c of canvas.cards) {
    c.w = Math.round(c.w * ratio);
    c.h = Math.round(c.h * ratio);
  }
}

// ---------- Persistencia del lienzo ----------
// Se guarda la disposición (tarjetas, nombres, carpeta de cada chat, vista); las conversaciones se vuelven a leer del historial.

const LAYOUT_KEY = "comodel.layout.v2";

interface SavedLayout {
  links?: { a: string; b: string }[];
  cards: { id: string; title: string; x: number; y: number; w: number; h: number; projectId: string | null; kind?: Card["kind"]; path?: string | null; content?: string; language?: string; ai?: ChatAI }[];
  view: { x: number; y: number; k: number };
  n: number;
}

/** Lo que se guarda. Leer los campos aquí hace que quien lo llame dentro de un efecto se entere de los cambios. */
export function layoutSnapshot(): SavedLayout {
  return {
    links: canvas.links.map((l) => ({ ...l })),
    cards: canvas.cards.map((c) => ({ id: c.id, title: c.title, x: Math.round(c.x), y: Math.round(c.y), w: Math.round(c.w), h: Math.round(c.h), projectId: c.projectId, kind: c.kind, path: c.path, content: c.content, language: c.language, ai: c.ai ? { ...c.ai } : undefined })),
    view: { x: Math.round(canvas.view.x), y: Math.round(canvas.view.y), k: Math.round(canvas.view.k * 1000) / 1000 },
    n: canvas.n,
  };
}

export function saveLayout() {
  try {
    localStorage.setItem(LAYOUT_KEY, JSON.stringify(layoutSnapshot()));
  } catch {
    // sin almacenamiento: el lienzo no se recordará
  }
}

/** Pone en el lienzo lo guardado. `exists` indica qué proyectos siguen existiendo: los chats de uno que ya no está se quedan sin carpeta. */
export function loadLayout(exists: (projectId: string) => boolean) {
  cancelAnimationFrame(raf);
  let saved: SavedLayout | null = null;
  try {
    saved = JSON.parse(localStorage.getItem(LAYOUT_KEY) ?? "null") as SavedLayout | null;
  } catch {
    saved = null;
  }
  // Un archivo de un proyecto que ya no existe no se puede abrir: se descarta. Un chat sin su proyecto se queda sin carpeta.
  canvas.cards = (saved?.cards ?? [])
    .filter((c) => c.kind !== "file" || (c.projectId && exists(c.projectId)))
    .map((c) => ({ ...c, kind: c.kind ?? "chat", path: c.path ?? null, projectId: c.projectId && exists(c.projectId) ? c.projectId : null, z: ++canvas.z, busy: false }));
  canvas.links = (saved?.links ?? []).filter((l) => l.a !== l.b && [l.a, l.b].every((id) => canvas.cards.some((c) => c.id === id && c.kind === "chat")));
  canvas.focusId = null;
  canvas.n = saved?.n ?? 0;
  canvas.view.x = saved?.view.x ?? 0;
  canvas.view.y = saved?.view.y ?? 0;
  canvas.view.k = saved?.view.k ?? 1;
}

/** Paneles independientes; el selector de archivos no depende del chat enfocado. */
export function openPanel(kind: "files" | "code", projectId: string | null = null, content = "", language = "txt") {
  const card = addCard(undefined, projectId);
  card.kind = kind;
  card.title = kind === "files" ? "Archivos" : `Código.${({ javascript: "js", typescript: "ts", python: "py", rust: "rs", bash: "sh" } as Record<string, string>)[language] ?? language}`;
  card.content = content;
  card.language = language;
  card.w = Math.round((kind === "files" ? 24 : 38) * remPx());
  return card;
}

export async function toggleChatLink(a: string, b: string) {
  if (canvas.projectSync || a === b || ![a, b].every((id) => canvas.cards.some((c) => c.id === id && c.kind === "chat"))) return;
  const i = canvas.links.findIndex((l) => (l.a === a && l.b === b) || (l.a === b && l.b === a));
  if (i >= 0) { canvas.links.splice(i, 1); return; }
  const first = canvas.cards.find((c) => c.id === a)!;
  const second = canvas.cards.find((c) => c.id === b)!;
  const members = [...new Map([first, ...linkedChats(a), second, ...linkedChats(b)].map((c) => [c.id, c])).values()];
  if (members.some((c) => c.workspacePending || c.busy)) {
    canvas.projectError = "Espera a que terminen los chats que quieres conectar.";
    return;
  }
  if (members.length > 33) { canvas.projectError = "Conecta como máximo 33 chats por grupo."; return; }
  // Manda la carpeta del chat desde el que se conecta; si no tiene, se hereda la del otro grupo.
  const projectId = first.projectId ?? members.find((c) => c.projectId)?.projectId ?? null;
  canvas.projectSync = true;
  canvas.projectError = "";
  try {
    await assignWorkspace(members, projectId);
    canvas.links.push({ a, b });
  } catch (e) { canvas.projectError = String(e); }
  finally { canvas.projectSync = false; saveLayout(); }
}

/** Alinea también los grupos conectados con versiones anteriores de CoModel. */
export async function syncLinkedWorkspaces() {
  const visited = new Set<string>();
  for (const card of canvas.cards.filter((c) => c.kind === "chat")) {
    if (visited.has(card.id)) continue;
    const members = [card, ...linkedChats(card.id)];
    members.forEach((c) => visited.add(c.id));
    const projectId = members.find((c) => c.projectId)?.projectId ?? null;
    if (members.some((c) => c.projectId !== projectId)) {
      await setCardProject(card.id, projectId);
      if (canvas.projectError) break;
    }
  }
}

/** Todos los chats del grupo conectado reciben el contexto de sus compañeros en cada turno. */
export function linkedChats(id: string) {
  const ids = new Set([id]);
  let changed = true;
  while (changed) {
    changed = false;
    for (const l of canvas.links) {
      if (ids.has(l.a) && !ids.has(l.b)) { ids.add(l.b); changed = true; }
      if (ids.has(l.b) && !ids.has(l.a)) { ids.add(l.a); changed = true; }
    }
  }
  return canvas.cards.filter((c) => c.kind === "chat" && c.id !== id && ids.has(c.id));
}
