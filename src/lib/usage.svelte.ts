// Uso restante por proveedor. Es de la cuenta, no de un chat, así que vive fuera de las tarjetas.

export interface Window {
  fraction: number;
  resetsAt: number | null;
}

export const usage = $state<{ byProvider: Record<string, Record<string, Window>> }>({ byProvider: {} });

export function recordQuota(provider: string, window: string, fraction: number, resetsAt: number | null) {
  (usage.byProvider[provider] ??= {})[window] = { fraction, resetsAt };
}

export function resetsIn(ts: number | null): string | undefined {
  if (!ts) return undefined;
  const mins = Math.max(0, Math.round((ts * 1000 - Date.now()) / 60000));
  return mins >= 60 ? `se reinicia en ${Math.floor(mins / 60)} h ${mins % 60} min` : `se reinicia en ${mins} min`;
}

/** Último relevo entre IAs, de cualquier chat. */
export const handoffs = $state<{ last: { from: string; to: string; reason: string; at: number } | null }>({ last: null });

export function recordHandoff(from: string, to: string, reason: string) {
  handoffs.last = { from, to, reason, at: Date.now() };
}

export function ago(at: number, now = Date.now()): string {
  const s = Math.max(0, Math.round((now - at) / 1000));
  if (s < 60) return "hace un momento";
  const m = Math.round(s / 60);
  if (m < 60) return `hace ${m} min`;
  return `hace ${Math.round(m / 60)} h`;
}
