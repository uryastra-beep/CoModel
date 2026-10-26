// Copiar texto y abrir enlaces fuera de la app.
import { inTauri } from "./backend";

export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    // Sin permiso para el portapapeles: el método antiguo sigue funcionando en casi todos los webviews.
    const area = document.createElement("textarea");
    area.value = text;
    area.style.position = "fixed";
    area.style.opacity = "0";
    document.body.appendChild(area);
    area.select();
    const ok = document.execCommand("copy");
    area.remove();
    return ok;
  }
}

/** Abre un enlace en el navegador del sistema, nunca dentro de la ventana de la app. */
export async function openExternal(href: string) {
  if (!/^(https?:\/\/|mailto:)/i.test(href)) return;
  if (inTauri) {
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    await openUrl(href);
  } else window.open(href, "_blank", "noopener");
}
