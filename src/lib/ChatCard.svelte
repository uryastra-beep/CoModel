<script lang="ts">
  import { onMount, tick } from "svelte";
  import ActionMenu, { type Action } from "./ActionMenu.svelte";
  import AIPicker from "./AIPicker.svelte";
  import { defaultsFor, effortsFor, modelLabel, modesFor, providerLabel as aiLabel } from "./ai.svelte";
  import { answerInteraction, chatHistory, clearChat, onChatEvent, pickFiles, savePastedImage, sendMessage, stopMessage, type Answer, type OutFile } from "./backend";
  import type { Card } from "./canvas.svelte";
  import { bumpFiles, canvas, ensureChatWorkspace, centerOn, openFileCard, setCardAI } from "./canvas.svelte";
  import { trackPointer } from "./cardDrag";
  import { copyText } from "./clipboard";
  import DiffView from "./DiffView.svelte";
  import { countOf, isEditTool, toolDiff, type Row } from "./diff";
  import FolderMenu from "./FolderMenu.svelte";
  import Icon, { type IconName } from "./Icon.svelte";
  import Markdown from "./Markdown.svelte";
  import ChatImage from "./ChatImage.svelte";
  import ChatFiles from "./ChatFiles.svelte";
  import { autoTitle, linkedChats } from "./canvas.svelte";
  import { cardIn, cardOut, ms, popIn, popOut, rise } from "./motion";
  import { fade } from "svelte/transition";
  import PermissionBlock, { type PermissionItem } from "./PermissionBlock.svelte";
  import QuestionBlock, { type QuestionItem } from "./QuestionBlock.svelte";
  import { formatTokens, providerLabel, toolLabel } from "./providers";
  import { projectById } from "./projects.svelte";
  import { ui } from "./ui.svelte";
  import { recordHandoff, recordQuota } from "./usage.svelte";

  interface Props {
    card: Card;
    /** Zoom actual del lienzo, para que el arrastre siga al puntero. */
    scale: number;
    focused: boolean;
    onfocus: () => void;
    onclose: () => void;
    onmove: (dx: number, dy: number) => void;
    onresize: (dw: number, dh: number) => void;
    onrename: (title: string) => void;
    onbusy: (busy: boolean) => void;
    onproject: (projectId: string | null) => void;
  }
  let { card, scale, focused, onfocus, onclose, onmove, onresize, onrename, onbusy, onproject }: Props = $props();

  type Item =
    | { kind: "user"; text: string; files?: Attached[] }
    | { kind: "assistant"; provider: string; model?: string | null; text: string; files?: OutFile[] }
    | { kind: "log"; text: string; icon?: IconName }
    | { kind: "change"; path: string; rows: Row[]; created: boolean; memory: boolean }
    | ({ kind: "permission" } & PermissionItem)
    | ({ kind: "question" } & QuestionItem)
    | { kind: "handoff"; from: string; to: string; reason: string }
    | { kind: "error"; text: string };

  // La IA del chat. Un chat antiguo (sin elección guardada) empieza con la última que se usó.
  onMount(() => {
    if (!card.ai) setCardAI(card.id, defaultsFor());
  });
  const ai = $derived(card.ai ?? defaultsFor());
  const hasFolder = $derived(!!card.projectId);
  const starters = $derived(
    card.projectId
      ? ["Resume este proyecto y lo que falta", "¿Qué cambios hay pendientes?", "Revisa el código y propón mejoras"]
      : ["Ayúdame a escribir un texto", "Explícame un concepto paso a paso", "Dame ideas para un proyecto"],
  );

  interface Attached {
    name: string;
    path: string;
    /** Miniatura, solo para imágenes pegadas. */
    thumb?: string;
  }

  let items = $state<Item[]>([]);
  let attached = $state<Attached[]>([]);
  let attachError = $state("");
  let draft = $state("");
  let busy = $state(false);
  let elapsed = $state(0);
  let dragging = $state(false);
  let renaming = $state(false);
  let activeModel = $state<string | null>(null);
  /** Último modelo que anunció cada IA en este chat. */
  const lastModel: Record<string, string | null> = {};
  let context = $state<{ used: number; window: number | null } | null>(null);
  let sel = $state(0);
  let menuDismissed = $state(false);
  let copied = $state<number | null>(null);
  let scroller: HTMLDivElement;
  let input: HTMLTextAreaElement;

  /** Ruta de un archivo relativa a la carpeta del chat, si está dentro de ella. */
  function relPath(abs: string): string | null {
    const root = projectById(card.projectId)?.path;
    return root && abs.startsWith(`${root}/`) ? abs.slice(root.length + 1) : null;
  }

  // ---------- Comandos con "/" ----------

  interface Command {
    name: string;
    /** Lo que se puede escribir después del comando. */
    arg?: string;
    desc: string;
  }
  const COMMANDS: Command[] = [
    { name: "compact", arg: "enfoque", desc: "Resume la conversación para liberar memoria de trabajo" },
    { name: "clear", desc: "Empieza de cero; el historial se conserva" },
    { name: "memory", desc: "Abre la memoria compartida del proyecto" },
    { name: "help", desc: "Muestra los comandos" },
  ];

  const slashQuery = $derived(/^\/([a-z]*)$/i.exec(draft)?.[1].toLowerCase() ?? null);
  const matches = $derived(slashQuery === null ? [] : COMMANDS.filter((c) => c.name.startsWith(slashQuery)));
  const menuOpen = $derived(!busy && !menuDismissed && matches.length > 0);
  $effect(() => {
    void slashQuery;
    sel = 0;
    menuDismissed = false;
  });

  const note = (text: string) => items.push({ kind: "log", text, icon: "right" });

  /** Ejecuta un comando de la barra. Devuelve falso si el texto no es un comando y debe enviarse a la IA. */
  async function runCommand(text: string): Promise<boolean> {
    const token = text.split(/\s/)[0];
    if (!/^\/[a-z]+$/i.test(token)) return false; // una ruta como /home/ury se envía normal
    const name = token.slice(1).toLowerCase();
    const arg = text.slice(token.length).trim();
    const known = COMMANDS.find((c) => c.name === name);
    draft = "";
    resize();
    if (!known) {
      items.push({ kind: "error", text: `No existe el comando /${name}. Escribe /help para ver los que hay.` });
    } else if (name === "help") {
      for (const c of COMMANDS) note(`/${c.name}${c.arg ? ` [${c.arg}]` : ""}: ${c.desc}`);
    } else if (name === "memory") {
      openMemory();
    } else if (name === "clear") {
      await clearContext();
    } else {
      await compact(arg);
    }
    scrollDown();
    return true;
  }

  function openMemory() {
    if (card.projectId) openFileCard(card.projectId, ".comodel/MEMORY.md");
    else note("Sin carpeta no hay memoria compartida. Elige una abajo.");
  }

  async function clearContext() {
    if (canvas.projectSync) return;
    try {
      await clearChat(card.projectId, card.id);
      context = null;
      items = [{ kind: "log", text: "Conversación nueva. Lo anterior ya no cuenta como contexto.", icon: "refresh" }];
    } catch (e) {
      items.push({ kind: "error", text: String(e) });
    }
  }

  async function compact(focus: string) {
    if (canvas.projectSync) return;
    if (!items.some((i) => i.kind === "user" || i.kind === "assistant")) {
      note("Todavía no hay nada que resumir.");
      return;
    }
    try { await ensureChatWorkspace(card.id); }
    catch (e) { canvas.projectError = String(e); return; }
    busy = true;
    onbusy(true);
    note("Resumiendo la conversación");
    const before = items.length;
    try {
      const summary = await sendMessage(card.projectId, card.id, focus, { ...turnOptions(), attachments: [], compact: true });
      const provider = (items.slice(before).reverse().find((i) => i.kind === "assistant") as { provider: string } | undefined)?.provider ?? ai.provider;
      items = [
        { kind: "log", text: "Conversación resumida. Esto es lo que recordará la IA:", icon: "memory" },
        { kind: "assistant", provider, text: summary },
      ];
      context = null;
    } catch (e) {
      const msg = String(e);
      if (msg === "Detenido.") note("Detuviste el resumen.");
      else items.push({ kind: "error", text: msg });
    } finally {
      cancelPending();
      busy = false;
      onbusy(false);
      scrollDown();
      input?.focus();
    }
  }

  function pickCommand(c: Command, run: boolean) {
    if (run) {
      draft = `/${c.name}`;
      submit();
    } else {
      draft = `/${c.name} `;
      input?.focus();
    }
  }

  const isImage = (path: string) => /\.(png|jpe?g|gif|webp|svg)(?:$|[?#])/i.test(path);
  function storedUser(text: string, attachments: string[] = []): Item {
    const at = text.lastIndexOf("\n\n[Archivos adjuntos]\n");
    if (at < 0) return { kind: "user", text, files: attachments.map((path) => ({ path, name: path.split("/").pop() || path })) };
    const paths = attachments.length ? attachments : text.slice(at).split("\n").filter((l) => l.startsWith("- ")).map((l) => l.slice(2));
    return { kind: "user", text: text.slice(0, at), files: paths.map((path) => ({ path: !path.startsWith("/") && projectById(card.projectId)?.path ? `${projectById(card.projectId)!.path}/${path}` : path, name: path.split("/").pop() || path })) };
  }

  // ---------- Conversación ----------

  const ctxFraction = $derived(context?.window ? Math.min(1, context.used / context.window) : null);

  async function scrollDown() {
    await tick();
    scroller?.scrollTo({ top: scroller.scrollHeight });
  }

  function toolIcon(name: string): IconName {
    if (name === "Bash") return "terminal";
    if (name === "Read") return "file";
    if (name === "Grep" || name === "Glob") return "search";
    if (name.startsWith("Web")) return "globe";
    return "right";
  }

  onMount(() => {
    // Un chat creado desde Inicio con un mensaje lo envía al abrirse; uno vacío recibe el foco.
    if (ui.pending?.cardId === card.id) {
      const first = ui.pending.text;
      ui.pending = null;
      submit(first);
    } else if (canvas.focusId === card.id && items.length === 0) {
      queueMicrotask(() => input?.focus());
    }

    // Al reabrir la app, la conversación vuelve de lo guardado.
    chatHistory(card.projectId, card.id)
      .then((msgs) => {
        if (items.length > 0 || msgs.length === 0) return;
        items = msgs.map((m) => (m.role === "user" ? storedUser(m.content, m.attachments) : { kind: "assistant", provider: m.provider ?? "claude", text: m.content, files: m.outputs?.length ? m.outputs : undefined }));
        scrollDown();
      })
      .catch((e) => { canvas.projectError = String(e); });

    let off: (() => void) | undefined;
    onChatEvent((ev) => {
      if (ev.chat_id !== card.id) return;
      if (ev.kind === "Handoff") {
        recordHandoff(ev.from, ev.to, ev.reason);
        items.push({ kind: "handoff", from: ev.from, to: ev.to, reason: ev.reason });
        // El chat sigue con la IA del relevo: el selector lo refleja, con lo último que se usó con ella.
        if (["claude", "codex", "openrouter"].includes(ev.to)) setCardAI(card.id, { ...defaultsFor(ev.to) });
        scrollDown();
        return;
      }
      if (ev.kind === "Outputs") {
        // Los archivos van con la respuesta de este turno, aunque la IA no los nombre en el texto.
        const start = items.map((i) => i.kind).lastIndexOf("user");
        let target = items.slice(start + 1).reverse().find((i) => i.kind === "assistant") as Extract<Item, { kind: "assistant" }> | undefined;
        if (!target && ev.files.length) {
          items.push({ kind: "assistant", provider: ev.provider, model: lastModel[ev.provider] ?? null, text: "" });
          target = items[items.length - 1] as Extract<Item, { kind: "assistant" }>;
        }
        if (target) {
          const known = new Set((target.files ?? []).map((f) => f.path));
          target.files = [...(target.files ?? []), ...ev.files.filter((f) => !known.has(f.path))];
        }
        for (const r of ev.rejected) items.push({ kind: "log", text: `No se adjuntó ${r}`, icon: "alert" });
        bumpFiles();
        scrollDown();
        return;
      }
      const e = ev.event;
      switch (e.type) {
        case "Finished":
          bumpFiles();
          break;
        case "Started":
          activeModel = e.data.model;
          lastModel[ev.provider] = e.data.model;
          break;
        case "Text": {
          const last = items[items.length - 1];
          if (last?.kind === "assistant" && last.provider === ev.provider) last.text += e.data;
          else items.push({ kind: "assistant", provider: ev.provider, model: lastModel[ev.provider] ?? (ev.provider === ai.provider ? ai.model : null), text: e.data });
          break;
        }
        case "ToolUse": {
          const data = (e.data.input ?? {}) as Record<string, unknown>;
          const d = isEditTool(e.data.name) ? toolDiff(e.data.name, data) : null;
          if (d) {
            items.push({ kind: "change", path: d.path, rows: d.rows, created: d.created, memory: d.path.endsWith("/.comodel/MEMORY.md") });
            setTimeout(bumpFiles, 1200); // la herramienta ya habrá escrito el archivo
          } else items.push({ kind: "log", text: toolLabel(e.data.name, data), icon: toolIcon(e.data.name) });
          break;
        }
        case "FileTouched":
          // Ya se muestra con su diff al llegar el uso de la herramienta.
          break;
        case "Permission":
          items.push({
            kind: "permission",
            id: e.data.id,
            tool: e.data.tool,
            input: e.data.input,
            description: e.data.description,
            canAcceptEdits: e.data.can_accept_edits,
            state: "pending",
          });
          break;
        case "Question":
          items.push({ kind: "question", id: e.data.id, questions: e.data.questions, state: "pending" });
          break;
        case "Usage":
          context = { used: e.data.input_tokens, window: e.data.context_window };
          break;
        case "Quota":
          recordQuota(ev.provider, e.data.window, e.data.used_fraction, e.data.resets_at);
          break;
        case "LimitReached":
          items.push({ kind: "log", text: `${providerLabel(ev.provider)} se quedó sin uso`, icon: "alert" });
          break;
        case "Error":
          items.push({ kind: "error", text: e.data });
          break;
      }
      scrollDown();
    }).then((f) => (off = f));
    return () => off?.();
  });

  // Cronómetro mientras el chat trabaja.
  $effect(() => {
    if (!busy) return;
    elapsed = 0;
    const t = setInterval(() => (elapsed += 1), 1000);
    return () => clearInterval(t);
  });

  const clock = (s: number) => `${String(Math.floor(s / 60)).padStart(2, "0")}:${String(s % 60).padStart(2, "0")}`;

  /** Lo que se le pasa a la IA elegida: solo lo que esa IA admite. */
  function turnOptions() {
    return {
      provider: ai.provider,
      model: ai.model || null,
      effort: effortsFor(ai.provider, ai.model).length ? ai.effort : null,
      mode: modesFor(ai.provider, hasFolder).length ? ai.mode : null,
      title: card.title,
      sharedChats: linkedChats(card.id).map((c) => ({ chatId: c.id, projectId: c.projectId, title: c.title })),
    };
  }

  async function submit(text = draft) {
    text = text.trim();
    const files = attached;
    if ((!text && files.length === 0) || busy || canvas.projectSync) return;
    if (files.length === 0 && text.startsWith("/") && (await runCommand(text))) return;
    // El título se pone antes de crear la carpeta del workspace, para que ya se llame por el tema.
    if (text) autoTitle(card.id, text, ai.provider);
    try { await ensureChatWorkspace(card.id); }
    catch (e) { canvas.projectError = String(e); return; }
    if (!canvas.cards.some((c) => c.id === card.id) || busy || canvas.projectSync) return;
    if (!text) text = "Revisa los archivos adjuntos.";
    try {
      localStorage.setItem("comodel.sentFirst", "1");
    } catch {
      // sin almacenamiento
    }
    draft = "";
    attached = [];
    attachError = "";
    resize();
    busy = true;
    onbusy(true);
    items.push({ kind: "user", text, files: files.map((f) => ({ ...f })) });
    scrollDown();
    try {
      await sendMessage(card.projectId, card.id, text, { ...turnOptions(), attachments: files.map((f) => f.path) });
    } catch (e) {
      const msg = String(e);
      if (msg === "Detenido.") items.push({ kind: "log", text: "Detuviste el mensaje.", icon: "stop" });
      else items.push({ kind: "error", text: msg });
    } finally {
      cancelPending();
      busy = false;
      onbusy(false);
      scrollDown();
      input?.focus();
    }
  }

  /** Contesta un permiso o una pregunta y deja anotado qué se eligió. */
  async function respond(item: PermissionItem | QuestionItem, answer: Answer, answers?: Record<string, string>) {
    const it = items.find((x) => (x.kind === "permission" || x.kind === "question") && x.id === item.id);
    try {
      await answerInteraction(item.id, answer);
    } catch (e) {
      items.push({ kind: "error", text: String(e) });
      return;
    }
    if (it?.kind === "permission") it.state = answer.kind === "allow" ? (answer.always ? "always" : "allowed") : "denied";
    if (it?.kind === "question") {
      it.state = answer.kind === "answers" ? "answered" : "denied";
      it.answers = answers;
    }
  }

  /** Lo que quedó sin respuesta al terminar o parar el mensaje deja de esperar. */
  function cancelPending() {
    for (const it of items) if ((it.kind === "permission" || it.kind === "question") && it.state === "pending") it.state = "cancelled";
  }

  async function stop() {
    try {
      await stopMessage(card.id);
    } catch {
      // si ya terminó, no hay nada que parar
    }
  }

  // ---------- Copiar ----------

  async function copyMessage(i: number, text: string) {
    if (await copyText(text)) {
      copied = i;
      setTimeout(() => {
        if (copied === i) copied = null;
      }, 1500);
    }
  }

  /** La conversación como Markdown, para pegarla en otro sitio. */
  function transcriptMarkdown() {
    const parts = [`# ${card.title}`];
    for (const it of items) {
      if (it.kind === "user") parts.push(`**Tú:**\n\n${it.text}`);
      else if (it.kind === "assistant") parts.push(`**${providerLabel(it.provider)}:**\n\n${it.text}`);
      else if (it.kind === "handoff") parts.push(`_Relevo de ${providerLabel(it.from)} a ${providerLabel(it.to)}: ${it.reason}_`);
    }
    return parts.join("\n\n");
  }

  // ---------- Adjuntos ----------

  const MAX_FILES = 10;

  function addFiles(list: Attached[]) {
    attachError = "";
    const known = new Set(attached.map((a) => a.path));
    const fresh = list.filter((f) => !known.has(f.path));
    if (attached.length + fresh.length > MAX_FILES) {
      attachError = `Puedes adjuntar hasta ${MAX_FILES} archivos por mensaje.`;
      return;
    }
    attached = [...attached, ...fresh];
  }

  async function choose() {
    try {
      addFiles(await pickFiles());
    } catch (e) {
      attachError = String(e);
    }
    input?.focus();
  }

  const removeFile = (path: string) => {
    attached = attached.filter((a) => a.path !== path);
    attachError = "";
  };

  /** Pegar una imagen en el cuadro de texto la adjunta; el texto pegado sigue funcionando igual. */
  async function onPaste(e: ClipboardEvent) {
    const images = [...(e.clipboardData?.files ?? [])].filter((f) => f.type.startsWith("image/"));
    if (images.length === 0) return;
    e.preventDefault();
    for (const file of images) {
      try {
        const data = await new Promise<string>((resolve, reject) => {
          const r = new FileReader();
          r.onload = () => resolve(String(r.result).split(",")[1] ?? "");
          r.onerror = () => reject(new Error("No se pudo leer la imagen pegada."));
          r.readAsDataURL(file);
        });
        const name = file.name && file.name !== "image.png" ? file.name : `imagen-${attached.length + 1}.png`;
        const path = await savePastedImage(card.projectId, name, data);
        addFiles([{ name, path, thumb: URL.createObjectURL(file) }]);
      } catch (err) {
        attachError = String(err instanceof Error ? err.message : err);
      }
    }
  }

  // Archivos soltados sobre esta tarjeta desde el sistema (los reparte Canvas).
  onMount(() => {
    const onAttach = (e: Event) => {
      const d = (e as CustomEvent<{ cardId: string; files: Attached[] }>).detail;
      if (d.cardId === card.id) addFiles(d.files);
    };
    window.addEventListener("comodel:attach", onAttach);
    return () => window.removeEventListener("comodel:attach", onAttach);
  });

  function onKey(e: KeyboardEvent) {
    if (menuOpen && !e.isComposing) {
      if (e.key === "ArrowDown" || e.key === "ArrowUp") {
        e.preventDefault();
        sel = (sel + (e.key === "ArrowDown" ? 1 : matches.length - 1)) % matches.length;
        return;
      }
      if (e.key === "Tab" || e.key === "Enter") {
        e.preventDefault();
        // Enter ejecuta; Tab solo completa, para escribir un enfoque después.
        pickCommand(matches[sel], e.key === "Enter");
        return;
      }
      if (e.key === "Escape") {
        e.preventDefault();
        menuDismissed = true;
        return;
      }
    }
    if (e.key === "Enter" && !e.shiftKey && !e.isComposing) {
      e.preventDefault();
      submit();
    }
  }

  function resize() {
    if (!input) return;
    const max = input.ownerDocument.defaultView!.getComputedStyle(input).maxHeight;
    input.style.height = "auto";
    const limit = parseFloat(max) || Infinity;
    input.style.height = `${Math.min(input.scrollHeight, limit)}px`;
    input.style.overflowY = input.scrollHeight > limit ? "auto" : "hidden";
  }

  // Recalcular la altura del cuadro de texto cuando cambia su contenido (también al vaciarlo tras enviar,
  // cuando el valor ya se aplicó al DOM) o el ancho de la tarjeta.
  $effect(() => {
    void card.w;
    void draft;
    resize();
  });

  // ---------- Tarjeta ----------

  function track(e: PointerEvent, apply: (dx: number, dy: number) => void) {
    trackPointer(e, scale, onfocus, apply, (on) => (dragging = on));
  }

  function startDrag(e: PointerEvent) {
    if ((e.target as HTMLElement).closest("button, input, [role='menu']")) return;
    track(e, onmove);
  }

  function headerKey(e: KeyboardEvent) {
    if (e.target !== e.currentTarget) return;
    const step = e.shiftKey ? 64 : 16;
    const d: Record<string, [number, number]> = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] };
    const v = d[e.key];
    if (v) {
      e.preventDefault();
      onmove(...v);
    }
  }

  function commitRename(e: Event) {
    onrename((e.currentTarget as HTMLInputElement).value);
    renaming = false;
  }

  const hasTalk = $derived(items.some((i) => i.kind === "user" || i.kind === "assistant"));
  const actions = $derived<Action[]>([
    { label: "Renombrar", icon: "pencil", run: () => (renaming = true) },
    { label: "Copiar la conversación", icon: "copy", disabled: !hasTalk, run: () => copyText(transcriptMarkdown()) },
    { label: "Centrar en el lienzo", icon: "fit", run: () => centerOn(card.id) },
    { label: "Resumir para liberar memoria", icon: "memory", disabled: !hasTalk || busy, sep: true, run: () => compact("") },
    { label: "Empezar de cero", icon: "refresh", disabled: busy, run: clearContext },
    { label: "Abrir la memoria del proyecto", icon: "file", disabled: !card.projectId, run: openMemory },
    { label: "Cerrar el chat", icon: "x", danger: true, sep: true, run: onclose },
  ]);

  // Mientras trabaja y aún no hay texto nuevo de la IA, una línea dice que sigue en marcha.
  const thinking = $derived(busy && items[items.length - 1]?.kind !== "assistant");
  const shownModel = $derived(activeModel ? modelLabel(ai.provider, activeModel) : modelLabel(ai.provider, ai.model));
</script>

<article
  class="card"
  data-card-id={card.id}
  class:focused
  class:dragging
  style:left="{card.x}px"
  style:top="{card.y}px"
  style:width="{card.w}px"
  style:height="{card.h}px"
  style:z-index={card.z}
  aria-label={card.title}
  onpointerdown={onfocus}
  in:cardIn|global
  out:cardOut|global
>
  <!-- El encabezado es el asa de arrastre; también se mueve con las flechas. -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
  <header role="group" onpointerdown={startDrag} onkeydown={headerKey} tabindex="0" aria-label="Mover {card.title}. Flechas para mover.">
    {#if busy}<span class="activity on" aria-hidden="true"></span>{:else}<span class="pdot" data-p={ai.provider} aria-hidden="true"></span>{/if}
    {#if renaming}
      <!-- svelte-ignore a11y_autofocus -->
      <input
        class="rename input"
        value={card.title}
        maxlength="40"
        aria-label="Nombre del chat"
        autofocus
        onblur={commitRename}
        onkeydown={(e) => {
          if (e.key === "Enter") (e.currentTarget as HTMLInputElement).blur();
          if (e.key === "Escape") renaming = false;
        }}
      />
    {:else}
      <button class="title" ondblclick={() => (renaming = true)} title="Doble clic para renombrar">{card.title}</button>
    {/if}
    <span class="spacer"></span>
    {#if busy}
      <span class="status" role="status">Trabajando <span class="clock">{clock(elapsed)}</span></span>
    {:else}
      <span class="status" title="IA y modelo de este chat">{aiLabel(ai.provider)}{#if shownModel}<span class="sep">·</span>{shownModel}{/if}</span>
    {/if}
    <ActionMenu label="Acciones del chat" {actions} />
    <button class="icon-btn sm" onclick={onclose} aria-label="Cerrar {card.title}" title="Cerrar (el historial se guarda)"><Icon name="x" size="0.95rem" /></button>
  </header>

  <div class="transcript" bind:this={scroller} aria-live="polite">
    {#if items.length === 0}
      <div class="start" out:fade={{ duration: ms(120) }}>
        <p class="lead">¿Qué le pedimos a {aiLabel(ai.provider)}?</p>
        <p class="sub">
          {card.projectId
            ? "Trabaja sobre los archivos del proyecto y recuerda lo que hicieron las otras IAs."
            : "Su carpeta de trabajo se creará en CoModel/Chats al enviar el primer mensaje."}
        </p>
        <div class="starters">
          {#each starters as s}<button class="starter" onclick={() => submit(s)}><Icon name="right" size="0.8rem" />{s}</button>{/each}
        </div>
      </div>
    {/if}

    {#each items as item, i}
      {#if item.kind === "user"}
        <section class="turn user" in:rise>
          <p>{item.text}</p>
          {#if item.files?.length}
            <div class="sent">{#each item.files as f}{#if isImage(f.path)}<ChatImage src={f.thumb ?? f.path} alt={f.name} projectId={card.projectId} />{:else}<span><Icon name="file" size="0.75rem" />{f.name}</span>{/if}{/each}</div>
          {/if}
        </section>
      {:else if item.kind === "assistant"}
        <section class="turn ai" in:rise>
          <div class="who">
            <span class="pdot" data-p={item.provider} aria-hidden="true"></span>
            <span class="pname">{providerLabel(item.provider)}</span>
            {#if item.model}<span class="pmodel">{modelLabel(item.provider, item.model)}</span>{/if}
            <button class="copy icon-btn sm" onclick={() => copyMessage(i, item.text)} aria-label="Copiar la respuesta" title="Copiar la respuesta">
              <Icon name={copied === i ? "check" : "copy"} size="0.85rem" />
            </button>
          </div>
          <Markdown text={item.text} projectId={card.projectId} live={busy && i === items.length - 1} />
          {#if item.files?.length}<ChatFiles files={item.files} projectId={card.projectId} />{/if}
        </section>
      {:else if item.kind === "change"}
        <details class="change" in:rise>
          <summary>
            <span class="what">
              <Icon name={item.memory ? "memory" : "pencil"} size="0.8rem" />
              {#if item.memory}Actualizó la memoria compartida{:else}{item.created ? "Escribió" : "Cambió"} <span class="p">{item.path.split("/").pop()}</span>{/if}
            </span>
            <span class="count"><span class="plus">+{countOf(item.rows).add}</span>{#if countOf(item.rows).del}<span class="minus">−{countOf(item.rows).del}</span>{/if}</span>
          </summary>
          <div class="change-body">
            <p class="fullpath">{item.path}</p>
            {#if item.rows.length}<DiffView rows={item.rows} />{/if}
            {#if relPath(item.path) && card.projectId}
              <button class="link open-file" onclick={() => openFileCard(card.projectId!, relPath(item.path)!)}>Abrir el archivo</button>
            {/if}
          </div>
        </details>
      {:else if item.kind === "permission"}
        <PermissionBlock item={item} provider={aiLabel(ai.provider)} onanswer={(a) => respond(item, a)} />
      {:else if item.kind === "question"}
        <QuestionBlock item={item} provider={aiLabel(ai.provider)} onanswer={(a, ans) => respond(item, a, ans)} />
      {:else if item.kind === "log"}
        <div class="log" in:rise><Icon name={item.icon ?? "right"} size="0.8rem" /><span>{item.text}</span></div>
      {:else if item.kind === "handoff"}
        <div class="handoff" in:rise role="status">
          <span class="hline"></span>
          <span class="htext"><Icon name="relay" size="0.85rem" />Sigue {providerLabel(item.to)}: {providerLabel(item.from)} se quedó sin uso</span>
          <span class="hline"></span>
        </div>
      {:else}
        <div class="error" role="alert" in:rise><Icon name="alert" size="0.95rem" /><span>{item.text}</span></div>
      {/if}
    {/each}

    {#if thinking}
      <div class="thinking" in:fade={{ duration: ms(160) }} role="status">
        <span class="activity on" aria-hidden="true"></span>{aiLabel(ai.provider)} está trabajando…
      </div>
    {/if}
  </div>

  <div class="composer">
    {#if menuOpen}
      <div class="slash popover" role="listbox" aria-label="Comandos" in:popIn out:popOut>
        <div class="popover-label">Comandos</div>
        {#each matches as c, i (c.name)}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <div
            class="cmd"
            class:active={i === sel}
            role="option"
            tabindex="-1"
            aria-selected={i === sel}
            onmousemove={() => (sel = i)}
            onmousedown={(e) => {
              e.preventDefault();
              pickCommand(c, true);
            }}
          >
            <span class="cname">/{c.name}{#if c.arg}<span class="carg"> [{c.arg}]</span>{/if}</span>
            <span class="cdesc">{c.desc}</span>
          </div>
        {/each}
      </div>
    {/if}

    <div class="box" class:busy>
      {#if attached.length}
        <ul class="chips" aria-label="Archivos adjuntos">
          {#each attached as a (a.path)}
            <li class="chip" in:rise={{ y: 4, duration: 140 }} out:rise={{ y: 4, duration: 90 }}>
              {#if isImage(a.path)}<ChatImage src={a.thumb ?? a.path} alt={a.name} projectId={card.projectId} compact />{:else}<Icon name="file" size="0.8rem" />{/if}
              <span class="fname" title={a.path}>{a.name}</span>
              <button type="button" onclick={() => removeFile(a.path)} aria-label="Quitar {a.name}"><Icon name="x" size="0.75rem" /></button>
            </li>
          {/each}
        </ul>
      {/if}
      <textarea
        bind:this={input}
        bind:value={draft}
        oninput={resize}
        onkeydown={onKey}
        onpaste={onPaste}
        rows="1"
        placeholder={busy ? "Esperando la respuesta…" : "Escribe un mensaje o / para comandos"}
        aria-label="Mensaje para {aiLabel(ai.provider)}"
        disabled={busy}
      ></textarea>
      <div class="bar">
        <button type="button" class="icon-btn sm" onclick={choose} disabled={busy} aria-label="Adjuntar archivos" title="Adjuntar archivos (o arrástralos aquí)"><Icon name="plus" size="1.05rem" /></button>
        <AIPicker {ai} {hasFolder} disabled={busy} onchange={(v) => setCardAI(card.id, v)} />
        <span class="grow"></span>
        <span
          class="ctx"
          class:warn={ctxFraction !== null && ctxFraction >= 0.7}
          class:danger={ctxFraction !== null && ctxFraction >= 0.9}
          title={context
            ? `Memoria de trabajo ocupada: ${formatTokens(context.used)}${context.window ? ` de ${formatTokens(context.window)}` : ""} tokens. Si se llena, usa /compact.`
            : "Memoria de trabajo de la IA: aún sin datos"}
        >
          <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
            <circle cx="8" cy="8" r="6" class="ring-bg" />
            <circle cx="8" cy="8" r="6" class="ring" style:stroke-dasharray="{(ctxFraction ?? 0) * 37.7} 37.7" />
          </svg>
          {#if ctxFraction !== null}{Math.max(1, Math.round(ctxFraction * 100))}%{:else if context}{formatTokens(context.used)}{/if}
        </span>
        {#if busy}
          <button class="send stop" onclick={stop} aria-label="Parar" title="Parar"><Icon name="stop" size="0.95rem" /></button>
        {:else}
          <button class="send" onclick={() => submit()} disabled={canvas.projectSync || card.workspacePending || (!draft.trim() && attached.length === 0)} aria-label="Enviar mensaje" title="Enviar (Enter)"><Icon name="send" size="1rem" /></button>
        {/if}
      </div>
    </div>
    {#if attachError}<p class="attach-error" role="alert">{attachError}</p>{/if}
    <div class="where">
      <FolderMenu projectId={card.projectId} locked={items.length > 0 || busy || canvas.projectSync || card.workspacePending === true} onchange={onproject} />
      <span class="hintkeys" aria-hidden="true"><kbd>Enter</kbd> enviar · <kbd>Mayús</kbd>+<kbd>Enter</kbd> nueva línea</span>
    </div>
  </div>

  <div
    class="grip"
    aria-hidden="true"
    onpointerdown={(e) => {
      e.stopPropagation();
      track(e, onresize);
    }}
  ></div>
</article>

<style>
  .card {
    position: absolute;
    display: flex;
    flex-direction: column;
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: var(--radius-xl);
    box-shadow: var(--shadow-card);
    transition:
      border-color var(--dur-base) var(--ease),
      box-shadow var(--dur-base) var(--ease);
  }
  .card.focused {
    border-color: color-mix(in oklch, var(--accent) 50%, var(--line));
    box-shadow:
      var(--shadow-card),
      0 0 0 3px color-mix(in oklch, var(--accent) 12%, transparent);
  }
  .card.dragging {
    box-shadow: var(--lift);
  }
  :global(.card.drop) {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in oklch, var(--accent) 25%, transparent);
  }

  header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex: none;
    height: 2.6rem;
    padding: 0 0.4rem 0 0.9rem;
    border-bottom: 1px solid var(--line-soft);
    cursor: grab;
    touch-action: none;
    user-select: none;
  }
  .card.dragging header {
    cursor: grabbing;
  }
  .title {
    min-width: 0;
    max-width: 13rem;
    padding: 0.1rem 0.25rem;
    margin-left: -0.15rem;
    border: 0;
    border-radius: var(--radius-sm);
    background: none;
    font-size: var(--fs-sm);
    font-weight: 600;
    color: var(--text);
    cursor: inherit;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .rename {
    width: 11rem;
    height: 1.6rem;
    font-weight: 600;
  }
  .spacer {
    flex: 1;
  }
  .status {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .status .sep {
    opacity: 0.6;
  }
  .clock {
    font-family: var(--mono);
    font-variant-numeric: tabular-nums;
  }

  .transcript {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overscroll-behavior: contain;
    padding: 0.75rem 0 1rem;
  }

  .start {
    padding: 1.75rem 1.4rem 1rem;
  }
  .lead {
    margin: 0;
    font-size: var(--fs-lg);
    font-weight: 600;
    letter-spacing: -0.01em;
  }
  .sub {
    margin: 0.35rem 0 1.1rem;
    max-width: 38ch;
    color: var(--text-3);
    font-size: var(--fs-sm);
  }
  .starters {
    display: grid;
    gap: 0.35rem;
  }
  .starter {
    display: flex;
    align-items: center;
    gap: 0.55rem;
    padding: 0.5rem 0.7rem;
    border: 1px solid var(--line-soft);
    border-radius: var(--radius);
    background: none;
    color: var(--text-2);
    font-size: var(--fs-sm);
    text-align: left;
    transition:
      background-color var(--dur-fast) var(--ease),
      border-color var(--dur-fast) var(--ease),
      color var(--dur-fast) var(--ease);
  }
  .starter :global(.icon) {
    color: var(--text-3);
  }
  .starter:hover {
    border-color: var(--line);
    background: var(--raised);
    color: var(--text);
  }

  .turn {
    padding: 0.5rem 1.4rem;
  }
  .turn.user {
    margin: 0.35rem 0.9rem 0.6rem;
    padding: 0.6rem 0.8rem;
    border-radius: var(--radius-lg);
    background: var(--raised);
  }
  .turn.user p {
    margin: 0;
    font-size: var(--fs-md);
    line-height: 1.55;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .turn.ai {
    position: relative;
  }
  .who {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    height: 1.6rem;
    margin-bottom: 0.15rem;
  }
  .pname {
    font-size: var(--fs-xs);
    font-weight: 600;
    color: var(--text-2);
  }
  .pmodel {
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .copy {
    margin-left: auto;
    opacity: 0;
    color: var(--text-3);
    transition:
      opacity var(--dur-fast) var(--ease),
      background-color var(--dur-fast) var(--ease);
  }
  .turn.ai:hover .copy,
  .copy:focus-visible {
    opacity: 1;
  }

  .log {
    display: flex;
    align-items: flex-start;
    gap: 0.5rem;
    padding: 0.12rem 1.4rem;
    font-size: var(--fs-xs);
    line-height: 1.45;
    color: var(--text-3);
    overflow-wrap: anywhere;
  }
  .log :global(.icon) {
    margin-top: 0.15rem;
  }
  .thinking {
    display: flex;
    align-items: center;
    gap: 0.55rem;
    padding: 0.5rem 1.4rem;
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .handoff {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    margin: 0.75rem 1.4rem;
    font-size: var(--fs-xs);
    color: var(--warn);
  }
  .hline {
    flex: 1;
    height: 1px;
    background: var(--line);
  }
  .htext {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    max-width: 75%;
    text-align: center;
  }
  .error {
    display: flex;
    align-items: flex-start;
    gap: 0.55rem;
    margin: 0.4rem 0.9rem;
    padding: 0.55rem 0.75rem;
    border-radius: var(--radius);
    background: color-mix(in oklch, var(--danger) 10%, var(--surface));
    border: 1px solid color-mix(in oklch, var(--danger) 35%, var(--line));
    font-size: var(--fs-sm);
    color: var(--text);
    overflow-wrap: anywhere;
  }
  .error :global(.icon) {
    margin-top: 0.15rem;
    color: var(--danger);
  }

  .change {
    margin: 0.15rem 1.4rem;
    font-size: var(--fs-xs);
  }
  .change summary {
    display: flex;
    justify-content: space-between;
    gap: 0.75rem;
    cursor: pointer;
    list-style: none;
    padding: 0.12rem 0;
    color: var(--text-2);
  }
  .change summary::-webkit-details-marker {
    display: none;
  }
  .what {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    min-width: 0;
  }
  .what :global(.icon) {
    color: var(--text-3);
  }
  .change summary:hover .p {
    text-decoration: underline;
    text-underline-offset: 0.2em;
  }
  .p {
    font-family: var(--mono);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .change-body {
    padding: 0.35rem 0 0.4rem;
  }
  .fullpath {
    margin: 0 0 0.35rem;
    font-family: var(--mono);
    color: var(--text-3);
    overflow-wrap: anywhere;
  }
  .count {
    display: inline-flex;
    gap: 0.4rem;
    flex: none;
    font-family: var(--mono);
    font-variant-numeric: tabular-nums;
  }
  .plus {
    color: var(--ok);
  }
  .minus {
    color: var(--danger);
  }
  .open-file {
    margin-top: 0.35rem;
    font-size: var(--fs-xs);
  }
  .sent {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
    margin-top: 0.45rem;
  }
  .sent span {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    padding: 0.08rem 0.45rem;
    background: var(--surface);
    border: 1px solid var(--line-soft);
    border-radius: var(--radius-sm);
    font-size: var(--fs-xs);
    color: var(--text-2);
  }

  .composer {
    position: relative;
    flex: none;
    padding: 0 0.65rem 0.5rem;
  }
  .box {
    border: 1px solid var(--line);
    border-radius: var(--radius-lg);
    background: var(--bg);
    padding: 0.45rem 0.45rem 0.35rem;
    transition:
      border-color var(--dur-fast) var(--ease),
      box-shadow var(--dur-fast) var(--ease);
  }
  .box:focus-within {
    border-color: color-mix(in oklch, var(--accent) 55%, var(--line));
    box-shadow: 0 0 0 3px color-mix(in oklch, var(--accent) 13%, transparent);
  }
  .box.busy {
    border-style: dashed;
  }
  .slash {
    position: absolute;
    left: 0.65rem;
    right: 0.65rem;
    bottom: calc(100% + 0.2rem);
  }
  .cmd {
    display: grid;
    gap: 0.05rem;
    padding: 0.4rem 0.55rem;
    border-radius: var(--radius);
    cursor: pointer;
  }
  .cmd.active {
    background: var(--raised-2);
  }
  .cname {
    font-family: var(--mono);
    font-size: var(--fs-sm);
    color: var(--text);
  }
  .carg {
    color: var(--text-3);
  }
  .cdesc {
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  textarea {
    display: block;
    width: 100%;
    resize: none;
    border: 0;
    background: transparent;
    padding: 0.2rem 0.35rem;
    font-size: var(--fs-md);
    line-height: 1.45;
    max-height: 11.5rem;
    overflow-y: hidden;
  }
  textarea:focus-visible {
    outline: none;
  }
  textarea::placeholder {
    color: var(--text-3);
  }
  .bar {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 0.15rem;
    margin-top: 0.3rem;
  }
  .grow {
    flex: 1;
  }
  .ctx {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    margin-right: 0.35rem;
    font-size: var(--fs-xs);
    color: var(--text-3);
    font-variant-numeric: tabular-nums;
  }
  .ring-bg {
    fill: none;
    stroke: var(--line);
    stroke-width: 2;
  }
  .ring {
    fill: none;
    stroke: var(--text-3);
    stroke-width: 2;
    stroke-linecap: round;
    transform: rotate(-90deg);
    transform-origin: 50% 50%;
    transition: stroke-dasharray var(--dur-slow) var(--ease);
  }
  .ctx.warn {
    color: var(--warn);
  }
  .ctx.warn .ring {
    stroke: var(--warn);
  }
  .ctx.danger {
    color: var(--danger);
  }
  .ctx.danger .ring {
    stroke: var(--danger);
  }
  .send {
    display: grid;
    place-items: center;
    flex: none;
    width: 1.9rem;
    height: 1.9rem;
    border: 0;
    border-radius: var(--radius);
    background: var(--accent-solid);
    color: var(--on-accent);
    transition: background-color var(--dur-fast) var(--ease);
  }
  .send:hover:not(:disabled) {
    background: var(--accent-solid-hover);
  }
  .send:disabled {
    background: var(--raised-2);
    color: var(--text-3);
  }
  .send.stop {
    background: var(--text);
    color: var(--surface);
  }
  .send.stop:hover {
    background: var(--text-2);
  }
  .where {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    margin-top: 0.3rem;
    padding: 0 0.1rem;
  }
  .hintkeys {
    font-size: 0.6875rem;
    color: var(--text-3);
    white-space: nowrap;
    overflow: hidden;
  }
  .hintkeys kbd {
    font-size: 0.625rem;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 0.35rem;
    margin: 0 0 0.35rem;
    padding: 0.1rem 0.2rem 0;
    list-style: none;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    max-width: 14rem;
    min-height: 1.6rem;
    padding: 0.15rem 0.15rem 0.15rem 0.45rem;
    background: var(--raised);
    border: 1px solid var(--line-soft);
    border-radius: var(--radius);
    font-size: var(--fs-xs);
    color: var(--text-2);
  }
  .fname {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .chip button {
    display: grid;
    place-items: center;
    border: 0;
    background: none;
    width: 1.25rem;
    height: 1.25rem;
    border-radius: var(--radius-sm);
    color: var(--text-3);
  }
  .chip button:hover {
    background: var(--raised-2);
    color: var(--text);
  }
  .attach-error {
    margin: 0.3rem 0.2rem 0;
    font-size: var(--fs-xs);
    color: var(--danger);
  }

  .grip {
    position: absolute;
    right: 0;
    bottom: 0;
    width: 1rem;
    height: 1rem;
    cursor: nwse-resize;
    touch-action: none;
    border-bottom-right-radius: var(--radius-xl);
    background: linear-gradient(135deg, transparent 58%, var(--line) 58%, var(--line) 64%, transparent 64%, transparent 76%, var(--line) 76%, var(--line) 82%, transparent 82%);
    opacity: 0;
    transition: opacity var(--dur-fast) var(--ease);
  }
  .card:hover .grip {
    opacity: 1;
  }
</style>
