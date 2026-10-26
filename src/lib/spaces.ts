// Los workspaces que se pueden elegir, para menús y búsquedas.
import type { Choice } from "./providers";
import { canvas } from "./canvas.svelte";
import { projectById } from "./projects.svelte";

export const chatsIn = (space: string) => canvas.cards.filter((c) => c.kind === "chat" && c.space === space).length;

export function workspaceChoices(): Choice[] {
  return canvas.workspaces.map((w) => {
    const n = chatsIn(w.id);
    return { id: w.id, label: w.name, hint: projectById(w.projectId)?.name ?? "Su carpeta se creará al enviar el primer mensaje", note: n ? String(n) : undefined };
  });
}

export function workspaceName(space: string): string {
  return canvas.workspaces.find((w) => w.id === space)?.name ?? "";
}
