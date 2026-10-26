// Arrastre por puntero compartido por todas las tarjetas del lienzo (mover y cambiar de tamaño).

/** Empieza a seguir el puntero desde `e`; `apply` recibe el desplazamiento ya corregido por el zoom. */
export function trackPointer(e: PointerEvent, scale: number, onstart: () => void, apply: (dx: number, dy: number) => void, dragging: (on: boolean) => void) {
  if (e.button !== 0) return;
  onstart();
  const el = e.currentTarget as HTMLElement;
  el.setPointerCapture(e.pointerId);
  let lx = e.clientX;
  let ly = e.clientY;
  dragging(true);
  const move = (ev: PointerEvent) => {
    apply((ev.clientX - lx) / scale, (ev.clientY - ly) / scale);
    lx = ev.clientX;
    ly = ev.clientY;
  };
  const up = () => {
    dragging(false);
    el.removeEventListener("pointermove", move);
    el.removeEventListener("pointerup", up);
    el.removeEventListener("pointercancel", up);
  };
  el.addEventListener("pointermove", move);
  el.addEventListener("pointerup", up);
  el.addEventListener("pointercancel", up);
}
