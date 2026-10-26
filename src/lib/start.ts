// Abrir un chat nuevo desde cualquier sitio (Inicio, la lista de proyectos, la paleta...).
import type { ChatAI } from "./ai.svelte";
import { addCard } from "./canvas.svelte";
import { touchProject } from "./projects.svelte";
import { showCanvas, ui } from "./ui.svelte";

/** Crea un chat en el lienzo con su carpeta (o sin ella) y, si hay texto, lo envía al abrirse. */
export function startChat(message: string, projectId: string | null, ai?: ChatAI) {
  showCanvas();
  const card = addCard(undefined, projectId, { ai });
  touchProject(projectId);
  if (message.trim()) ui.pending = { cardId: card.id, text: message.trim() };
  return card;
}
