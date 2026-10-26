# CoModel

App de productividad con modelos de IA.

## Idea

Un espacio de trabajo vacío donde puedes abrir varias IAs a la vez:

- Codex
- Claude Code
- OpenRouter
- Otras IAs

Además:

- Selector de archivos integrado.
- Paneles de código.
- Memoria compartida: todas las IAs ven el mismo contexto.
- Continuidad entre modelos: si un modelo llega al límite de uso, otro conectado retoma desde donde quedó el anterior.

## Plataforma

- App de escritorio. Linux es la plataforma principal (ahí se desarrolla por ahora).

## Conexión a cada IA

- Cada proveedor se puede conectar con API key, con CLI (Claude Code, Codex), o con ambas.
- Se elige al crear un proyecto y se puede cambiar después desde la configuración del proyecto.

## Memoria compartida

- Doble: archivo local y base de datos.

## Relevo al límite

- Cuando un modelo se queda sin uso, la conversación pasa al siguiente disponible desde donde quedó.
- El usuario define el orden de relevo, por ejemplo:
  1. Claude
  2. Codex
  3. OpenRouter
  4. Gemini

## Estado

App de escritorio (Tauri 2 + Svelte 5) con núcleo en Rust (`core/`). Lienzo con tarjetas de chat y de código, carpeta opcional por chat, varias IAs con relevo automático y memoria compartida, permisos y preguntas dentro del chat, adjuntos, árbol de archivos con editor y diffs, y pantalla de uso. Detalle en `docs/ROADMAP.md`.

## Cuenta

Opcional: correo, Google o GitHub (ver [docs/ACCOUNTS.md](docs/ACCOUNTS.md)). Sin cuenta todo funciona en local.

## Desarrollo

```bash
npm install
npm run tauri dev
```

## Decisiones pendientes

- Framework de UI para los paneles (ahora es TypeScript sin framework).
- Cómo se detecta el límite de uso de cada proveedor.
- Formato del archivo local de memoria y esquema de la base de datos.
