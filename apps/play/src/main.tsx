import { useEffect, useState } from "react"
import { createRoot } from "react-dom/client"
import "./play.css"

const initialSource = `import { style } from "shorth/react";

declare const loading: boolean;
export const props = style(
  "inline-flex items-center rounded-md bg-accent px-4 py-2 text-white hover:bg-accent/90 md:px-4",
  loading && "opacity-60 cursor-wait",
  { "aria-busy": loading },
);`

const initialTheme = `@theme {
  --color-accent: oklch(.6 .2 250);
  --color-white: white;
  --spacing-2: .5rem;
  --spacing-4: 1rem;
  --radius-md: .375rem;
  --breakpoint-md: 48rem;
}`

type Result = {
  css: string
  code: string | null
  classes: Array<{ class_name: string }>
  diagnostics: Array<{ code: string; message: string }>
}

function Play() {
  const [source, setSource] = useState(initialSource)
  const [theme, setTheme] = useState(initialTheme)
  const [result, setResult] = useState<Result | null>(null)

  useEffect(() => {
    const controller = new AbortController()
    const timer = window.setTimeout(async () => {
      const response = await fetch("/__shorth/compile", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ source, theme }),
        signal: controller.signal,
      })
      if (response.ok) setResult(await response.json() as Result)
    }, 180)
    return () => { controller.abort(); window.clearTimeout(timer) }
  }, [source, theme])

  const className = result?.classes.map((item) => item.class_name).join(" ") ?? ""
  return (
    <main>
      <style>{result?.css}</style>
      <header><span>shorth</span><strong>Play</strong><p>CSS-first static compiler</p></header>
      <section className="editors">
        <label><span>Component.tsx</span><textarea value={source} onChange={(event) => setSource(event.target.value)} spellCheck={false} /></label>
        <label><span>shorth.css</span><textarea value={theme} onChange={(event) => setTheme(event.target.value)} spellCheck={false} /></label>
      </section>
      <section className="output">
        <article><h2>Preview</h2><div className="preview"><button className={className}>Save changes</button></div></article>
        <article><h2>Generated CSS</h2><pre>{result?.css ?? "Compiling…"}</pre></article>
        <article><h2>Macro output</h2><pre>{result?.code ?? "Compiling…"}</pre></article>
      </section>
      {result?.diagnostics.length ? <aside>{result.diagnostics.map((item) => `${item.code}: ${item.message}`).join("\n")}</aside> : null}
    </main>
  )
}

createRoot(document.getElementById("root")!).render(<Play />)
