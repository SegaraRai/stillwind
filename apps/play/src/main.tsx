import { css } from "@codemirror/lang-css";
import { javascript } from "@codemirror/lang-javascript";
import { EditorSelection, EditorState, type Extension } from "@codemirror/state";
import { oneDark } from "@codemirror/theme-one-dark";
import { EditorView } from "@codemirror/view";
import { useEffect, useMemo, useRef, useState } from "react";
import { createRoot } from "react-dom/client";

import { requestCompilation, type PlayOutput as Result } from "./compilation.ts";

import "./play.css";

const initialSource = `import { useState } from "react";
import { style } from "stillwind/react";

export function App() {
  const [loading, setLoading] = useState(false);
  const trigger = style.ref();
  const enter = style.animation({
    from: "opacity-0 translate-y-2 scale-98",
    to: "opacity-100 translate-y-0 scale-100",
  });
  return (
    <main {...style("bg-surface p-4 rounded-md", enter("duration-normal ease-standard both"))}>
      <p>Stillwind Play</p>
      <h1>React, compiled as you type.</h1>
      <div {...style(trigger.anchor, "inline-flex")}>
        <button
          {...style(
            "inline-flex items-center rounded-md bg-accent px-4 py-2 text-white transition hover:bg-accent/90",
            loading && "opacity-60 cursor-wait",
            { "aria-busy": loading },
          )}
          onClick={() => setLoading((value) => !value)}
        >
          {loading ? "Saving…" : "Save changes"}
        </button>
        <span {...style({ [trigger\`&:hover\`]: "text-accent" })}> ← automatic named parent selector</span>
      </div>
    </main>
  );
}`;

const initialTheme = `@import "stillwind";

@theme {
  --color-accent: oklch(.6 .2 250);
  --color-white: white;
  --color-surface: oklch(.97 .01 250);
  --spacing-2: .5rem;
  --spacing-4: 1rem;
  --radius-md: .5rem;
  --breakpoint-md: 48rem;
  --duration-normal: 180ms;
  --ease-standard: cubic-bezier(.2, .8, .2, 1);
}`;

type FileTab = "component" | "theme";
type OutputTab = "css" | "transform" | "diagnostics";
type Viewport = "desktop" | "tablet" | "mobile";

function formatCss(source: string): string {
  let output = "";
  let token = "";
  let indent = 0;
  let quote = "";
  let parentheses = 0;

  const write = (value: string) => {
    if (!value) {
      return;
    }
    output += `${"  ".repeat(indent)}${value}\n`;
  };
  const normalize = (value: string, declaration = false) => {
    const compact = value.trim().replace(/\s+/g, " ");
    if (
      !declaration ||
      compact.startsWith("@") ||
      compact.startsWith("&") ||
      compact.startsWith(":")
    ) {
      return compact;
    }
    const colon = compact.indexOf(":");
    return colon === -1
      ? compact
      : `${compact.slice(0, colon).trim()}: ${compact.slice(colon + 1).trim()}`;
  };

  for (const character of source) {
    if (quote) {
      token += character;
      if (character === quote && token.at(-2) !== "\\") {
        quote = "";
      }
      continue;
    }
    if (character === '"' || character === "'") {
      quote = character;
      token += character;
      continue;
    }
    if (character === "(") {
      parentheses += 1;
    }
    if (character === ")") {
      parentheses = Math.max(0, parentheses - 1);
    }
    if (parentheses === 0 && character === "{") {
      write(`${normalize(token)} {`);
      token = "";
      indent += 1;
    } else if (parentheses === 0 && character === ";") {
      write(`${normalize(token, true)};`);
      token = "";
    } else if (parentheses === 0 && character === "}") {
      if (token.trim()) {
        write(normalize(token, true));
      }
      token = "";
      indent = Math.max(0, indent - 1);
      write("}");
    } else {
      token += character;
    }
  }
  if (token.trim()) {
    write(normalize(token, true));
  }
  return output.trim();
}

function CodeEditor({
  value,
  onChange,
  extensions,
  selection,
}: {
  value: string;
  onChange(this: void, value: string): void;
  extensions: Extension;
  selection: { start: number; end: number } | null;
}) {
  const host = useRef<HTMLDivElement>(null);
  const view = useRef<EditorView | null>(null);
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;
  useEffect(() => {
    if (!host.current) {
      return undefined;
    }
    view.current = new EditorView({
      state: EditorState.create({
        doc: value,
        extensions: [
          oneDark,
          extensions,
          EditorView.lineWrapping,
          EditorView.updateListener.of((update) => {
            if (update.docChanged) {
              onChangeRef.current(update.state.doc.toString());
            }
          }),
        ],
      }),
      parent: host.current,
    });
    return () => {
      view.current?.destroy();
      view.current = null;
    };
  }, [extensions]);
  useEffect(() => {
    const editor = view.current;
    if (!editor || editor.state.doc.toString() === value) {
      return;
    }
    editor.dispatch({ changes: { from: 0, to: editor.state.doc.length, insert: value } });
  }, [value]);
  useEffect(() => {
    const editor = view.current;
    if (!editor || !selection) {
      return;
    }
    editor.dispatch({
      selection: EditorSelection.range(selection.start, selection.end),
      scrollIntoView: true,
    });
    editor.focus();
  }, [selection]);
  return <div className="code-editor" ref={host} />;
}

function Play() {
  const [source, setSource] = useState(initialSource);
  const [theme, setTheme] = useState(initialTheme);
  const [fileTab, setFileTab] = useState<FileTab>("component");
  const [outputTab, setOutputTab] = useState<OutputTab>("css");
  const [viewport, setViewport] = useState<Viewport>("desktop");
  const [result, setResult] = useState<Result | null>(null);
  const [status, setStatus] = useState("Compiling");
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [selection, setSelection] = useState<{ start: number; end: number } | null>(null);
  const frame = useRef<HTMLIFrameElement>(null);
  const requestId = useRef(0);
  const tsxLanguage = useMemo(() => javascript({ jsx: true, typescript: true }), []);
  const cssLanguage = useMemo(() => css(), []);

  useEffect(() => {
    const controller = new AbortController();
    const currentRequest = ++requestId.current;
    setStatus("Compiling");
    const timer = window.setTimeout(async () => {
      try {
        const next = await requestCompilation(source, theme, controller.signal);
        if (requestId.current !== currentRequest) {
          return;
        }
        setResult(next);
        setPreviewError(null);
        setStatus(next.diagnostics.length ? "Diagnostics" : "Ready");
        if (next.diagnostics.length) {
          setOutputTab("diagnostics");
        }
      } catch (error) {
        if (controller.signal.aborted || requestId.current !== currentRequest) {
          return;
        }
        setResult(null);
        setPreviewError(error instanceof Error ? error.message : String(error));
        setStatus("Error");
      }
    }, 280);
    return () => {
      controller.abort();
      window.clearTimeout(timer);
    };
  }, [source, theme]);

  useEffect(() => {
    const receiveError = (event: MessageEvent<{ type?: string; message?: string }>) => {
      if (
        event.source !== frame.current?.contentWindow ||
        event.data?.type !== "stillwind-preview-error"
      ) {
        return;
      }
      setPreviewError(event.data.message ?? "Preview execution failed");
      setStatus("Runtime error");
    };
    window.addEventListener("message", receiveError);
    return () => window.removeEventListener("message", receiveError);
  }, []);

  const diagnostics = result?.diagnostics ?? [];
  const output =
    outputTab === "css"
      ? result?.css && formatCss(result.css)
      : outputTab === "transform"
        ? result?.code
        : null;
  const viewportLabel = { desktop: "Desktop", tablet: "Tablet", mobile: "Mobile" }[viewport];
  const navigateToDiagnostic = (diagnostic: Result["diagnostics"][number]) => {
    setFileTab(diagnostic.uri === "stillwind-input:theme" ? "theme" : "component");
    setSelection({ start: diagnostic.start_utf16, end: diagnostic.end_utf16 });
  };
  return (
    <main className="play-shell">
      <header className="topbar">
        <a className="brand" href="/" aria-label="Stillwind Play home">
          stillwind<span>Play</span>
        </a>
        <div className={`compile-state ${status === "Ready" ? "is-ready" : ""}`}>
          <i />
          {status}
        </div>
        <div className="topbar-actions">
          <button type="button" onClick={() => setSource(initialSource)}>
            Reset
          </button>
          <button
            type="button"
            className="share"
            onClick={() => navigator.clipboard.writeText(location.href)}
          >
            Copy link
          </button>
        </div>
      </header>
      <section className="workspace">
        <aside className="files" aria-label="Playground files">
          <p>FILES</p>
          <button
            className={fileTab === "component" ? "active" : ""}
            type="button"
            onClick={() => {
              setSelection(null);
              setFileTab("component");
            }}
          >
            Component.tsx
          </button>
          <button
            className={fileTab === "theme" ? "active" : ""}
            type="button"
            onClick={() => {
              setSelection(null);
              setFileTab("theme");
            }}
          >
            stillwind.css
          </button>
          <small>React TSX · CSS-first theme</small>
        </aside>
        <section className="editor-panel">
          <div className="panel-bar">
            <strong>{fileTab === "component" ? "Component.tsx" : "stillwind.css"}</strong>
            <span>Reactive compiler</span>
          </div>
          {fileTab === "component" ? (
            <CodeEditor
              value={source}
              onChange={setSource}
              extensions={tsxLanguage}
              selection={selection}
            />
          ) : (
            <CodeEditor
              value={theme}
              onChange={setTheme}
              extensions={cssLanguage}
              selection={selection}
            />
          )}
        </section>
        <section className="preview-panel">
          <div className="panel-bar preview-bar">
            <strong>Preview</strong>
            <div className="viewport-switcher" aria-label="Preview viewport">
              {(["desktop", "tablet", "mobile"] as const).map((item) => (
                <button
                  className={viewport === item ? "active" : ""}
                  type="button"
                  onClick={() => setViewport(item)}
                  key={item}
                >
                  {item[0].toUpperCase()}
                </button>
              ))}
            </div>
            <span>{viewportLabel}</span>
          </div>
          <div className={`preview-stage ${viewport}`}>
            {result?.previewUrl && !result.diagnostics.length ? (
              <iframe
                key={result.previewUrl}
                ref={frame}
                title="Stillwind Play preview"
                sandbox="allow-scripts"
                src={result.previewUrl}
              />
            ) : (
              <div className="preview-empty">
                {previewError ?? "Fix diagnostics to render the preview."}
              </div>
            )}
          </div>
          {previewError ? <div className="runtime-error">Runtime: {previewError}</div> : null}
        </section>
        <section className="output-panel">
          <div className="output-tabs">
            {(["css", "transform", "diagnostics"] as const).map((item) => (
              <button
                type="button"
                key={item}
                onClick={() => setOutputTab(item)}
                className={outputTab === item ? "active" : ""}
              >
                {item === "css"
                  ? "Generated CSS"
                  : item === "transform"
                    ? "Macro output"
                    : `Diagnostics${diagnostics.length ? ` (${diagnostics.length})` : ""}`}
              </button>
            ))}
          </div>
          {outputTab === "diagnostics" ? (
            <div className="diagnostics-output" aria-live="polite">
              {!result ? (
                <p className="output-empty">Compiling diagnostics…</p>
              ) : diagnostics.length === 0 ? (
                <p className="output-empty">
                  <strong>No diagnostics</strong>
                  <span>The current component and theme compile without issues.</span>
                </p>
              ) : (
                <table>
                  <thead>
                    <tr>
                      <th scope="col">Code</th>
                      <th scope="col">Message</th>
                      <th scope="col">Range</th>
                    </tr>
                  </thead>
                  <tbody>
                    {diagnostics.map((item) => (
                      <tr key={`${item.code}-${item.start}-${item.end}`}>
                        <td>
                          <code>{item.code}</code>
                        </td>
                        <td>
                          {item.message}
                          {item.notes.map((note) => (
                            <small key={note}>{note}</small>
                          ))}
                        </td>
                        <td>
                          <button
                            className="diagnostic-jump"
                            type="button"
                            onClick={() => navigateToDiagnostic(item)}
                          >
                            <code>
                              {item.uri}:{item.start_line + 1}:{item.start_column + 1}
                            </code>
                          </button>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
            </div>
          ) : (
            <pre>{output || "Compiling…"}</pre>
          )}
        </section>
      </section>
    </main>
  );
}

createRoot(document.getElementById("root")!).render(<Play />);
