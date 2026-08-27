import { StrictMode, useState } from "react"
import { createRoot } from "react-dom/client"
import { style } from "shorth/react"
import "./screen.css"

function SaveButton() {
  const [loading, setLoading] = useState(false)

  return (
    <button
      {...style(
        "inline-flex items-center rounded-md bg-accent px-4 py-2 text-white transition hover:bg-accent/90 focus-visible:opacity-60",
        loading && "opacity-60 cursor-wait",
        { "aria-busy": loading },
      )}
      onClick={() => setLoading((value) => !value)}
    >
      {loading ? "Saving…" : "Save changes"}
    </button>
  )
}

function App() {
  return (
    <main>
      <p className="eyebrow">Shorth × Vite</p>
      <h1>Static macro, native CSS.</h1>
      <p>Click the button to exercise a compile-time conditional branch.</p>
      <SaveButton />
    </main>
  )
}

createRoot(document.getElementById("root")!).render(
  <StrictMode><App /></StrictMode>,
)
