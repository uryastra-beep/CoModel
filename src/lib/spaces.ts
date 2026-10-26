// Los lienzos que se pueden elegir: el general y uno por cada proyecto (las carpetas automáticas de los chats no cuentan).
import type { Choice } from "./providers";
import { canvas, GENERAL, isChatFolder } from "./canvas.svelte";
import { projects } from "./projects.svelte";

export function spaceChoices(): Choice[] {
  const count = (space: string) => canvas.cards.filter((c) => c.kind === "chat" && c.space === space).length;
  const note = (space: string) => (count(space) ? String(count(space)) : undefined);
  const list: Choice[] = [{ id: GENERAL, label: "General", hint: "Chats sin proyecto", note: note(GENERAL) }];
  for (const p of projects.list) if (!isChatFolder(p.path)) list.push({ id: p.id, label: p.name, hint: p.path, note: note(p.id) });
  // Un lienzo con chats siempre se puede elegir, aunque su proyecto ya no figure en la lista.
  return list;
}

export function spaceName(space: string): string {
  return spaceChoices().find((c) => c.id === space)?.label ?? "General";
}
