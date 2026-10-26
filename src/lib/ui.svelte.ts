// Qué pantalla se ve. El lienzo siempre queda montado debajo para no perder los chats.
export type Section = "perfil" | "proyecto" | "ias" | "uso" | "apariencia" | "atajos";

const SIDEBAR_KEY = "comodel.sidebarCollapsed";

function readFlag(key: string): boolean {
  try {
    return localStorage.getItem(key) === "1";
  } catch {
    return false;
  }
}

export const ui = $state({
  /** Capa de Ajustes (un diálogo) encima de la pantalla actual. */
  view: "canvas" as "canvas" | "settings",
  /** Pantalla principal: la de inicio (al abrir la app) o el lienzo. */
  screen: "home" as "home" | "canvas",
  section: "perfil" as Section,
  sidebarTab: "chats" as "chats" | "projects" | "files",
  /** Barra lateral reducida a una columna de iconos. */
  sidebarCollapsed: readFlag(SIDEBAR_KEY),
  /** Paleta de búsqueda y acciones (Ctrl+K). */
  paletteOpen: false,
  filesProject: null as string | null,
  /** Diálogo para importar un chat desde un enlace. */
  importOpen: false,
  /** Primer mensaje de un chat recién creado desde Inicio: el chat lo envía al abrirse. */
  pending: null as { cardId: string; text: string } | null,
});

export function openSettings(section: Section = "apariencia") {
  ui.section = section;
  ui.view = "settings";
}

/** Cierra Ajustes y deja la pantalla donde estaba. */
export function closeSettings() {
  ui.view = "canvas";
}

export function showCanvas() {
  ui.view = "canvas";
  ui.screen = "canvas";
}

export function showHome() {
  ui.view = "canvas";
  ui.screen = "home";
}

export function toggleSidebar() {
  ui.sidebarCollapsed = !ui.sidebarCollapsed;
  try {
    localStorage.setItem(SIDEBAR_KEY, ui.sidebarCollapsed ? "1" : "0");
  } catch {
    // sin almacenamiento: solo dura esta sesión
  }
}
