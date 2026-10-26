// Movimiento compartido. Todo lo que aparece o desaparece pasa por aquí para que el estilo sea uno solo
// y "Reducir movimiento" (Ajustes) lo apague de una vez.
import { quartIn, quartOut } from "svelte/easing";
import { reducedMotion } from "./theme";

/** Duración en ms: cero si el movimiento está reducido. */
export const ms = (n: number) => (reducedMotion() ? 0 : n);

interface Params {
  y?: number;
  x?: number;
  duration?: number;
}

/** Aparece desplazándose un poco desde un lado. Para mensajes, filas y paneles pequeños. */
export function rise(_: Element, { y = 6, x = 0, duration = 180 }: Params = {}) {
  return {
    duration: ms(duration),
    easing: quartOut,
    css: (t: number) => `opacity: ${t}; transform: translate(${(1 - t) * x}px, ${(1 - t) * y}px)`,
  };
}

/** Menús flotantes: entran con un leve acercamiento. */
export function popIn(_: Element, { y = 6, duration = 150 }: Params = {}) {
  return {
    duration: ms(duration),
    easing: quartOut,
    css: (t: number) => `opacity: ${t}; transform: translateY(${(1 - t) * y}px) scale(${0.97 + 0.03 * t})`,
  };
}

/** Salida más corta y con aceleración que la entrada. */
export function popOut(_: Element, { y = 4, duration = 100 }: Params = {}) {
  return {
    duration: ms(duration),
    easing: quartIn,
    css: (t: number) => `opacity: ${t}; transform: translateY(${(1 - t) * y}px) scale(${0.98 + 0.02 * t})`,
  };
}

/** Tarjetas del lienzo. */
export function cardIn(_: Element, { duration = 220 }: Params = {}) {
  return {
    duration: ms(duration),
    easing: quartOut,
    css: (t: number) => `opacity: ${t}; transform: scale(${0.96 + 0.04 * t})`,
  };
}

export function cardOut(_: Element, { duration = 140 }: Params = {}) {
  return {
    duration: ms(duration),
    easing: quartIn,
    css: (t: number) => `opacity: ${t}; transform: scale(${0.97 + 0.03 * t})`,
  };
}
