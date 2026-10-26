// Proyectos del usuario: carpetas con sus IAs y su orden de relevo. Un chat puede estar en uno o en ninguno.
import { createProject, listProjects, openFolder as openFolderApi, removeProject, setDefaultRelay as setDefaultRelayApi, updateProject, type Project } from "./backend";

export const projects = $state<{ list: Project[]; defaultRelay: string[]; loaded: boolean }>({ list: [], defaultRelay: ["claude"], loaded: false });

const RECENT_KEY = "comodel.recent";

function readRecent(): Record<string, number> {
  try {
    return JSON.parse(localStorage.getItem(RECENT_KEY) ?? "{}") as Record<string, number>;
  } catch {
    return {};
  }
}

/** Cuándo se usó cada proyecto por última vez, para ordenar los recientes en Inicio. */
export const recent = $state<{ at: Record<string, number> }>({ at: readRecent() });

export function touchProject(id: string | null) {
  if (!id) return;
  recent.at[id] = Date.now();
  try {
    localStorage.setItem(RECENT_KEY, JSON.stringify(recent.at));
  } catch {
    // sin almacenamiento: solo dura esta sesión
  }
}

export const projectById = (id: string | null | undefined) => (id ? (projects.list.find((p) => p.id === id) ?? null) : null);

export async function loadProjects() {
  const s = await listProjects();
  projects.list = s.projects;
  projects.defaultRelay = s.default_relay;
  projects.loaded = true;
}

/** Crea el proyecto. Lanza el texto del error si algo no es válido. */
export async function addProject(name: string, path: string, relay: string[]) {
  const p = await createProject(name, path, relay);
  projects.list = [...projects.list, p];
  return p;
}

/** Abre una carpeta suelta como proyecto (si ya lo era, devuelve el existente). */
export async function openFolder(path: string) {
  const p = await openFolderApi(path);
  if (!projects.list.some((x) => x.id === p.id)) projects.list = [...projects.list, p];
  return p;
}

export async function editProject(id: string, name: string | null, relay: string[] | null) {
  const p = await updateProject(id, name, relay);
  projects.list = projects.list.map((x) => (x.id === id ? p : x));
}

export async function deleteProject(id: string) {
  await removeProject(id);
  projects.list = projects.list.filter((p) => p.id !== id);
}

export async function changeDefaultRelay(relay: string[]) {
  await setDefaultRelayApi(relay);
  projects.defaultRelay = relay;
}
