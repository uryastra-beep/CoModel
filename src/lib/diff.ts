// Diff mínimo para mostrar lo que va a cambiar (o cambió) una edición. No necesita el archivo entero:
// la herramienta de la IA ya trae el texto de antes y el de después.

export type Row = { t: "add" | "del" | "ctx" | "hunk"; text: string };

const lines = (s: string) => (s === "" ? [] : s.replace(/\r\n/g, "\n").split("\n"));

/** Reemplazo de `before` por `after`. Las líneas iguales al principio y al final quedan como contexto. */
export function diffRows(before: string | null, after: string, context = 2): Row[] {
  const b = before === null ? [] : lines(before);
  const a = lines(after);
  if (before === null) return a.map((text) => ({ t: "add", text }));

  let start = 0;
  while (start < b.length && start < a.length && b[start] === a[start]) start++;
  let end = 0;
  while (end < b.length - start && end < a.length - start && b[b.length - 1 - end] === a[a.length - 1 - end]) end++;

  const rows: Row[] = [];
  for (const text of b.slice(Math.max(0, start - context), start)) rows.push({ t: "ctx", text });
  for (const text of b.slice(start, b.length - end)) rows.push({ t: "del", text });
  for (const text of a.slice(start, a.length - end)) rows.push({ t: "add", text });
  for (const text of b.slice(b.length - end, Math.min(b.length, b.length - end + context))) rows.push({ t: "ctx", text });
  return rows;
}

export const countOf = (rows: Row[]) => ({ add: rows.filter((r) => r.t === "add").length, del: rows.filter((r) => r.t === "del").length });

const EDIT_TOOLS = ["Edit", "Write", "MultiEdit", "NotebookEdit"];
export const isEditTool = (tool: string) => EDIT_TOOLS.includes(tool);

/** Filas de diff de una llamada a una herramienta de edición, o `null` si no se puede mostrar. */
export function toolDiff(tool: string, input: Record<string, unknown>): { path: string; rows: Row[]; created: boolean } | null {
  const path = (input.file_path ?? input.notebook_path) as string | undefined;
  if (!path) return null;
  if (tool === "Write" && typeof input.content === "string") return { path, rows: diffRows(null, input.content), created: true };
  if (tool === "Edit" && typeof input.new_string === "string") return { path, rows: diffRows(String(input.old_string ?? ""), input.new_string), created: false };
  if (tool === "MultiEdit" && Array.isArray(input.edits)) {
    const rows = (input.edits as Record<string, unknown>[]).flatMap((e, i) => [
      ...(i > 0 ? [{ t: "ctx" as const, text: "···" }] : []),
      ...diffRows(String(e.old_string ?? ""), String(e.new_string ?? "")),
    ]);
    return { path, rows, created: false };
  }
  return { path, rows: [], created: false };
}

/** Convierte la salida de `git diff` en filas: se omiten las cabeceras y se conservan los tramos (`@@`). */
export function parseUnifiedDiff(text: string): Row[] {
  const rows: Row[] = [];
  let inHunk = false;
  for (const line of text.replace(/\r\n/g, "\n").split("\n")) {
    if (line.startsWith("@@")) {
      inHunk = true;
      rows.push({ t: "hunk", text: line.replace(/^@@ .*? @@ ?/, "").trim() || line });
    } else if (!inHunk || line.startsWith("\\")) continue;
    else if (line.startsWith("+")) rows.push({ t: "add", text: line.slice(1) });
    else if (line.startsWith("-")) rows.push({ t: "del", text: line.slice(1) });
    else if (line.startsWith(" ")) rows.push({ t: "ctx", text: line.slice(1) });
  }
  return rows;
}
