// Estado del lienzo: tarjetas y vista (desplazamiento y zoom). Es de la ventana entera, no de un componente.
import type { ChatAI } from "./ai.svelte";
import { closeChat, createChatWorkspace, moveChatWorkspace, termClose } from "./backend";
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
  /** En qué lienzo vive: `general` o el id de un proyecto. */
  space: string;
  /** Un chat o un archivo abierto en el editor. */
  kind: "chat" | "file" | "files" | "code" | "terminal";
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

/** Un workspace: un lienzo con sus chats, que comparten una carpeta y una memoria. */
export interface Workspace {
  id: string;
  name: string;
  /** Su carpeta de trabajo; se prepara sola antes del primer mensaje. */
  projectId: string | null;
}

export const canvas = $state({
  /** Workspace que se está viendo (vacío si no hay ninguno). Las tarjetas de los demás siguen vivas, solo ocultas. */
  space: "",
  workspaces: [] as Workspace[],
  /** Vista (desplazamiento y zoom) de cada workspace que no es el actual. */
  views: {} as Record<string, { x: number; y: number; k: number }>,
  cards: [] as Card[],
  projectSync: false,
  projectError: "",
  view: { x: 0, y: 0, k: 1 },
  size: { w: 900, h: 600 },
  focusId: null as string | null,
  z: 1,
  n: 0,
  /** Contador para dar nombre a los workspaces nuevos. */
  wn: 0,
});

/** Tarjetas del workspace que se está viendo. */
export const here = () => canvas.cards.filter((c) => c.space === canvas.space);

export const workspaceById = (id: string | null | undefined) => (id ? (canvas.workspaces.find((w) => w.id === id) ?? null) : null);

/** Las carpetas automáticas (CoModel/Chats/Workspace #n) no son proyectos que el usuario haya añadido. */
export const isChatFolder = (path: string) => /[\\/]Chats[\\/](Chat|Workspace) #\d+/.test(path);

/** Cambia de workspace guardando la vista del actual y recuperando la del otro. */
export function switchSpace(space: string) {
  if (space === canvas.space) return;
  cancelAnimationFrame(raf);
  if (canvas.space) canvas.views[canvas.space] = { x: canvas.view.x, y: canvas.view.y, k: canvas.view.k };
  const v = canvas.views[space] ?? { x: 0, y: 0, k: 1 };
  canvas.space = space;
  canvas.view.x = v.x;
  canvas.view.y = v.y;
  canvas.view.k = v.k;
  canvas.focusId = here().slice(-1)[0]?.id ?? null;
}

/** Crea un workspace vacío y pasa a verlo. */
export function addWorkspace(name?: string, projectId: string | null = null): Workspace {
  canvas.wn += 1;
  const ws: Workspace = { id: `w${Date.now().toString(36)}${canvas.wn}`, name: (name ?? `Workspace ${canvas.wn}`).trim().slice(0, 40), projectId };
  canvas.workspaces.push(ws);
  switchSpace(ws.id);
  return ws;
}

export function renameWorkspace(id: string, name: string) {
  const ws = workspaceById(id);
  const n = name.trim();
  if (ws && n) ws.name = n.slice(0, 40);
}

/** Cierra un workspace y sus tarjetas. El historial de sus chats se conserva en su carpeta. */
export function removeWorkspace(id: string) {
  for (const c of [...canvas.cards]) if (c.space === id) removeCard(c.id);
  const i = canvas.workspaces.findIndex((w) => w.id === id);
  if (i < 0) return;
  const wasCurrent = canvas.space === id;
  canvas.workspaces.splice(i, 1);
  delete canvas.views[id];
  if (wasCurrent) {
    canvas.space = "";
    switchSpace(canvas.workspaces[Math.min(i, canvas.workspaces.length - 1)]?.id ?? "");
  }
}

const remPx = () => parseFloat(getComputedStyle(document.documentElement).fontSize) || 16;
const clamp = (v: number, a: number, b: number) => Math.min(b, Math.max(a, v));

/** Abre un chat. `at` es el centro deseado en coordenadas del lienzo; sin él, el centro de la vista. */
export function addCard(at?: { x: number; y: number }, projectId?: string | null, init?: { id?: string; title?: string; ai?: ChatAI }): Card {
  canvas.n += 1;
  const w = Math.round(30 * remPx());
  // Que quepa entera en lo que se ve del lienzo.
  const h = Math.round(Math.min(40 * remPx(), Math.max(20 * remPx(), (canvas.size.h - 64) / canvas.view.k)));
  // Sin workspace donde ponerlo, se abre uno.
  const ws = workspaceById(canvas.space) ?? addWorkspace();
  const cascade = (here().length % 6) * 28;
  const cx = at?.x ?? (canvas.size.w / 2 - canvas.view.x) / canvas.view.k + cascade;
  const cy = at?.y ?? (canvas.size.h / 2 - canvas.view.y) / canvas.view.k + cascade;
  // Un chat nuevo trabaja en la carpeta de su workspace.
  const folder = projectId !== undefined ? projectId : ws.projectId;
  const card: Card = {
    id: init?.id ?? `c${Date.now().toString(36)}${canvas.n}`,
    title: init?.title ?? `Chat ${canvas.n}`,
    x: Math.round(cx - w / 2),
    y: Math.round(cy - h / 2),
    w,
    h,
    z: ++canvas.z,
    busy: false,
    projectId: folder,
    space: canvas.space,
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
  const cascade = (here().length % 6) * 28;
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
    space: canvas.space,
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
  if (canvas.focusId === id) canvas.focusId = here().slice(-1)[0]?.id ?? null;
  if (kind === "chat") void closeChat(id).catch(() => {});
  if (kind === "terminal") void termClose(id).catch(() => {});
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

async function automaticProject(ws: Workspace) {
  const p = await createChatWorkspace(ws.id, ws.name);
  if (!projects.list.some((x) => x.id === p.id)) projects.list = [...projects.list, p];
  return p;
}

/** La carpeta automática se prepara solo antes del primer mensaje enviado a una IA. */
export function ensureChatWorkspace(id: string): Promise<void> {
  const existing = workspaceJobs.get(id);
  if (existing) return existing;
  const card = canvas.cards.find((c) => c.id === id && c.kind === "chat");
  if (!card || card.projectId) return Promise.resolve();
  const ws = workspaceById(card.space);
  const members = [card, ...linkedChats(id)];
  members.forEach((c) => (c.workspacePending = true));
  const job = (async () => {
    const projectId = ws?.projectId ?? members.find((c) => c.projectId)?.projectId ?? (await automaticProject(ws ?? { id: card.id, name: card.title, projectId: null })).id;
    await assignWorkspace(members, projectId);
    if (ws) ws.projectId = projectId;
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
    // La carpeta es del workspace entero.
    const ws = workspaceById(c.space);
    if (ws && c.kind === "chat") ws.projectId = projectId;
  }
  catch (e) { canvas.projectError = String(e); }
  finally { canvas.projectSync = false; saveLayout(); }
}

/** Cierra los chats y archivos de un proyecto (cuando se quita de CoModel). */
export function removeCardsOfProject(projectId: string) {
  for (const c of [...canvas.cards]) if (c.projectId === projectId) removeCard(c.id);
  for (const w of canvas.workspaces.filter((w) => w.projectId === projectId)) removeWorkspace(w.id);
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
  const cards = here();
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
  switchSpace(c.space);
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

interface SavedCard {
  id: string;
  space?: string;
  title: string;
  x: number;
  y: number;
  w: number;
  h: number;
  projectId: string | null;
  kind?: Card["kind"];
  path?: string | null;
  content?: string;
  language?: string;
  ai?: ChatAI;
}

interface SavedLayout {
  workspaces?: Workspace[];
  /** Solo de versiones anteriores: chats conectados. */
  links?: { a: string; b: string }[];
  space?: string;
  views?: Record<string, { x: number; y: number; k: number }>;
  cards: SavedCard[];
  view: { x: number; y: number; k: number };
  n: number;
  wn?: number;
}

/** Lo que se guarda. Leer los campos aquí hace que quien lo llame dentro de un efecto se entere de los cambios. */
export function layoutSnapshot(): SavedLayout {
  return {
    workspaces: canvas.workspaces.map((w) => ({ ...w })),
    space: canvas.space,
    views: { ...Object.fromEntries(Object.entries(canvas.views).map(([k, v]) => [k, { ...v }])) },
    cards: canvas.cards.map((c) => ({ id: c.id, space: c.space, title: c.title, x: Math.round(c.x), y: Math.round(c.y), w: Math.round(c.w), h: Math.round(c.h), projectId: c.projectId, kind: c.kind, path: c.path, content: c.content, language: c.language, ai: c.ai ? { ...c.ai } : undefined })),
    view: { x: Math.round(canvas.view.x), y: Math.round(canvas.view.y), k: Math.round(canvas.view.k * 1000) / 1000 },
    n: canvas.n,
    wn: canvas.wn,
  };
}

export function saveLayout() {
  try {
    localStorage.setItem(LAYOUT_KEY, JSON.stringify(layoutSnapshot()));
  } catch {
    // sin almacenamiento: el lienzo no se recordará
  }
}

/** Convierte lo guardado antes de los workspaces: un workspace por cada grupo de chats conectados, y uno por cada proyecto que tenía su lienzo. */
function migrate(saved: SavedLayout, cards: SavedCard[], projectName: (id: string) => string | null): { workspaces: Workspace[]; spaceOf: Map<string, string>; views: Record<string, { x: number; y: number; k: number }>; current: string } {
  const workspaces: Workspace[] = [];
  const spaceOf = new Map<string, string>();
  const views: Record<string, { x: number; y: number; k: number }> = {};
  let wn = 0;
  const make = (name: string, projectId: string | null) => {
    wn += 1;
    const ws: Workspace = { id: `w${Date.now().toString(36)}${wn}`, name: name.slice(0, 40), projectId };
    workspaces.push(ws);
    return ws;
  };
  const generalView = (saved.space && saved.space !== "general" ? saved.views?.general : saved.view) ?? { x: 0, y: 0, k: 1 };
  const inGeneral = (c: SavedCard) => !c.space || c.space === "general" || projectName(c.space) === null;

  // Lienzos de proyecto: cada uno pasa a ser un workspace con el nombre del proyecto.
  for (const space of new Set(cards.filter((c) => !inGeneral(c)).map((c) => c.space!))) {
    const ws = make(projectName(space)!, space);
    cards.filter((c) => c.space === space).forEach((c) => spaceOf.set(c.id, ws.id));
    views[ws.id] = (saved.space === space ? saved.view : saved.views?.[space]) ?? { x: 0, y: 0, k: 1 };
  }
  // Chats sueltos: los conectados entre sí forman un workspace; los demás, uno cada uno.
  const chats = cards.filter((c) => inGeneral(c) && (c.kind ?? "chat") === "chat");
  const parent = new Map(chats.map((c) => [c.id, c.id]));
  const find = (x: string): string => (parent.get(x) === x ? x : find(parent.get(x)!));
  for (const l of saved.links ?? []) if (parent.has(l.a) && parent.has(l.b)) parent.set(find(l.a), find(l.b));
  const groups = new Map<string, SavedCard[]>();
  for (const c of chats) groups.set(find(c.id), [...(groups.get(find(c.id)) ?? []), c]);
  for (const members of groups.values()) {
    const ws = make(members[0].title, members.find((c) => c.projectId)?.projectId ?? null);
    members.forEach((c) => spaceOf.set(c.id, ws.id));
    views[ws.id] = generalView;
  }
  // Archivos y paneles: con el workspace de su proyecto, o con el primero.
  for (const c of cards.filter((c) => inGeneral(c) && (c.kind ?? "chat") !== "chat")) {
    const ws = workspaces.find((w) => w.projectId && w.projectId === c.projectId) ?? workspaces[0] ?? make("Workspace 1", c.projectId);
    if (!views[ws.id]) views[ws.id] = generalView;
    spaceOf.set(c.id, ws.id);
  }
  canvas.wn = Math.max(saved.wn ?? 0, wn);
  const currentCard = cards.find((c) => c.space === saved.space) ?? cards.find((c) => c.id === (saved as { focusId?: string }).focusId);
  const current = (saved.space && saved.space !== "general" ? workspaces.find((w) => w.projectId === saved.space)?.id : undefined) ?? (currentCard ? spaceOf.get(currentCard.id) : undefined) ?? workspaces[0]?.id ?? "";
  return { workspaces, spaceOf, views, current };
}

/** Pone en el lienzo lo guardado. `projectName` da el nombre de un proyecto, o `null` si ya no existe: los chats de uno que ya no está se quedan sin carpeta. */
export function loadLayout(projectName: (projectId: string) => string | null) {
  cancelAnimationFrame(raf);
  const exists = (id: string) => projectName(id) !== null;
  let saved: SavedLayout | null = null;
  try {
    saved = JSON.parse(localStorage.getItem(LAYOUT_KEY) ?? "null") as SavedLayout | null;
  } catch {
    saved = null;
  }
  // Un archivo de un proyecto que ya no existe no se puede abrir: se descarta.
  const cards = (saved?.cards ?? []).filter((c) => c.kind !== "file" || (c.projectId && exists(c.projectId)));
  let current = "";
  let views: Record<string, { x: number; y: number; k: number }> = {};
  let spaceOf = new Map<string, string>();
  if (saved?.workspaces) {
    canvas.workspaces = saved.workspaces.map((w) => ({ ...w, projectId: w.projectId && exists(w.projectId) ? w.projectId : null }));
    for (const c of cards) if (c.space && canvas.workspaces.some((w) => w.id === c.space)) spaceOf.set(c.id, c.space);
    views = saved.views ?? {};
    current = saved.space && canvas.workspaces.some((w) => w.id === saved!.space) ? saved.space : (canvas.workspaces[0]?.id ?? "");
    canvas.wn = saved.wn ?? canvas.workspaces.length;
  } else if (saved) {
    const m = migrate(saved, cards, projectName);
    canvas.workspaces = m.workspaces;
    spaceOf = m.spaceOf;
    views = m.views;
    current = m.current;
  } else {
    canvas.workspaces = [];
    canvas.wn = 0;
  }
  canvas.cards = cards
    .filter((c) => spaceOf.has(c.id))
    .map((c) => ({ ...c, space: spaceOf.get(c.id)!, kind: c.kind ?? "chat", path: c.path ?? null, projectId: c.projectId && exists(c.projectId) ? c.projectId : null, z: ++canvas.z, busy: false }));
  canvas.views = views;
  canvas.space = "";
  canvas.focusId = null;
  canvas.n = saved?.n ?? 0;
  canvas.view.x = 0;
  canvas.view.y = 0;
  canvas.view.k = 1;
  // Se entra por el workspace que se estaba viendo, con su vista guardada.
  const v = (saved?.workspaces ? saved.view : views[current]) ?? { x: 0, y: 0, k: 1 };
  canvas.space = current;
  canvas.view.x = v.x;
  canvas.view.y = v.y;
  canvas.view.k = v.k;
  delete canvas.views[current];
  canvas.focusId = here().slice(-1)[0]?.id ?? null;
}

/** Paneles independientes; el selector de archivos no depende del chat enfocado. */
export function openPanel(kind: "files" | "code" | "terminal", projectId?: string | null, content = "", language = "txt") {
  const card = addCard(undefined, projectId);
  card.kind = kind;
  card.title = kind === "terminal" ? "Terminal" : kind === "files" ? "Archivos" : `Código.${({ javascript: "js", typescript: "ts", python: "py", rust: "rs", bash: "sh" } as Record<string, string>)[language] ?? language}`;
  card.content = content;
  card.language = language;
  card.w = Math.round((kind === "files" ? 24 : kind === "terminal" ? 44 : 38) * remPx());
  if (kind === "terminal") card.h = Math.round(26 * remPx());
  return card;
}

/** Alinea la carpeta de los chats de cada workspace (los de versiones anteriores podían tener cada uno la suya). */
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

/** Los demás chats del mismo workspace: comparten su carpeta y reciben el contexto de sus compañeros en cada turno. */
export function linkedChats(id: string) {
  const space = canvas.cards.find((c) => c.id === id)?.space;
  return canvas.cards.filter((c) => c.kind === "chat" && c.id !== id && c.space === space);
}
