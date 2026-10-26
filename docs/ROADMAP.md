# CoModel: hoja de ruta

## Criterio de v1 terminada

Mínimo: **2 IAs con memoria compartida** y **relevo automático** entre ellas.

## Fase 0: base (hecha)

- Esqueleto Tauri 2 con lienzo vacío.
- README y documentación de arquitectura.

## Fase 1: prueba técnica del núcleo (hecha)

Resultados en [PROVIDER-FINDINGS.md](PROVIDER-FINDINGS.md). Falta probar OpenRouter con una clave real.

Sin UI bonita. Objetivo: demostrar el relevo.

- Adaptador de Claude Code (CLI) y adaptador de OpenRouter (API).
- Memoria en SQLite y `MEMORY.md`.
- Simular un límite en el primer proveedor y comprobar que el segundo continúa donde quedó.
- Verificar qué datos exponen los proveedores (modos, modelo, esfuerzo, contexto, uso restante).

## Fase 2: UI base (hecha, falta probar la app real)

Hecho:
- Svelte 5, tipografía Geist incluida en la app, tokens en OKLCH y tres temas.
- Lienzo con desplazamiento (rueda), zoom (Ctrl + rueda o barra de estado), tarjetas de chat arrastrables y redimensionables, doble clic para abrir un chat, tecla `N`.
- Cada chat con su historial y la memoria del proyecto compartida (núcleo y Tauri).
- Cabecera de uso en la barra de estado, menús de modelo, modo y esfuerzo con explicaciones, markdown en las respuestas.
- Accesibilidad: texto escalable, foco visible, teclado en menús y tarjetas, respeta "reducir movimiento".

Pendiente: peticiones de permiso dentro del tarjeta, cabecera según el proveedor activo, persistir la disposición del lienzo, minimapa.

## Fase 3: proyectos (hecha)

- La carpeta es **opcional por chat**, como en la app de Claude: debajo del cuadro de texto, "Proyecto o carpeta". Sin carpeta la IA solo conversa (sin herramientas); al elegir una con el selector nativo, la carpeta pasa a ser un proyecto con el nombre de la carpeta.
- Cada proyecto tiene sus IAs y su orden de relevo (Ajustes, Proyectos); los chats sin carpeta usan el orden por defecto (Ajustes, IAs).
- La memoria de cada proyecto vive en su carpeta (`.comodel/MEMORY.md`; la base de datos se excluye de git sola).
- El lienzo, los nombres, la carpeta de cada chat y las conversaciones se recuerdan al reabrir.

## Pantalla de inicio (hecha)

Los proyectos y los chats viven en la barra lateral (pestañas Chats, Proyectos y Archivos); Inicio no los repite.

Es la que se ve al abrir la app (y con el botón de la casa, arriba a la izquierda): saludo según la hora, una caja para empezar un chat al instante con su carpeta opcional y una guía de "Primeros pasos" que se oculta sola al completarla (o con "Ocultar").

## Fase 4: permisos, adjuntos y archivos (hecha, falta probar la app real)

- **Permisos y preguntas en el chat:** Claude Code se lanza en modo `stream-json` con `--permission-prompt-tool stdio`. Cada permiso (editar, crear, ejecutar un comando) aparece en la tarjeta con su diff y se responde ahí; también las preguntas de la IA. El archivo de memoria del propio proyecto se permite solo. Se puede parar un mensaje en curso.
- **Adjuntos:** botón "+", arrastrar archivos y pegar imágenes. Los de fuera del proyecto se copian a `.comodel/attachments/`. Una IA sin acceso a archivos recibe el contenido de los de texto dentro del mensaje.
- **Archivos y código:** pestaña Archivos en la barra lateral (árbol por carpetas con marcas de git), tarjetas de código con CodeMirror (resaltado, búsqueda, deshacer), guardar con Ctrl+S, aviso si el archivo cambió en disco, vista de cambios contra el último commit de git y diff de cada edición de la IA en el chat.
- Seguridad: toda ruta queda confinada a la carpeta del proyecto (con `..` o enlaces simbólicos no se puede salir; hay tests).

## Cuentas (hecha, falta configurar Supabase y probar)

Inicio de sesión opcional con correo, Google y GitHub sobre Supabase Auth; sesión en el llavero. Pasos en [ACCOUNTS.md](ACCOUNTS.md). Pendiente: sincronizar ajustes entre equipos.

## Fase 5: más proveedores y pulido

- Codex, Gemini y otros.
- Configuración de tema, idioma, empaquetado para Linux.

## Después de la v1

- Windows y macOS.
