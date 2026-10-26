// Abrir un chat nuevo desde cualquier sitio (Inicio, la lista de proyectos, la paleta...).
import type { ChatAI } from "./ai.svelte";
import { addCard, isChatFolder, switchSpace } from "./canvas.svelte";
import { projectById, touchProject } from "./projects.svelte";
import { showCanvas, ui } from "./ui.svelte";

/** Crea un chat en el lienzo con su carpeta (o sin ella) y, si hay texto, lo envía al abrirse. */
export function startChat(message: string, projectId: string | null, ai?: ChatAI) {
  showCanvas();
  // Un chat en un proyecto se abre en el lienzo de ese proyecto; sin proyecto, en el lienzo que se está viendo.
  const project = projectById(projectId);
  if (project && !isChatFolder(project.path)) switchSpace(project.id);
  const card = addCard(undefined, projectId ?? undefined, { ai });
  touchProject(projectId);
  if (message.trim()) ui.pending = { cardId: card.id, text: message.trim() };
  return card;
}
