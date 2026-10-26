// Markdown seguro: se devuelve estructura, nunca HTML, así que no hay nada que inyectar.
// Cubre lo que escriben las IAs a diario: títulos, listas (con un nivel de anidado), listas numeradas,
// citas, tablas, separadores, bloques de código y, en línea, código, negrita, cursiva, tachado y enlaces.

export type Inline =
  | { t: "image"; alt: string; src: string }
  | { t: "text"; v: string }
  | { t: "code"; v: string }
  | { t: "strong"; v: Inline[] }
  | { t: "em"; v: Inline[] }
  | { t: "del"; v: Inline[] }
  | { t: "link"; v: Inline[]; href: string };

export interface ListItem {
  inl: Inline[];
  /** Sublista (un nivel). */
  sub?: { ordered: boolean; items: Inline[][] };
}

export type Block =
  | { t: "p"; inl: Inline[] }
  | { t: "h"; level: 1 | 2 | 3; inl: Inline[] }
  | { t: "ul"; items: ListItem[] }
  | { t: "ol"; start: number; items: ListItem[] }
  | { t: "quote"; inl: Inline[] }
  | { t: "hr" }
  | { t: "table"; head: Inline[][]; rows: Inline[][][]; align: ("left" | "center" | "right" | null)[] }
  | { t: "code"; lang: string; v: string };

/** Solo enlaces web y de correo; cualquier otro esquema se queda como texto. */
export function safeHref(href: string): string | null {
  const h = href.trim();
  return /^(https?:\/\/|mailto:)/i.test(h) ? h : null;
}

const INLINE = /(`[^`\n]+`)|(\[[^\]\n]+\]\([^)\s]+\))|(\*\*[^*\n]+\*\*|__[^_\n]+__)|(~~[^~\n]+~~)|(\*[^*\s][^*\n]*\*)|(https?:\/\/[^\s<>()]+[^\s<>().,;:!?'"])/;

export function parseInline(src: string): Inline[] {
  const out: Inline[] = [];
  let rest = src;
  while (rest) {
    const m = INLINE.exec(rest);
    const img = /!\[([^\]\n]*)\]\((<[^>\n]+>|(?:[^()\n]|\([^()]*\))+?)(?:\s+"[^"\n]*")?\)/.exec(rest);
    if (img && (!m || img.index <= m.index)) {
      if (img.index) out.push({ t: "text", v: rest.slice(0, img.index) });
      const raw = img[2].replace(/^<|>$/g, "");
      // No ejecutar esquemas de script; el componente resuelve las rutas locales.
      if (!/^[a-z][a-z0-9+.-]*:/i.test(raw) || /^(https?:|file:|data:image\/(?:png|jpeg|gif|webp);base64,)/i.test(raw))
        out.push({ t: "image", alt: img[1], src: raw });
      else out.push({ t: "text", v: img[1] });
      rest = rest.slice(img.index + img[0].length);
      continue;
    }
    if (!m) {
      out.push({ t: "text", v: rest });
      break;
    }
    if (m.index > 0) out.push({ t: "text", v: rest.slice(0, m.index) });
    const tok = m[0];
    if (m[1]) out.push({ t: "code", v: tok.slice(1, -1) });
    else if (m[2]) {
      const lm = /^\[([^\]]+)\]\(([^)\s]+)\)$/.exec(tok)!;
      const href = safeHref(lm[2]);
      if (href) out.push({ t: "link", v: parseInline(lm[1]), href });
      else out.push({ t: "text", v: lm[1] });
    } else if (m[3]) out.push({ t: "strong", v: parseInline(tok.slice(2, -2)) });
    else if (m[4]) out.push({ t: "del", v: parseInline(tok.slice(2, -2)) });
    else if (m[5]) out.push({ t: "em", v: parseInline(tok.slice(1, -1)) });
    else out.push({ t: "link", v: [{ t: "text", v: tok }], href: tok });
    rest = rest.slice(m.index + tok.length);
  }
  return out;
}

/** Texto plano de una línea en línea (para copiar o medir). */
export function plain(inl: Inline[]): string {
  return inl.map((p) => (p.t === "image" ? p.alt : p.t === "text" || p.t === "code" ? p.v : plain(p.v))).join("");
}

const BULLET = /^(\s*)[-*+]\s+(.*)$/;
const NUMBER = /^(\s*)(\d{1,4})[.)]\s+(.*)$/;
const isTableSep = (l: string) => /^\s*\|?\s*:?-{2,}:?\s*(\|\s*:?-{2,}:?\s*)*\|?\s*$/.test(l);
const cells = (l: string) =>
  l
    .trim()
    .replace(/^\|/, "")
    .replace(/\|$/, "")
    .split("|")
    .map((c) => c.trim());

export function parseMarkdown(src: string): Block[] {
  const blocks: Block[] = [];
  const lines = src.replace(/\r\n/g, "\n").split("\n");
  let para: string[] = [];
  let list: { ordered: boolean; start: number; items: ListItem[] } | null = null;
  let quote: string[] = [];

  const flushPara = () => {
    if (para.length) blocks.push({ t: "p", inl: parseInline(para.join(" ")) });
    para = [];
  };
  const flushList = () => {
    if (list) blocks.push(list.ordered ? { t: "ol", start: list.start, items: list.items } : { t: "ul", items: list.items });
    list = null;
  };
  const flushQuote = () => {
    if (quote.length) blocks.push({ t: "quote", inl: parseInline(quote.join(" ")) });
    quote = [];
  };
  const flushAll = () => {
    flushPara();
    flushList();
    flushQuote();
  };

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];

    const fence = line.match(/^\s*```\s*([\w+#.-]*)\s*$/);
    if (fence) {
      flushAll();
      const body: string[] = [];
      i++;
      while (i < lines.length && !/^\s*```\s*$/.test(lines[i])) body.push(lines[i++]);
      blocks.push({ t: "code", lang: fence[1], v: body.join("\n") });
      continue;
    }

    // Tabla: una fila con barras seguida de la línea de separación.
    if (line.includes("|") && i + 1 < lines.length && isTableSep(lines[i + 1])) {
      flushAll();
      const head = cells(line);
      const align = cells(lines[i + 1]).map((c) => (c.startsWith(":") && c.endsWith(":") ? "center" : c.endsWith(":") ? "right" : c.startsWith(":") ? "left" : null));
      const rows: Inline[][][] = [];
      i += 2;
      while (i < lines.length && lines[i].includes("|") && lines[i].trim()) rows.push(cells(lines[i++]).map(parseInline));
      i--;
      blocks.push({ t: "table", head: head.map(parseInline), rows, align });
      continue;
    }

    if (/^\s*([-*_])(\s*\1){2,}\s*$/.test(line)) {
      flushAll();
      blocks.push({ t: "hr" });
      continue;
    }

    const heading = line.match(/^(#{1,6})\s+(.*?)\s*#*\s*$/);
    if (heading) {
      flushAll();
      blocks.push({ t: "h", level: Math.min(3, heading[1].length) as 1 | 2 | 3, inl: parseInline(heading[2]) });
      continue;
    }

    const q = line.match(/^\s*>\s?(.*)$/);
    if (q) {
      flushPara();
      flushList();
      quote.push(q[1]);
      continue;
    }
    flushQuote();

    const b = line.match(BULLET);
    const n = line.match(NUMBER);
    if (b || n) {
      flushPara();
      const indent = (b ? b[1] : n![1]).length;
      const ordered = !b;
      const text = b ? b[2] : n![3];
      const current = list as { ordered: boolean; start: number; items: ListItem[] } | null;
      if (indent >= 2 && current && current.items.length) {
        // Sublista dentro del último elemento.
        const last = current.items[current.items.length - 1];
        last.sub ??= { ordered, items: [] };
        last.sub.items.push(parseInline(text));
        continue;
      }
      if (!current || current.ordered !== ordered) {
        flushList();
        list = { ordered, start: n ? Number(n[2]) : 1, items: [] };
      }
      list!.items.push({ inl: parseInline(text) });
      continue;
    }

    if (!line.trim()) {
      flushPara();
      flushList();
      continue;
    }
    // Una línea con sangría justo después de un elemento de lista lo continúa.
    if (list && /^\s{2,}\S/.test(line)) {
      const l = list as { items: ListItem[] };
      const last = l.items[l.items.length - 1];
      last.inl = [...last.inl, { t: "text", v: " " }, ...parseInline(line.trim())];
      continue;
    }
    flushList();
    para.push(line.trim());
  }
  flushAll();
  return blocks;
}
