# Product

## Register

product

## Users

Cualquier persona que quiera trabajar con varias IAs a la vez, no solo desarrolladores. Incluye gente sin experiencia técnica, que no sabe qué es un "modo de permisos" o una "ventana de contexto" y aun así necesita decidir sobre ellos. El repositorio es público, así que la primera impresión la tiene un desconocido, no el autor.

El trabajo es: abrir un proyecto, hablar con una o varias IAs sobre archivos reales, y que el trabajo no se corte cuando una IA se queda sin uso, porque otra retoma donde quedó.

## Product Purpose

CoModel es un espacio de trabajo de escritorio (Linux primero) donde varias IAs (Claude Code, Codex, OpenRouter, Gemini y otras) comparten memoria y se relevan solas al llegar a su límite. Éxito: el usuario deja de pensar en qué IA le queda uso y se concentra en su proyecto.

## Brand Personality

Sobria, precisa, de confianza. Una herramienta hecha con cuidado, no una demo. Habla claro y no presume de inteligencia artificial: la IA es el material, no el decorado.

Referencia de estructura: Morphy (lienzo infinito con tarjetas de chat arrastrables, barra lateral de proyectos, uso de la IA en la esquina inferior, selector de modelo dentro de cada tarjeta). Se toma la estructura, no su estilo: CoModel tiene identidad propia.

## Anti-references

- Neón morado o azul sobre negro con brillos y degradados: el look típico de "app de IA".
- Emojis e iconos decorativos. Un icono se usa cuando comunica solo y reemplaza texto con claridad (engranaje, enviar, chat nuevo); nunca como adorno.
- Lo que se nota "vibecoded": cero esfuerzo, valores por defecto del framework, el mismo espaciado en todo, cada cosa en una tarjeta redondeada.
- El chat genérico tipo ChatGPT (burbujas, avatares, mucho aire, sin información técnica).

## Design Principles

1. **Lenguaje claro primero, detalle técnico a un clic.** Etiquetas en palabras normales ("Cuánto puede recordar", no "ventana de contexto"), con el término técnico disponible para quien lo quiera.
2. **El estado de las IAs siempre visible, nunca ruidoso.** Quién responde, cuánto uso queda y cuándo hubo un relevo se leen de un vistazo, sin banners ni colores chillones.
3. **El contenido manda, no la caja.** Menos contenedores y sombras; jerarquía con tipografía, espaciado y alineación.
4. **Detalle de oficio.** Estados completos (hover, foco, deshabilitado, cargando, error), alineación fina, números tabulares. Lo que distingue una herramienta hecha a mano.
5. **Tema como sistema.** Todo el color sale de tokens; cambiar de tema nunca rompe un componente.
6. **Icono si se entiende solo, texto si hace falta.** Lo que un icono comunica perfecto se deja como icono, con etiqueta accesible y tooltip.
7. **Todo lo que puede moverse, se mueve con suavidad.** Paneles, popups, menús, tarjetas y listas entran y salen con animaciones apropiadas (ver `DESIGN.md`), y el usuario puede apagarlas todas con "Reducir movimiento".

## Accessibility & Inclusion

- Contraste de texto WCAG AA como mínimo en todos los temas.
- Opción "Reducir movimiento" en Ajustes que apaga todas las animaciones; sin elección, respeta `prefers-reduced-motion` del sistema.
- Tamaño de texto escalable desde configuración; la interfaz debe aguantar el cambio sin romperse.
- Foco de teclado visible en todo control.
