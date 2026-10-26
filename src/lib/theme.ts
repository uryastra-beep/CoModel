export const THEMES = [
  { id: "graphite", label: "Grafito", hint: "Oscuro y neutro" },
  { id: "violet", label: "Violeta", hint: "Oscuro con el color de la marca" },
  { id: "paper", label: "Papel", hint: "Claro" },
] as const;

export type ThemeId = (typeof THEMES)[number]["id"];

export const TEXT_SCALES = [
  { id: "0.9", label: "Pequeño" },
  { id: "1", label: "Normal" },
  { id: "1.15", label: "Grande" },
  { id: "1.3", label: "Muy grande" },
] as const;

// v2: los temas cambiaron con el rediseño; los ids viejos se traducen una vez.
const THEME_KEY = "comodel.theme.v2";
const OLD_THEME_KEY = "comodel.theme";
const OLD_THEMES: Record<string, ThemeId> = { "purple-dark": "violet", "github-dark": "graphite", "purple-light": "paper" };
const SCALE_KEY = "comodel.textScale";
const MOTION_KEY = "comodel.reduceMotion";

function read(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function write(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    // Sin almacenamiento disponible: el ajuste solo dura esta sesión.
  }
}

function isTheme(v: string | null): v is ThemeId {
  return THEMES.some((t) => t.id === v);
}

export function currentTheme(): ThemeId {
  const v = document.documentElement.getAttribute("data-theme");
  return isTheme(v) ? v : "graphite";
}

export function setTheme(id: ThemeId) {
  document.documentElement.setAttribute("data-theme", id);
  write(THEME_KEY, id);
}

export function currentTextScale(): string {
  const v = document.documentElement.style.getPropertyValue("--text-scale") || "1";
  return TEXT_SCALES.some((s) => s.id === v) ? v : "1";
}

export function setTextScale(id: string) {
  if (!TEXT_SCALES.some((s) => s.id === id)) return;
  document.documentElement.style.setProperty("--text-scale", id);
  write(SCALE_KEY, id);
}

/** Si el movimiento está reducido ahora mismo (por el ajuste o, sin ajuste, por el sistema). */
export function reducedMotion(): boolean {
  return document.documentElement.dataset.motion === "reduced";
}

export function setReducedMotion(reduced: boolean) {
  document.documentElement.dataset.motion = reduced ? "reduced" : "full";
  write(MOTION_KEY, reduced ? "1" : "0");
}

export function initTheme() {
  // Sin elección guardada se respeta la opción de reducir movimiento del sistema.
  const m = read(MOTION_KEY);
  const system = typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;
  document.documentElement.dataset.motion = (m === null ? system : m === "1") ? "reduced" : "full";

  // Un tema elegido solo en la versión anterior (el claro) se conserva; los oscuros pasan al nuevo por defecto.
  let t = read(THEME_KEY);
  if (!isTheme(t)) {
    const old = read(OLD_THEME_KEY);
    t = old === "purple-light" ? OLD_THEMES[old] : null;
  }
  document.documentElement.setAttribute("data-theme", isTheme(t) ? t : "graphite");
  setTextScale(read(SCALE_KEY) ?? "1");
}
