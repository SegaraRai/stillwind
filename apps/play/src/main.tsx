import { EditorState, type Extension } from "@codemirror/state"
import { css } from "@codemirror/lang-css"
import { javascript } from "@codemirror/lang-javascript"
import { oneDark } from "@codemirror/theme-one-dark"
import { EditorView } from "@codemirror/view"
import { useEffect, useMemo, useRef, useState } from "react"
import { createRoot } from "react-dom/client"
import "./play.css"

const initialSource = `import { useState } from "react";
import { style } from "shorth/react";

export function App() {
  const [loading, setLoading] = useState(false);
  return (
    <main {...style("bg-surface p-4 rounded-md")}>
      <p>Shorth Play</p>
      <h1>React, compiled as you type.</h1>
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
    </main>
  );
}`

const initialTheme = `@theme {
  --color-accent: oklch(.6 .2 250);
  --color-white: white;
  --color-surface: oklch(.97 .01 250);
  --spacing-2: .5rem;
  --spacing-4: 1rem;
  --radius-md: .5rem;
  --breakpoint-md: 48rem;
}`

type Result = {
  css: string
  code: string | null
  diagnostics: Array<{ code: string; message: string; start: number; end: number }>
  previewUrl: string | null
}
type FileTab = "component" | "theme"
type OutputTab = "css" | "transform" | "diagnostics"
type Viewport = "desktop" | "tablet" | "mobile"

function CodeEditor({ value, onChange, extensions }: { value: string; onChange(value: string): void; extensions: Extension }) {
  const host = useRef<HTMLDivElement>(null)
  const view = useRef<EditorView | null>(null)
  const onChangeRef = useRef(onChange)
  onChangeRef.current = onChange
  useEffect(() => {
    if (!host.current) return undefined
    view.current = new EditorView({
      state: EditorState.create({ doc: value, extensions: [oneDark, extensions, EditorView.lineWrapping, EditorView.updateListener.of((update) => {
        if (update.docChanged) onChangeRef.current(update.state.doc.toString())
      })] }),
      parent: host.current,
    })
    return () => { view.current?.destroy(); view.current = null }
  }, [extensions])
  useEffect(() => {
    const editor = view.current
    if (!editor || editor.state.doc.toString() === value) return
    editor.dispatch({ changes: { from: 0, to: editor.state.doc.length, insert: value } })
  }, [value])
  return <div className="code-editor" ref={host} />
}

function Play() {
  const [source, setSource] = useState(initialSource)
  const [theme, setTheme] = useState(initialTheme)
  const [fileTab, setFileTab] = useState<FileTab>("component")
  const [outputTab, setOutputTab] = useState<OutputTab>("css")
  const [viewport, setViewport] = useState<Viewport>("desktop")
  const [result, setResult] = useState<Result | null>(null)
  const [status, setStatus] = useState("Compiling")
  const [previewError, setPreviewError] = useState<string | null>(null)
  const frame = useRef<HTMLIFrameElement>(null)
  const requestId = useRef(0)
  const tsxLanguage = useMemo(() => javascript({ jsx: true, typescript: true }), [])
  const cssLanguage = useMemo(() => css(), [])

  useEffect(() => {
    const controller = new AbortController()
    const currentRequest = ++requestId.current
    setStatus("Compiling")
    const timer = window.setTimeout(async () => {
      try {
        const response = await fetch("/__shorth/compile", { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ source, theme }), signal: controller.signal })
        const next = await response.json() as Result
        if (!response.ok) throw new Error("error" in next ? String(next.error) : "Preview compilation failed")
        if (requestId.current !== currentRequest) return
        setResult(next); setPreviewError(null); setStatus(next.diagnostics.length ? "Diagnostics" : "Ready")
        if (next.diagnostics.length) setOutputTab("diagnostics")
      } catch (error) {
        if (controller.signal.aborted || requestId.current !== currentRequest) return
        setResult(null); setPreviewError(error instanceof Error ? error.message : String(error)); setStatus("Error")
      }
    }, 280)
    return () => { controller.abort(); window.clearTimeout(timer) }
  }, [source, theme])

  useEffect(() => {
    const receiveError = (event: MessageEvent<{ type?: string; message?: string }>) => {
      if (event.source !== frame.current?.contentWindow || event.data?.type !== "shorth-preview-error") return
      setPreviewError(event.data.message ?? "Preview execution failed"); setStatus("Runtime error")
    }
    window.addEventListener("message", receiveError)
    return () => window.removeEventListener("message", receiveError)
  }, [])

  const output = outputTab === "css" ? result?.css : outputTab === "transform" ? result?.code : result?.diagnostics.map((item) => `${item.code}  ${item.message}`).join("\n")
  const viewportLabel = { desktop: "Desktop", tablet: "Tablet", mobile: "Mobile" }[viewport]
  return <main className="play-shell">
    <header className="topbar">
      <a className="brand" href="/" aria-label="Shorth Play home">shorth<span>Play</span></a>
      <div className={`compile-state ${status === "Ready" ? "is-ready" : ""}`}><i />{status}</div>
      <div className="topbar-actions"><button type="button" onClick={() => setSource(initialSource)}>Reset</button><button type="button" className="share" onClick={() => navigator.clipboard.writeText(location.href)}>Copy link</button></div>
    </header>
    <section className="workspace">
      <aside className="files" aria-label="Playground files"><p>FILES</p><button className={fileTab === "component" ? "active" : ""} type="button" onClick={() => setFileTab("component")}>Component.tsx</button><button className={fileTab === "theme" ? "active" : ""} type="button" onClick={() => setFileTab("theme")}>shorth.css</button><small>React TSX · CSS-first theme</small></aside>
      <section className="editor-panel"><div className="panel-bar"><strong>{fileTab === "component" ? "Component.tsx" : "shorth.css"}</strong><span>Reactive compiler</span></div>{fileTab === "component" ? <CodeEditor value={source} onChange={setSource} extensions={tsxLanguage} /> : <CodeEditor value={theme} onChange={setTheme} extensions={cssLanguage} />}</section>
      <section className="preview-panel"><div className="panel-bar preview-bar"><strong>Preview</strong><div className="viewport-switcher" aria-label="Preview viewport">{(["desktop", "tablet", "mobile"] as const).map((item) => <button className={viewport === item ? "active" : ""} type="button" onClick={() => setViewport(item)} key={item}>{item[0].toUpperCase()}</button>)}</div><span>{viewportLabel}</span></div><div className={`preview-stage ${viewport}`}>{result?.previewUrl && !result.diagnostics.length ? <iframe key={result.previewUrl} ref={frame} title="Shorth Play preview" sandbox="allow-scripts" src={result.previewUrl} /> : <div className="preview-empty">{previewError ?? "Fix diagnostics to render the preview."}</div>}</div>{previewError ? <div className="runtime-error">Runtime: {previewError}</div> : null}</section>
      <section className="output-panel"><div className="output-tabs">{(["css", "transform", "diagnostics"] as const).map((item) => <button type="button" key={item} onClick={() => setOutputTab(item)} className={outputTab === item ? "active" : ""}>{item === "css" ? "Generated CSS" : item === "transform" ? "Macro output" : `Diagnostics${result?.diagnostics.length ? ` (${result.diagnostics.length})` : ""}`}</button>)}</div><pre>{output || "Compiling…"}</pre></section>
    </section>
  </main>
}

createRoot(document.getElementById("root")!).render(<Play />)
