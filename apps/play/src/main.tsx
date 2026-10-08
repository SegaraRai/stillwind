import { useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import type * as vscode from "vscode";

import { ApplicationBuilder, type ApplicationBuild } from "./build/client.ts";
import { PreviewChannel, previewDocument } from "./preview/channel.ts";
import { PlaygroundWorkspace } from "./workspace/editor.ts";

import "./play.css";

type Issue = {
  uri: vscode.Uri;
  diagnostic: vscode.Diagnostic;
};

function Play() {
  const editor = useRef<HTMLDivElement>(null);
  const frame = useRef<HTMLIFrameElement>(null);
  const workspace = useRef<PlaygroundWorkspace>(undefined);
  const [files, setFiles] = useState<string[]>([]);
  const [active, setActive] = useState("/workspace/Component.tsx");
  const [status, setStatus] = useState("Starting language services");
  const [error, setError] = useState<string>();
  const [result, setResult] = useState<ApplicationBuild>();
  const [issues, setIssues] = useState<Issue[]>([]);
  const [output, setOutput] = useState<"css" | "code" | "diagnostics">("css");
  const [viewport, setViewport] = useState("desktop");

  useEffect(() => {
    let disposed = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let builder: ApplicationBuilder | undefined;
    let channel: PreviewChannel | undefined;
    let diagnostics: vscode.Disposable | undefined;
    let sequence = 0;
    let running = false;
    let dirty = false;
    const build = async () => {
      if (disposed || !builder || !workspace.current) {
        return;
      }
      if (running) {
        dirty = true;
        return;
      }
      running = true;
      dirty = false;
      const revision = ++sequence;
      setStatus("Compiling");
      try {
        const reload = await workspace.current.saveSettings();
        const next = await builder.build(reload);
        if (!disposed && !dirty) {
          setResult(next);
          setError(undefined);
          setStatus("Ready");
          channel?.update({ revision, code: next.code, css: next.css });
        }
      } catch (error) {
        if (!disposed && !dirty) {
          setError(error instanceof Error ? error.message : String(error));
          setStatus("Build error");
        }
      } finally {
        running = false;
        if (dirty && !disposed) {
          schedule();
        }
      }
    };
    const schedule = () => {
      dirty = true;
      clearTimeout(timer);
      timer = setTimeout(() => void build(), 300);
    };
    void (async () => {
      try {
        const current = await PlaygroundWorkspace.open();
        if (disposed) {
          current.dispose();
          return;
        }
        workspace.current = current;
        current.mount(editor.current!, () => {
          setFiles([...current.files.keys()]);
          schedule();
        });
        setFiles([...current.files.keys()]);
        const refreshDiagnostics = () =>
          setIssues(
            current.api.languages
              .getDiagnostics()
              .flatMap(([uri, values]) => values.map((diagnostic) => ({ uri, diagnostic }))),
          );
        diagnostics = current.api.languages.onDidChangeDiagnostics(refreshDiagnostics);
        channel = new PreviewChannel(frame.current!, (message) => {
          setError(message);
          setStatus("Runtime error");
        });
        const runtime = await fetch(
          new URL(import.meta.env.BASE_URL + "preview-runtime.js", location.origin),
        ).then((response) => response.text());
        frame.current!.srcdoc = previewDocument(runtime);
        builder = new ApplicationBuilder(current.api);
        await build();
        refreshDiagnostics();
      } catch (error) {
        if (!disposed) {
          setError(error instanceof Error ? error.message : String(error));
          setStatus("Startup error");
        }
      }
    })();
    return () => {
      disposed = true;
      clearTimeout(timer);
      diagnostics?.dispose();
      builder?.dispose();
      channel?.dispose();
      workspace.current?.dispose();
    };
  }, []);

  const select = (file: string, range?: vscode.Range) => {
    setActive(file);
    workspace.current?.select(file, range);
  };
  return (
    <main className="play-shell">
      <header className="topbar">
        <a className="brand" href={import.meta.env.BASE_URL}>
          stillwind<span>Play</span>
        </a>
        <div className={`compile-state ${status === "Ready" ? "is-ready" : ""}`} role="status">
          <i />
          {status}
        </div>
        <div className="topbar-actions">
          <button
            type="button"
            onClick={async () => {
              const name = window.prompt("New file path, e.g. components/Counter.tsx");
              if (!name) {
                return;
              }
              try {
                await workspace.current?.add(name);
                setFiles([...workspace.current!.files.keys()]);
                select("/workspace/" + name);
              } catch (error) {
                setError(String(error));
              }
            }}
          >
            Add file
          </button>
          <button type="button" onClick={() => location.reload()}>
            Reset
          </button>
        </div>
      </header>
      <section className="workspace">
        <aside className="files" aria-label="Playground files">
          <p>FILES</p>
          {files.map((file) => (
            <button
              key={file}
              type="button"
              className={file === active ? "active" : ""}
              onClick={() => select(file)}
            >
              {file.replace("/workspace/", "")}
            </button>
          ))}
          <small>React · TypeScript · Stillwind</small>
        </aside>
        <section className="editor-panel">
          <div className="panel-bar">
            <strong>{active.replace("/workspace/", "")}</strong>
            <span>Monaco / VS Code</span>
            <button
              type="button"
              onClick={async () => {
                try {
                  await workspace.current?.format();
                } catch (error) {
                  setError(String(error));
                }
              }}
            >
              Format
            </button>
          </div>
          <div className="code-editor" ref={editor} />
        </section>
        <section className="preview-panel">
          <div className="panel-bar preview-bar">
            <strong>Preview</strong>
            <div className="viewport-switcher">
              {["desktop", "tablet", "mobile"].map((size) => (
                <button
                  key={size}
                  type="button"
                  className={viewport === size ? "active" : ""}
                  onClick={() => setViewport(size)}
                >
                  {size[0].toUpperCase()}
                </button>
              ))}
            </div>
            <span>React Refresh</span>
          </div>
          <div className={`preview-stage ${viewport}`}>
            <iframe ref={frame} title="Stillwind Play preview" sandbox="allow-scripts" />
          </div>
          {error && (
            <div className="runtime-error" role="alert">
              {error}
            </div>
          )}
        </section>
        <section className="output-panel">
          <div className="output-tabs">
            {(["css", "code", "diagnostics"] as const).map((tab) => (
              <button
                key={tab}
                type="button"
                className={output === tab ? "active" : ""}
                onClick={() => setOutput(tab)}
              >
                {tab === "css"
                  ? "Generated CSS"
                  : tab === "code"
                    ? "Macro output"
                    : `Diagnostics (${issues.length})`}
              </button>
            ))}
          </div>
          {output !== "diagnostics" ? (
            <pre>
              {output === "css"
                ? result?.css
                : result?.transformed.map(([file, code]) => `// ${file}\n${code}`).join("\n\n")}
            </pre>
          ) : (
            <div className="diagnostics-output">
              {issues.length === 0 ? (
                <p>No diagnostics</p>
              ) : (
                issues.map(({ uri, diagnostic }, index) => (
                  <button
                    type="button"
                    className="diagnostic-jump"
                    key={index}
                    onClick={() => select(uri.path, diagnostic.range)}
                  >
                    <code>
                      {uri.path.replace("/workspace/", "")}:{diagnostic.range.start.line + 1} ·{" "}
                      {diagnostic.source}{" "}
                      {typeof diagnostic.code === "object"
                        ? diagnostic.code.value
                        : diagnostic.code}
                    </code>{" "}
                    {diagnostic.message}
                  </button>
                ))
              )}
            </div>
          )}
        </section>
      </section>
    </main>
  );
}

createRoot(document.getElementById("root")!).render(<Play />);
