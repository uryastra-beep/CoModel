// Estado de las IAs conectadas. Lo comparten Ajustes y la barra lateral.
import { providerStatus, type ProviderStatus } from "./backend";

export const connections = $state<{ list: ProviderStatus[]; loaded: boolean }>({ list: [], loaded: false });

export async function refreshConnections() {
  try {
    connections.list = await providerStatus();
  } catch {
    connections.list = [];
  }
  connections.loaded = true;
}

export function statusOf(id: string): ProviderStatus | undefined {
  return connections.list.find((p) => p.id === id);
}
