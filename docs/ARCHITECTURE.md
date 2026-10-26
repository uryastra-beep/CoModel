# CoModel: arquitectura

## Qué es

App de escritorio (Linux primero) con un lienzo donde se abren chats con varias IAs que comparten memoria. Si una IA se queda sin uso, la siguiente de la lista sigue donde quedó.

## Stack

| Capa | Elección | Motivo |
| --- | --- | --- |
| Shell | Tauri 2 (Rust) | Ligero, nativo en Linux, buen acceso a procesos y llavero |
| UI | Svelte 5 + TypeScript + Vite | Reactivo y pequeño, ideal para paneles que cambian mucho |
| Memoria estructurada | SQLite (desde Rust) | Mensajes, sesiones, relevos |
| Memoria legible | `MEMORY.md` por proyecto | La leen las IAs y el usuario |
| Secretos | Llavero del sistema (Secret Service, crate `keyring`) | El repo es público: nada de claves en archivos |

## Tema

- Oscuro estilo GitHub, color base morado oscuro.
- Todo el color sale de variables CSS (tokens), así el tema se cambia desde configuración sin tocar componentes.

## Proyecto

Un proyecto es:

- una carpeta de trabajo,
- una lista de proveedores conectados (cada uno por API key, por CLI, o ambos),
- el orden de relevo definido por el usuario (ej. Claude, Codex, OpenRouter, Gemini),
- su memoria.

Los proveedores se eligen al crear el proyecto y se editan en la configuración del proyecto.

## Proveedores

Cada proveedor implementa la misma interfaz (adaptador):

- `start(session)` / `send(message)` / `stop()`
- eventos normalizados: texto, uso de herramienta, archivo editado, petición de permiso o de input, error, límite alcanzado
- `capabilities()`: modos, modelos, niveles de esfuerzo, ventana de contexto y uso restante (lo que el proveedor soporte)

Tipos de adaptador:

- **CLI** (Claude Code, Codex): CoModel lanza el proceso y lee su salida en streaming.
- **API** (OpenRouter, Gemini): peticiones HTTP directas.

Cuando un agente necesita un permiso o un dato del usuario, la petición aparece en su mismo panel de chat.

## Memoria compartida

Dos capas:

1. **SQLite:** registro completo y fiable de todo lo que pasa. Lo escribe CoModel, a partir de los eventos que ve.
2. **`MEMORY.md` del proyecto** (en `.comodel/MEMORY.md` dentro de la carpeta): resumen legible con diffs, resúmenes y pendientes.

En el primer mensaje de cada chat, CoModel antepone una instrucción a la IA: documentar sus cambios (diff y resumen) en `MEMORY.md` y leerlo al empezar.

Importante: no dependemos solo de que la IA lo cumpla. CoModel también registra por su cuenta lo que observa (mensajes, herramientas usadas, archivos tocados), así el relevo funciona aunque una IA olvide escribir.

## Relevo automático

1. El adaptador detecta un límite (error 429, mensaje de "usage limit", o ventana de contexto llena).
2. CoModel marca ese proveedor como agotado.
3. Elige el siguiente disponible en el orden del usuario.
4. Le entrega un paquete de relevo:
   - historial reciente,
   - resumen de la sesión,
   - archivos tocados (con diffs),
   - lo que quedó pendiente.
5. El nuevo modelo continúa en el mismo chat y se avisa al usuario del cambio.

## Cabecera de cada panel de chat

Igual que en las herramientas oficiales:

- modo (auto, plan, aceptar manualmente),
- modelo,
- esfuerzo,
- ventana de contexto y uso restante.

No todos los proveedores exponen todos los datos. Cada adaptador declara qué soporta y la cabecera muestra solo eso. A verificar durante la prueba técnica.

## Lienzo

Empieza vacío. Desde él se abren paneles: chats de IA, selector de archivos, visor y editor de código.

## Calidad

- Repo git, licencia, README, esta documentación.
- Tests del núcleo, sobre todo del relevo.
- Errores claros cuando un proveedor falla.
- CI que compile y pruebe.
- Interfaz preparada para varios idiomas desde el inicio (español por defecto).
