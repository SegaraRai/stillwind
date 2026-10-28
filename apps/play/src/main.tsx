import { useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { style } from "stillwind/react";
import type * as vscode from "vscode";

import { ApplicationBuilder, type ApplicationBuild } from "./build/client.ts";
import { fileIcon, getFileInfo } from "./file-info.ts";
import { PreviewChannel, previewDocument } from "./preview/channel.ts";
import { Explorer } from "./workbench/explorer.tsx";
import { countProblems, ProblemsPanel, type Problem } from "./workbench/problems.tsx";
import { ResultPane, selectOutput, type ResultTab, type Viewport } from "./workbench/results.tsx";
import { Sash } from "./workbench/sash.tsx";
import { PlaygroundWorkspace } from "./workspace/editor.ts";
import { formatSource } from "./workspace/formatter.ts";
import { OutputView } from "./workspace/output.ts";

import "@fontsource/instrument-sans/400.css";
import "@fontsource/instrument-sans/500.css";
import "@fontsource/instrument-sans/600.css";
import "./styles.css";

function Play() {
  const editor = useRef<HTMLDivElement>(null);
  const frame = useRef<HTMLIFrameElement>(null);
  const outputHost = useRef<HTMLDivElement>(null);
  const workspace = useRef<PlaygroundWorkspace>(undefined);
  const outputView = useRef<OutputView>(undefined);
  const [outputReady, setOutputReady] = useState(false);
  const [fixable, setFixable] = useState(false);
  const [files, setFiles] = useState<string[]>([]);
  const [active, setActive] = useState("/workspace/Component.tsx");
  const [status, setStatus] = useState("Starting language services");
  const [error, setError] = useState<string>();
  const [result, setResult] = useState<ApplicationBuild>();
  const [problems, setProblems] = useState<Problem[]>([]);
  const [tab, setTab] = useState<ResultTab>("preview");
  const [viewport, setViewport] = useState<Viewport>("desktop");
  const [sidebar, setSidebar] = useState(224);
  const [results, setResults] = useState(() => Math.max(480, Math.round(window.innerWidth * 0.38)));
  const [panel, setPanel] = useState(176);

  const output = tab === "preview" ? undefined : selectOutput(tab, active, result);

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
        outputView.current = new OutputView(outputHost.current!);
        setOutputReady(true);
        const refreshDiagnostics = () =>
          setProblems(
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
      outputView.current?.dispose();
      workspace.current?.dispose();
    };
  }, []);

  useEffect(() => {
    if (!output || !outputReady) {
      return;
    }
    let current = true;
    void (async () => {
      // The compiler emits compact CSS, so the view shows it formatted with Prettier.
      const text =
        output.generated && output.language === "css"
          ? await formatSource(output.text, "css", { insertSpaces: true, tabSize: 2 }).catch(
              () => output.text,
            )
          : output.text;
      if (current) {
        await outputView.current?.show(output.id, output.language, text);
      }
    })();
    return () => {
      current = false;
    };
  }, [output?.id, output?.language, output?.text, output?.generated, outputReady]);

  // Diagnostics change after edits and fixes, so they also signal changes in the fix actions.
  useEffect(() => {
    let current = true;
    void workspace.current
      ?.fixes(active)
      .then((fixes) => current && setFixable(fixes.length > 0))
      .catch(() => current && setFixable(false));
    return () => {
      current = false;
    };
  }, [active, problems]);

  const select = (file: string, range?: vscode.Range) => {
    setActive(file);
    workspace.current?.select(file, range);
  };

  const add = async (name: string) => {
    await workspace.current?.add(name);
    setFiles([...workspace.current!.files.keys()]);
    select("/workspace/" + name.replace(/^\/+/, ""));
  };

  const { errors, warnings } = countProblems(problems);
  const relative = active.replace("/workspace/", "");
  const segments = ["workspace", ...relative.split("/")];

  return (
    <div {...style("grid block-full grid-rows-[auto_minmax(0,1fr)_auto]")}>
      <header
        {...style("flex px-3 block-10 items-center gap-3 bg-background border-divider border-be")}
      >
        <span
          {...style("flex px-1 items-center gap-2 text-foreground rounded-control select-none")}
        >
          <span aria-label="Stillwind" role="img" {...style("text-heading-small stillwind-logo")} />
          <span
            {...style(
              "px-1.5 py-px text-action-foreground text-label font-semibold tracking-[0.08em] uppercase bg-action rounded-inline",
            )}
          >
            Play
          </span>
        </span>
        <button
          type="button"
          onClick={() => location.reload()}
          {...style("ms-auto text-button select-none")}
        >
          <span aria-hidden="true" {...style("icon-inline icon-lucide--rotate-ccw")} />
          {"Reset"}
        </button>
      </header>
      <div
        {...style(
          "grid min-block-0 grid-cols-[minmax(0,var(--sidebar))_minmax(0,1fr)]",
          "max-inline:grid-cols-1 max-inline:grid-rows-[auto_minmax(0,1fr)]",
          { "--sidebar": `${sidebar}px` },
        )}
      >
        <div
          {...style(
            "grid min-block-0 min-inline-0",
            "max-inline:max-block-36 max-inline:border-divider max-inline:border-be",
          )}
        >
          <Explorer files={files} active={active} onSelect={select} onAdd={add} />
        </div>
        <div
          {...style(
            "relative grid min-block-0 min-inline-0 grid-rows-[minmax(0,1fr)_var(--panel)] border-divider border-s",
            "max-workbench:grid-rows-[minmax(0,1fr)_10rem] max-inline:border-s-0",
            { "--panel": `${panel}px` },
          )}
        >
          <Sash
            label="Resize explorer"
            orientation="vertical"
            value={sidebar}
            min={160}
            max={480}
            direction={1}
            onChange={setSidebar}
          />
          <div
            {...style(
              "grid min-block-0 grid-cols-[minmax(0,1fr)_minmax(20rem,min(var(--results),calc(100%-20rem)))]",
              "max-workbench:grid-cols-1 max-workbench:grid-rows-2",
              { "--results": `${results}px` },
            )}
          >
            <section
              aria-label="Editor"
              {...style("flex min-block-0 min-inline-0 flex-col bg-editor")}
            >
              <div
                {...style(
                  "flex block-9 shrink-0 items-stretch bg-background border-divider border-be",
                )}
              >
                <div
                  role="tablist"
                  aria-label="Open editors"
                  {...style("flex items-stretch select-none")}
                >
                  <div role="tab" aria-selected="true" {...style("cursor-default workbench-tab")}>
                    <span
                      data-type={getFileInfo(active).type}
                      {...style(fileIcon, "icon-control shrink-0")}
                    />
                    {segments.at(-1)}
                  </div>
                </div>
                <div {...style("flex px-2 flex-1 items-center justify-end gap-1")}>
                  <button
                    type="button"
                    onClick={async () => {
                      try {
                        await workspace.current?.format();
                      } catch (error) {
                        setError(String(error));
                      }
                    }}
                    {...style("text-button select-none")}
                  >
                    <span aria-hidden="true" {...style("icon-inline icon-lucide--wand-sparkles")} />
                    {"Format"}
                  </button>
                  <button
                    type="button"
                    title="Apply Stillwind lint fixes to this file"
                    disabled={!fixable}
                    onClick={async () => {
                      try {
                        await workspace.current?.fix(active);
                      } catch (error) {
                        setError(String(error));
                      }
                    }}
                    {...style("text-button select-none")}
                  >
                    <span aria-hidden="true" {...style("icon-inline icon-lucide--wrench")} />
                    {"Fix"}
                  </button>
                </div>
              </div>
              <nav
                aria-label="Breadcrumb"
                {...style(
                  "flex px-3 block-6 shrink-0 items-center gap-0.5 text-caption text-muted select-none",
                )}
              >
                {segments.map((segment, index) => (
                  <span key={index} {...style("flex items-center gap-1")}>
                    {index > 0 && (
                      <span
                        aria-hidden="true"
                        {...style("icon-inline icon-lucide--chevron-right text-subtle")}
                      />
                    )}
                    {index === segments.length - 1 && (
                      <span
                        data-type={getFileInfo(segment).type}
                        {...style(fileIcon, "icon-control shrink-0")}
                      />
                    )}
                    <span {...style(index === segments.length - 1 && "text-foreground")}>
                      {segment}
                    </span>
                  </span>
                ))}
              </nav>
              <div ref={editor} {...style("min-block-0 flex-1")} />
            </section>
            <div
              {...style(
                "relative grid min-block-0 min-inline-0 border-divider border-s",
                "max-workbench:border-s-0 max-workbench:border-bs",
              )}
            >
              <Sash
                label="Resize result pane"
                orientation="vertical"
                value={results}
                min={320}
                max={1200}
                direction={-1}
                onChange={setResults}
              />
              <ResultPane
                tab={tab}
                onTab={setTab}
                viewport={viewport}
                onViewport={setViewport}
                frame={frame}
                outputHost={outputHost}
                output={output}
                error={error}
              />
            </div>
          </div>
          <div {...style("relative grid min-block-0")}>
            <Sash
              label="Resize problems panel"
              orientation="horizontal"
              value={panel}
              min={96}
              max={600}
              direction={-1}
              onChange={setPanel}
            />
            <ProblemsPanel problems={problems} onSelect={select} />
          </div>
        </div>
      </div>
      <footer
        {...style(
          "flex block-6 items-stretch text-caption text-muted bg-background border-divider border-bs select-none",
        )}
      >
        <span role="status" {...style("flex px-3 items-center gap-1.5")}>
          <span
            aria-hidden="true"
            {...style(
              "icon-inline",
              status === "Ready"
                ? "icon-lucide--check text-success"
                : status.endsWith("error")
                  ? "icon-lucide--circle-x text-danger"
                  : "icon-lucide--loader-circle text-accent animate-spin",
            )}
          />
          {status}
        </span>
        <span
          aria-label={`${errors} errors, ${warnings} warnings`}
          {...style("flex px-2 items-center gap-1")}
        >
          <span aria-hidden="true" {...style("icon-inline icon-lucide--circle-x")} />
          {errors}
          <span aria-hidden="true" {...style("icon-inline icon-lucide--triangle-alert ms-1")} />
          {warnings}
        </span>
        <span {...style("flex ms-auto px-3 items-center max-inline:hidden")}>
          {getFileInfo(active).label}
        </span>
      </footer>
    </div>
  );
}

createRoot(document.getElementById("root")!).render(<Play />);
