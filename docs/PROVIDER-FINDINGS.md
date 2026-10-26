# Hallazgos de la prueba técnica (Fase 1)

Código en `core/`. Pruebas: `cargo test --manifest-path core/Cargo.toml` (9 pruebas).
Demo real: `cargo run --manifest-path core/Cargo.toml --bin spike -- demo`.

## Qué quedó demostrado

- **Relevo automático con contexto:** un Claude real (A) "se agota" en su 2.ª petición; un segundo Claude real (B) responde la pregunta usando solo el paquete de relevo e historial que le pasó CoModel. B contestó correctamente el nombre del usuario y de la app.
- **La IA sí mantiene `MEMORY.md`** cuando CoModel se lo pide en el preámbulo: Claude Code escribió Resumen, Pendiente y Cambios en el archivo del proyecto sin que se le dijera cómo.
- **El estado es de CoModel:** cada turno es independiente (el CLI se lanza sin persistencia de sesión) y CoModel reenvía historial y preámbulo. Así cualquier proveedor puede retomar.

## Claude Code (CLI)

Se lanza con `claude -p --output-format stream-json --verbose`; el prompt entra por stdin.

| Dato de la cabecera | ¿Disponible? | De dónde sale |
| --- | --- | --- |
| Modelo | Sí | evento `system/init` |
| Modo de permisos | Sí | `system/init` (`permissionMode`); se fija con `--permission-mode` |
| Esfuerzo | Se fija, no se lee | `--effort low\|medium\|high\|xhigh\|max` |
| Ventana de contexto | Sí | `result.modelUsage.<modelo>.contextWindow` (1.000.000 en la prueba) |
| Tokens usados y coste | Sí | `result.usage`, `total_cost_usd` |
| Uso restante | Sí | evento `rate_limit_event`: ventanas `five_hour` y `seven_day` con `utilization` (0 a 1) y `resetsAt` |
| Archivos tocados | Parcial | herramientas Edit/Write/MultiEdit/NotebookEdit. **No** se ven los archivos creados con Bash |

### Hallazgo crítico: aislamiento

Lanzado sin más, Claude Code cargó todos los plugins, MCPs y skills del usuario: **292.432 tokens y $0,29 por responder "ok"**. Con `--strict-mcp-config --disable-slash-commands --setting-sources project` bajó a ~3.300 tokens y $0,0007. El adaptador usa siempre estas banderas. `--bare` no sirve: desactiva el inicio de sesión (OAuth/llavero) y exige API key.

### Pendiente de verificar

- **Formato exacto de un bloqueo por límite.** Solo se observó `status: "allowed"`. El adaptador trata cualquier otro estado, un `429` o un texto "usage limit" como límite, pero no se ha visto un bloqueo real. Se verificará la primera vez que ocurra.
- **Peticiones de permiso/input en vivo: resuelto.** Ver la sección siguiente.
- **Archivos creados por Bash:** hace falta complementar con un diff del sistema de archivos o con `git`.

## OpenRouter (API)

Verificado contra un servidor falso (sin clave real, ver abajo): streaming SSE, texto, tokens, coste y créditos restantes (`GET /key`).

- `429` y `402` (sin créditos) se tratan como límite y disparan el relevo. Otros errores HTTP no.
- Las líneas de comentario del stream (`: OPENROUTER PROCESSING`) deben ignorarse. Un test lo cubre (era un bug).
- La ventana de contexto no viene en el stream: hay que pedirla a `/models`. Pendiente.
- Esfuerzo: se envía como `reasoning.effort`. Modo: no aplica a una API sin herramientas.

**Sin probar con la API real:** no hay `OPENROUTER_API_KEY` en el entorno. Para probar: `OPENROUTER_API_KEY=... cargo run --manifest-path core/Cargo.toml --bin spike -- openrouter "hola"`.

## Decisiones de diseño que salieron de la prueba

- Cada turno es sin estado: se pierde la caché de prompt de Claude entre turnos. Optimización posible: reanudar la sesión (`--resume`) mientras el mismo proveedor siga activo.
- Un proveedor agotado se queda agotado hasta `reset_exhausted()`. Falta usar `resets_at` para volver a él automáticamente.
- El resumen del relevo lo escribe la IA en `MEMORY.md`; si no lo hizo, CoModel genera uno mínimo. Un resumen mejor requeriría llamar a un modelo barato.

## Permisos y preguntas (protocolo verificado)

Con `--input-format stream-json --permission-prompt-tool stdio`, el CLI pide permisos por stdout y espera la respuesta por stdin. Sin `--permission-prompt-tool` rechaza solo (evento `permission_denied`).

Petición (stdout):

```json
{"type":"control_request","request_id":"…","request":{"subtype":"can_use_tool","tool_name":"Write","input":{"file_path":"…","content":"hola"},"description":"hola.txt","permission_suggestions":[{"type":"setMode","mode":"acceptEdits","destination":"session"}],"tool_use_id":"…"}}
```

Respuesta (stdin):

```json
{"type":"control_response","response":{"subtype":"success","request_id":"…","response":{"behavior":"allow","updatedInput":{…}}}}
```

- **Rechazar:** `{"behavior":"deny","message":"…"}`. La IA lo ve y se adapta (comprobado).
- **Permitir todas las ediciones de la sesión:** añadir `"updatedPermissions":[{"type":"setMode","mode":"acceptEdits","destination":"session"}]` (sin comprobar con el CLI real todavía).
- **Preguntas de la IA:** llegan como `can_use_tool` con `tool_name: "AskUserQuestion"` y `input.questions` (pregunta, cabecera, opciones, selección múltiple). Se responde con `allow` y `updatedInput` igual al original más `answers`: texto de la pregunta → opción elegida (comprobado).
- El mensaje del usuario entra como `{"type":"user","message":{"role":"user","content":"…"}}` y hay que cerrar stdin tras el resultado para que el CLI termine.
- Pedir escribir en `.comodel/MEMORY.md` también genera permiso; CoModel lo permite solo.
