// Abrir un chat o un workspace nuevo desde cualquier sitio (Inicio, la lista de proyectos, la paleta...).
import type { ChatAI } from "./ai.svelte";
import { addCard, addWorkspace, canvas, switchSpace } from "./canvas.svelte";
import { projectById, touchProject } from "./projects.svelte";
import { showCanvas, ui } from "./ui.svelte";

/** Crea un workspace con un primer chat. */
export function startWorkspace(message = "", projectId: string | null = null, ai?: ChatAI) {
  showCanvas();
  const project = projectById(projectId);
  addWorkspace(project?.name, project?.id ?? null);
  return startChat(message, project?.id, ai);
}

/** Abre un chat en el workspace que se está viendo (o en uno nuevo si no hay) y, si hay texto, lo envía al abrirse.
 * Con un proyecto, el chat va al workspace de ese proyecto, que se crea si no existe. */
export function startChat(message: string, projectId: string | null | undefined, ai?: ChatAI) {
  showCanvas();
  const project = projectById(projectId);
  if (project) {
    const existing = canvas.workspaces.find((w) => w.projectId === project.id);
    if (existing) switchSpace(existing.id);
    else addWorkspace(project.name, project.id);
  }
  const card = addCard(undefined, project ? project.id : undefined, { ai });
  touchProject(project?.id ?? null);
  if (message.trim()) ui.pending = { cardId: card.id, text: message.trim() };
  return card;
}
