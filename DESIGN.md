# Design

Sistema visual de CoModel. Los valores viven en `src/styles/tokens.css`; este documento explica las decisiones.

## Escena

Una persona trabaja varias horas, por la tarde, en un portátil con la luz de la habitación baja, repartiendo trabajo entre varias IAs. Es casual o desarrolladora: la interfaz debe parecer una herramienta seria, no un producto de IA. Por eso el tema por defecto es oscuro y casi gris, pero hay un tema claro y texto escalable.

## Color

Estrategia **Restringida**: neutros con un matiz apenas perceptible (matiz 285, croma 0.005 a 0.009 en Grafito) y un solo acento, usado para acciones principales, selección y foco. Sin degradados, sin brillos. Texto sobre superficie con contraste de al menos 4.5:1 en los tres temas.

- `--bg` lienzo, `--surface` barras y tarjetas, `--raised` bloques del usuario y controles, `--raised-2` hover.
- `--text`, `--text-2`, `--text-3` (el más tenue sigue siendo legible).
- `--accent` para texto y foco, `--accent-solid` para botones.
- Semánticos `--ok`, `--warn`, `--danger`, solo para estado (uso alto, relevo, error).

- `--accent-tint` fondo de selección, `--line` y `--line-soft` bordes, `--scrim` fondo de diálogos, `--shadow-card` y `--lift` las dos únicas sombras.
- Identidad de cada IA (`--p-claude`, `--p-codex`, `--p-openrouter`, `--p-gemini`): solo en marcas pequeñas, como el punto `.pdot`. Nunca en texto ni en fondos grandes.
- Colores de sintaxis `--syn-*` para el código.

Temas (`data-theme` en `<html>`, elección en `comodel.theme.v2`):
- **Grafito** (por defecto): neutros casi grises. El violeta del logo queda para la acción principal, la selección y el foco.
- **Violeta**: los mismos tonos teñidos de violeta, para quien quiere el color de la marca en toda la interfaz.
- **Papel**: tema claro.

## Tipografía

Una sola familia, Geist, incluida en la app (funciona sin red), y Geist Mono para código y rutas. Escala cerrada en rem: `--fs-xs` 12, `--fs-sm` 13, `--fs-md` 15, `--fs-lg` 18, `--fs-xl` 24 y `--fs-2xl` 36 px a tamaño normal; el usuario puede subir el texto hasta 130% y toda la interfaz escala, incluidas las tarjetas.

Radios: `--radius-sm` 4, `--radius` 6, `--radius-lg` 10, `--radius-xl` 14 px. Alturas de control: `--control-sm` 26, `--control` 32, `--control-lg` 38 px.

## Estructura

- Barra lateral plegable (Ctrl+B) con proyectos, chats, y al pie el perfil (foto o inicial) y Ajustes. Barra superior con una sola acción primaria, lienzo infinito con controles de zoom flotantes, barra de estado.
- Inicio (`Home`): saludo grande centrado y, debajo, el cuadro para escribir. Sin tarjetas decorativas.
- Paleta de comandos (Ctrl+K) para ir a cualquier chat, proyecto o ajuste.
- Ajustes (Ctrl+,) en un diálogo con secciones: Perfil, Proyecto, IAs, Uso, Apariencia. Cada página usa las filas `.s-row` (etiqueta y explicación a la izquierda, control a la derecha).
- Las tarjetas de chat son las únicas "cajas": borde de 1 px, sin sombra salvo al arrastrar. Dentro no hay más tarjetas ni burbujas.
- El texto del chat es una transcripción: etiqueta pequeña ("Tú", "Claude"), texto a 15 px, el turno del usuario sobre una banda tenue.
- **Selector de IA por chat** (`AIPicker`): primero la IA (Claude, Codex, ChatGPT, OpenRouter) y, según cuál sea, su modelo, esfuerzo y modo. Cada IA da su propia lista y recuerda lo último que se usó con ella. Una IA sin conectar no se elige: lleva a Ajustes.
- Comandos con barra en el chat: `/compact`, `/clear`, `/memory`, `/help`.
- Estado de las IAs: uso (5 h, 7 d) en la barra de estado, contexto junto al botón de enviar, relevos como una línea divisoria en la transcripción.

## Controles comunes

Un botón, un campo o un interruptor se ven igual en toda la app. Los componentes usan las clases de `src/styles/controls.css` en lugar de definir las suyas: `.btn` (con `.sm`, `.lg`, `.primary`, `.ghost`, `.danger`), `.icon-btn`, `.link`, `.input`, `.switch`, `.pdot`, `.activity`, `.popover` y las `.s-*` de los ajustes. Menús (`Menu`, `ActionMenu`), diálogos (`Dialog`) y el avatar (`Avatar`) son componentes compartidos; un control nuevo se añade ahí, no dentro de una pantalla.

## Iconos

**Regla del proyecto: lo que se pueda convertir en icono y se entienda perfecto, se deja como icono.** Engranaje para Ajustes, hoja con lápiz para chat nuevo, flecha hacia arriba para enviar, esquinas para ajustar la vista, "×" para cerrar. Los textos se quedan donde el icono no basta o donde el texto enseña (el botón grande del estado vacío lleva icono y texto).

- Un solo estilo: trazo de 1.75 en una rejilla de 24, extremos redondeados, color del texto (`currentColor`). Componente `Icon.svelte`, basado en Lucide (ISC).
- Todo botón de icono lleva `aria-label` y `title` (con el atajo si lo tiene), y un área de pulsación de al menos 2 rem.
- Un icono nuevo solo se añade si comunica solo. Nunca como adorno.

## Lenguaje

Español llano, sin guiones largos. Palabras llanas ("Contexto", "Esfuerzo", "Aceptar manualmente") con una explicación en cada opción de menú. Las herramientas se muestran como acciones ("Leyó un archivo", "Cambió src/app.ts").

## Movimiento

**Regla del proyecto: todo lo que pueda animarse lleva una animación apropiada y suave.** Cada panel, popup, menú, tarjeta, fila, mensaje o estado que aparece, desaparece, se despliega o cambia se anima. Un cambio instantáneo es un defecto, salvo que haya una razón para que lo sea (por ejemplo, el arrastre, que debe seguir al puntero sin curva).

Cómo:
- **Entrar tarda más que salir.** Entrada 200 a 280 ms con curva de desaceleración exponencial (`--ease`); salida 100 a 150 ms con curva de aceleración (`--ease-in`). Tokens: `--dur-fast`, `--dur-base`, `--dur-slow`, `--dur-out`.
- **El movimiento es pequeño y con propósito:** un desplazamiento de 4 a 8 px o una escala de 0.96 a 1, con opacidad. Sin rebotes ni elásticos.
- **Cortina** (`Curtain.svelte`) para todo lo que se pliega y despliega: la altura se anima con la fila de la rejilla (0fr a 1fr) y el contenido baja un poco al abrir. Cerrada, es inerte. Es la excepción deliberada a "no animar propiedades de layout", pedida por el autor.
- **Popups y diálogos:** escala y desplazamiento leves al entrar, fundido del fondo, y salida propia antes de cerrar de verdad.
- **Cámara del lienzo:** centrar un chat, ajustar la vista y los botones de zoom se mueven con curva; la rueda, el pellizco y el arrastre siguen al puntero sin curva.
- **Listas y transcripción:** lo nuevo entra con un leve desplazamiento; lo que se va, se funde.
- **Un solo origen:** los movimientos de Svelte salen de `src/lib/motion.ts` y los de CSS de los tokens, para que el estilo sea uno y se apague de una vez.

**Reducir movimiento** (Ajustes, Apariencia): quita todas las animaciones. Sin elección guardada sigue la opción del sistema. Todo lo nuevo debe respetarlo: en CSS ya queda cubierto por `[data-motion="reduced"]`; en JavaScript hay que usar `ms()` de `motion.ts` o `reducedMotion()` de `theme.ts`.

Al añadir cualquier elemento que aparezca o cambie, la animación se diseña con él, no después.

## Prohibido en este proyecto

Emojis o iconos decorativos (los iconos son funcionales), texto con degradado, bordes laterales de color, tarjetas dentro de tarjetas, rejillas de tarjetas idénticas, modales para lo que cabe en línea.
