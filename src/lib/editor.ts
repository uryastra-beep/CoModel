// Editor de código (CodeMirror 6). Se carga solo al abrir el primer archivo, para no pesar en el arranque.
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { bracketMatching, foldGutter, foldKeymap, HighlightStyle, indentOnInput, LanguageDescription, syntaxHighlighting } from "@codemirror/language";
import { languages } from "@codemirror/language-data";
import { highlightSelectionMatches, searchKeymap } from "@codemirror/search";
import { Compartment, EditorState } from "@codemirror/state";
import { drawSelection, EditorView, highlightActiveLine, highlightActiveLineGutter, keymap, lineNumbers } from "@codemirror/view";
import { tags as t } from "@lezer/highlight";

// Todos los colores salen de los tokens del tema, así que cambiar de tema cambia también el editor.
const theme = EditorView.theme({
  "&": { height: "100%", color: "var(--text)", backgroundColor: "var(--bg)", fontSize: "var(--fs-sm)" },
  "&.cm-focused": { outline: "none" },
  ".cm-scroller": { fontFamily: "var(--mono)", lineHeight: "1.6", overflow: "auto" },
  ".cm-content": { caretColor: "var(--accent)", padding: "0.5rem 0" },
  ".cm-cursor": { borderLeftColor: "var(--accent)" },
  ".cm-gutters": { backgroundColor: "var(--bg)", color: "var(--text-3)", border: "none" },
  ".cm-activeLine": { backgroundColor: "color-mix(in oklch, var(--raised) 55%, transparent)" },
  ".cm-activeLineGutter": { backgroundColor: "transparent", color: "var(--text-2)" },
  "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection": { backgroundColor: "var(--accent-tint)" },
  ".cm-selectionMatch": { backgroundColor: "color-mix(in oklch, var(--accent) 22%, transparent)" },
  ".cm-searchMatch": { backgroundColor: "color-mix(in oklch, var(--warn) 30%, transparent)" },
  ".cm-searchMatch.cm-searchMatch-selected": { backgroundColor: "color-mix(in oklch, var(--warn) 55%, transparent)" },
  ".cm-matchingBracket": { backgroundColor: "var(--raised-2)", outline: "none" },
  ".cm-foldPlaceholder": { backgroundColor: "var(--raised)", border: "none", color: "var(--text-3)" },
  ".cm-panels": { backgroundColor: "var(--surface)", color: "var(--text)", borderColor: "var(--line)" },
  ".cm-panels input, .cm-panels button": { fontFamily: "var(--font)", fontSize: "var(--fs-xs)", color: "var(--text)" },
  ".cm-textfield": { backgroundColor: "var(--bg)", border: "1px solid var(--line)", borderRadius: "var(--radius)" },
  ".cm-button": { backgroundImage: "none", backgroundColor: "var(--raised)", border: "1px solid var(--line)", borderRadius: "var(--radius)" },
});

const highlight = HighlightStyle.define([
  { tag: [t.keyword, t.operatorKeyword, t.modifier, t.controlKeyword], color: "var(--syn-keyword)" },
  { tag: [t.string, t.special(t.string), t.regexp], color: "var(--syn-string)" },
  { tag: [t.number, t.bool, t.null, t.atom], color: "var(--syn-number)" },
  { tag: [t.comment, t.lineComment, t.blockComment], color: "var(--syn-comment)", fontStyle: "italic" },
  { tag: [t.function(t.variableName), t.function(t.propertyName), t.labelName], color: "var(--syn-function)" },
  { tag: [t.typeName, t.className, t.namespace], color: "var(--syn-type)" },
  { tag: [t.propertyName, t.attributeName], color: "var(--text)" },
  { tag: [t.punctuation, t.bracket, t.operator], color: "var(--text-2)" },
  { tag: [t.heading], color: "var(--syn-function)", fontWeight: "600" },
  { tag: [t.link, t.url], color: "var(--accent)", textDecoration: "underline" },
  { tag: t.strong, fontWeight: "700" },
  { tag: t.emphasis, fontStyle: "italic" },
]);

export interface Editor {
  getText(): string;
  /** Sustituye todo el contenido (al recargar desde disco). */
  setText(text: string): void;
  focus(): void;
  destroy(): void;
}

interface Options {
  parent: HTMLElement;
  text: string;
  filename: string;
  onChange: () => void;
  onSave: () => void;
}

export function createEditor({ parent, text, filename, onChange, onSave }: Options): Editor {
  const language = new Compartment();
  const view = new EditorView({
    parent,
    state: EditorState.create({
      doc: text,
      extensions: [
        lineNumbers(),
        foldGutter(),
        highlightActiveLine(),
        highlightActiveLineGutter(),
        history(),
        drawSelection(),
        indentOnInput(),
        bracketMatching(),
        highlightSelectionMatches(),
        syntaxHighlighting(highlight),
        theme,
        language.of([]),
        keymap.of([
          { key: "Mod-s", preventDefault: true, run: () => (onSave(), true) },
          ...defaultKeymap,
          ...historyKeymap,
          ...searchKeymap,
          ...foldKeymap,
          indentWithTab,
        ]),
        EditorView.updateListener.of((u) => u.docChanged && onChange()),
        EditorView.contentAttributes.of({ "aria-label": `Editor de ${filename}` }),
      ],
    }),
  });

  // El resaltado del lenguaje se carga aparte, según el nombre del archivo.
  const desc = LanguageDescription.matchFilename(languages, filename);
  desc?.load().then((support) => view.dispatch({ effects: language.reconfigure(support) })).catch(() => {});

  return {
    getText: () => view.state.doc.toString(),
    setText: (next) => view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: next } }),
    focus: () => view.focus(),
    destroy: () => view.destroy(),
  };
}
