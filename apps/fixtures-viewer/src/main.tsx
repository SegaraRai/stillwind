import { lazy, Suspense, useEffect, useMemo, useRef, useState } from "react";
import { createRoot } from "react-dom/client";

import "./viewer.css";

type FixtureFile = {
  path: string;
  language: "typescript" | "tsx";
  before: string;
  after: string;
  css: string;
  minifiedCss: string;
  diff?: Array<{ fromA: number; toA: number; fromB: number; toB: number }>;
};
type FixtureProject = {
  id: string;
  title: string;
  description: string;
  tags: string[];
  css: string;
  minifiedCss: string;
  files: FixtureFile[];
};
type Catalog = { schemaVersion: 1; projects: FixtureProject[] };

const Code = lazy(() => import("./CodeView"));
const Diff = lazy(() => import("./DiffView"));

function parseRoute(catalog: Catalog) {
  const match = location.hash.match(/^#\/projects\/([^/]+)\/files\/(.+)$/);
  const project =
    catalog.projects.find((item) => item.id === decodeURIComponent(match?.[1] ?? "")) ??
    catalog.projects[0];
  const file =
    project?.files.find((item) => item.path === decodeURIComponent(match?.[2] ?? "")) ??
    project?.files[0];
  return { project, file };
}

function href(project: FixtureProject, file: FixtureFile) {
  return `#/projects/${encodeURIComponent(project.id)}/files/${encodeURIComponent(file.path)}`;
}

function symbols(source: string) {
  return [
    ...source.matchAll(/\b(?:class|function|interface|type|const|let|var)\s+([A-Za-z_$][\w$]*)/g),
  ]
    .map((match) => match[1])
    .filter((value, index, all) => all.indexOf(value) === index);
}

function App() {
  const [catalog, setCatalog] = useState<Catalog | null>(null);
  const [routeVersion, setRouteVersion] = useState(0);
  const [query, setQuery] = useState("");
  const [cssScope, setCssScope] = useState<"module" | "project">("module");
  const [minified, setMinified] = useState(false);
  const [copied, setCopied] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const search = useRef<HTMLInputElement>(null);

  useEffect(() => {
    void fetch("./fixtures.json")
      .then((response) => {
        if (!response.ok) {
          throw new Error(`catalog request failed (${response.status})`);
        }
        return response.json() as Promise<Catalog>;
      })
      .then(setCatalog)
      .catch((error: unknown) =>
        setLoadError(error instanceof Error ? error.message : "Unable to load fixture catalog"),
      );
  }, []);
  useEffect(() => {
    const update = () => setRouteVersion((value) => value + 1);
    addEventListener("hashchange", update);
    return () => removeEventListener("hashchange", update);
  }, []);
  useEffect(() => {
    const focusSearch = (event: KeyboardEvent) => {
      if (event.key !== "/" || event.metaKey || event.ctrlKey || event.altKey) {
        return;
      }
      if ((event.target as HTMLElement).matches("input, textarea, [contenteditable=true]")) {
        return;
      }
      event.preventDefault();
      search.current?.focus();
    };
    addEventListener("keydown", focusSearch);
    return () => removeEventListener("keydown", focusSearch);
  }, []);

  const route = useMemo(() => (catalog ? parseRoute(catalog) : null), [catalog, routeVersion]);
  useEffect(() => {
    if (!route?.project || !route.file) {
      return;
    }
    const canonical = href(route.project, route.file);
    if (location.hash !== canonical) {
      history.replaceState(null, "", canonical);
    }
  }, [route]);
  const results = useMemo(() => {
    if (!catalog) {
      return [];
    }
    const needle = query.trim().toLocaleLowerCase();
    return catalog.projects.flatMap((project) =>
      project.files
        .filter((file) => {
          if (!needle) {
            return true;
          }
          return [
            project.title,
            project.id,
            project.tags.join(" "),
            file.path,
            ...symbols(file.before),
            file.before,
          ]
            .join("\n")
            .toLocaleLowerCase()
            .includes(needle);
        })
        .map((file) => ({ project, file })),
    );
  }, [catalog, query]);

  if (loadError) {
    return <main className="loading">{loadError}</main>;
  }
  if (!catalog || !route?.project || !route.file) {
    return <main className="loading">Loading fixture catalog…</main>;
  }
  const { project, file } = route;
  const cssSource = cssScope === "module" ? file : project;
  const cssOutput =
    (minified ? cssSource.minifiedCss : cssSource.css).trim() ||
    "/* No generated CSS in this selection. */\n";

  return (
    <div className="shell">
      <header>
        <a className="brand" href={href(project, file)} aria-label="Stillwind Fixture Review home">
          <span className="brand-mark">S</span>
          <span>
            <b>Stillwind</b>
            <small>Fixture Review</small>
          </span>
        </a>
        <label className="search">
          <span>⌕</span>
          <input
            ref={search}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Search files, classes, symbols…"
          />
          <kbd>/</kbd>
        </label>
        <button
          className="copy"
          onClick={() => {
            void navigator.clipboard.writeText(location.href);
            setCopied(true);
            setTimeout(() => setCopied(false), 1400);
          }}
        >
          {copied ? "Copied" : "Copy permalink"}
        </button>
      </header>

      <aside>
        <div className="aside-label">
          Projects <span>{catalog.projects.length}</span>
        </div>
        {catalog.projects.map((item) => (
          <section key={item.id}>
            <div className="project-title">{item.title}</div>
            {results
              .filter((result) => result.project.id === item.id)
              .map((result) => (
                <a
                  className={
                    result.project.id === project.id && result.file.path === file.path
                      ? "active"
                      : ""
                  }
                  href={href(result.project, result.file)}
                  key={result.file.path}
                >
                  <span className="file-icon">TS</span>
                  <span>{result.file.path}</span>
                </a>
              ))}
          </section>
        ))}
        {results.length === 0 && <p className="empty">No fixture matches “{query}”.</p>}
      </aside>

      <main>
        <div className="context">
          <div>
            <div className="eyebrow">
              {project.id} / {file.path}
            </div>
            <h1>{project.title}</h1>
            <p>{project.description}</p>
          </div>
          <div className="tags">
            {project.tags.map((tag) => (
              <span key={tag}>{tag}</span>
            ))}
          </div>
        </div>
        <div className="compare-head">
          <div>
            <span className="status before" />
            Before <small>{file.path}</small>
          </div>
          <div>
            <span className="status after" />
            After
          </div>
        </div>
        <div className="compare">
          <Suspense fallback={<div className="code-skeleton">Computing diff…</div>}>
            <Diff
              before={file.before}
              after={file.after}
              language={file.language}
              changes={file.diff}
            />
          </Suspense>
        </div>
        <div className="css-head">
          <div>
            <span className="status css" />
            Generated CSS
            <small>{cssScope === "module" ? file.path : project.id}</small>
          </div>
          <div className="css-controls">
            <label
              className="minify-toggle"
              title="Minify with Lightning CSS, then format for review"
            >
              <input
                type="checkbox"
                checked={minified}
                onChange={(event) => setMinified(event.target.checked)}
              />
              Minified (formatted)
            </label>
            <nav aria-label="CSS scope">
              <button
                className={cssScope === "module" ? "selected" : ""}
                aria-pressed={cssScope === "module"}
                onClick={() => setCssScope("module")}
              >
                Module CSS
              </button>
              <button
                className={cssScope === "project" ? "selected" : ""}
                aria-pressed={cssScope === "project"}
                onClick={() => setCssScope("project")}
              >
                Project CSS
              </button>
            </nav>
          </div>
        </div>
        <div className="css-panel">
          <Suspense fallback={<div className="code-skeleton">Loading source…</div>}>
            <Code value={cssOutput} language="css" />
          </Suspense>
        </div>
      </main>
    </div>
  );
}

const root = import.meta.hot?.data.root ?? createRoot(document.getElementById("root")!);
if (import.meta.hot) {
  import.meta.hot.data.root = root;
}
root.render(<App />);
