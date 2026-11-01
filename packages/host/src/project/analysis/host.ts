import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

import type { BaseConfig, PreviewOptions } from "@stillwind/config";
import {
  NativeProjectSession,
  type BindingReport,
  type NativeDesignSystemLoader,
} from "@stillwind/native";

import type { CompilerHost } from "../../compilation/index.ts";
import {
  attachPlugins,
  isStylesheet,
  loadCssGraph,
  loadEditorCssGraph,
  pluginLoader,
  type CssHost,
  type CssPluginInputs,
  type EditorCssGraph,
  type PluginRuntime,
} from "../../css/index.ts";
import type { DesignSystemContext } from "../../editor/index.ts";
import type { TypeFactsEnvironment } from "../../semantics/index.ts";
import { compilerOptions, cssEntrypoint } from "../options.ts";
import { StylesheetEditor } from "../stylesheet-editor.ts";

/**
 * Loads compiler inputs and CSS for one Node analysis project. AnalysisProject serializes calls
 * and installs captured document contents before asking this backend to resolve or load modules.
 */
export interface AnalysisHost extends CompilerHost {
  /** Absolute project directory used for identities and CSS paths. */
  root: string;
  /**
   * Compiler and preview settings fixed for this host's lifetime. Relative CSS and tsconfig paths
   * use root.
   */
  config: BaseConfig & {
    preview?: PreviewOptions;
  };
  /** CSS resolution and reads using the same captured document buffers as source loading. */
  css: CssHost;
  /** Fixed TypeScript settings selected when this host opens. */
  readonly typeFactsEnvironment: TypeFactsEnvironment;
  /** False disables additional semantic queries for this project. */
  readonly typeFacts?: false;
  /**
   * Checks whether a resolved target can be analyzed by this host.
   *
   * @param id - Full resolver identity of the target.
   * @returns Whether load can supply supported source for the identity.
   */
  supports(id: string): boolean;
  /**
   * Replaces all buffers used by subsequent source and CSS reads.
   *
   * @param documents - Absolute file paths to captured text. Missing entries use backend reads.
   * @returns A promise that resolves when subsequent reads can use the supplied snapshot.
   * @throws If the backend cannot install the supplied buffers.
   */
  setDocuments(documents: ReadonlyMap<string, string>): Promise<void>;
  /**
   * Discards backend caches before the next transaction. Must not overlap resolution or loads.
   *
   * @returns A promise that resolves when cached reads and resolutions have been invalidated.
   * @throws If backend invalidation fails.
   */
  invalidate(): Promise<void>;
  /**
   * Releases backend resources. The caller first waits for pending host operations.
   *
   * @returns A promise for cleanup completion.
   * @throws If backend cleanup fails.
   */
  close(): Promise<void>;
}

/**
 * Loads CSS and tracks the host dependencies that require a fresh design system.
 */
export class AnalysisCss {
  /** Files read by CSS or plugins. Open-buffer changes to these files reload CSS. */
  readonly #dependencies = new Set<string>();

  /** Delegates CSS operations to the host and records watched dependencies. */
  readonly #css: CssHost;

  /** CSS loader retained until a CSS update or explicit invalidation. */
  #loader?: NativeDesignSystemLoader;

  /** Session that uses the retained loader. Replaced when CSS changes. */
  #session?: NativeProjectSession;

  /** Query-only CSS graph, rebuilt after CSS changes and kept separate from strict analysis. */
  #editorCss?: EditorCssGraph;

  /** Identities that the retained loader marked for stylesheet lint, in load order. */
  #lintSources: string[] = [];

  /**
   * Plugin runtime of the retained loader. A CSS update keeps it as the previous runtime of the
   * next load, so definitions of unchanged plugins survive.
   */
  #plugins?: PluginRuntime;

  /** Host operations are called only inside the project's serialized transactions. */
  readonly #host: AnalysisHost;

  /**
   * Wraps CSS watching without loading files.
   * @param host - Host owned by the analysis project. Calls retain the host as their receiver.
   */
  constructor(host: AnalysisHost) {
    this.#host = host;
    this.#css = {
      resolve: (specifier, importer) => host.css.resolve(specifier, importer),
      read: (id) => host.css.read(id),
      documentUri: (id) => host.css.documentUri(id),
      sourceMapSource: (id) => host.css.sourceMapSource(id),
      plugins: host.css.plugins,
      lint: host.css.lint?.bind(host.css),
      watch: (id) => {
        this.#dependencies.add(id);
        host.css.watch(id);
      },
    };
  }

  /** Current strict loader, or undefined before loading or after invalidation. */
  get loader(): NativeDesignSystemLoader | undefined {
    return this.#loader;
  }

  /** Session that uses the strict loader, or undefined until load succeeds. */
  get session(): NativeProjectSession | undefined {
    return this.#session;
  }

  /** Plugin runtime of the latest successful load. Kept across CSS changes for reuse. */
  get plugins(): PluginRuntime | undefined {
    return this.#plugins;
  }

  /** Stylesheets marked for lint by the last strict load, in load order. */
  get lintSources(): readonly string[] {
    return this.#lintSources;
  }

  /**
   * Checks whether an open-buffer edit requires reloading CSS.
   *
   * @param id - Absolute path of the changed document.
   * @returns Whether it is a stylesheet or a dependency watched by CSS or a plugin.
   */
  affects(id: string): boolean {
    return isStylesheet(id) || [...this.#dependencies].some((file) => sameFile(file, id));
  }

  /** Returns the files that CSS or plugins have read. */
  get dependencies(): ReadonlySet<string> {
    return this.#dependencies;
  }

  /**
   * Discards loaders and the native session while keeping plugins available for the next load.
   */
  invalidate(): void {
    this.#loader = undefined;
    this.#session = undefined;
    this.#editorCss = undefined;
  }

  /**
   * Discards a failed plugin runtime and the loaders that depend on it before individual analysis retries.
   */
  discardPlugins(): void {
    this.invalidate();
    this.#plugins = undefined;
  }

  /**
   * Loads the configured CSS graph once until CSS invalidation clears it.
   *
   * @returns The retained loader with compiler options applied.
   * @throws If resolving, reading, or loading the CSS graph fails.
   */
  async load(): Promise<NativeDesignSystemLoader> {
    if (!this.#loader) {
      const css = path.resolve(this.#host.root, cssEntrypoint(this.#host.config));
      const loaded = await loadCssGraph(
        css,
        compilerOptions(this.#host.config),
        this.#css,
        undefined,
        undefined,
        this.#pluginInputs(),
      );
      this.#loader = loaded.loader;
      this.#lintSources = loaded.lintSources;
      this.#plugins = loaded.plugins;
      this.#session = new NativeProjectSession(
        this.#loader,
        false,
        "development",
        this.#host.config.naming,
      );
      attachPlugins(this.#session, this.#plugins, this.#plugins?.generation);
    }
    return this.#loader;
  }

  /**
   * Loads the design system for questions that read no source file.
   *
   * @returns A query over the definitions of the latest plugin load, and the plugin runtime.
   * @throws If resolving, reading, or loading the CSS graph fails.
   */
  async designSystem(): Promise<Pick<DesignSystemContext, "query" | "plugins">> {
    const loader = await this.load();
    return {
      query: (pluginLoader(this.#session!) ?? loader).editorQuery(),
      plugins: this.#plugins,
    };
  }

  /**
   * Selects the plugins of the project's CSS graph.
   *
   * @returns Configured plugins, the project root, and the runtime of the last load.
   */
  #pluginInputs(): CssPluginInputs {
    return {
      plugins: this.#host.config.plugins,
      tailwind: this.#host.config.tailwind,
      important: this.#host.config.important,
      root: this.#host.root,
      previous: this.#plugins,
    };
  }

  /**
   * Loads a query graph from original CSS or recovered definitions, without publishing its reports.
   *
   * @returns The cached editor graph, reusing the strict loader when available. Otherwise, a failed
   * original load is retried with recovered definitions. CSS invalidation clears this cache.
   * @throws If neither the original nor the recovered CSS graph can be resolved, read, and loaded.
   */
  async editor(): Promise<EditorCssGraph> {
    if (!this.#editorCss) {
      if (this.#loader) {
        this.#editorCss = {
          loader: this.#loader,
          resources: [],
          lintSources: this.#lintSources,
          recovered: false,
          plugins: this.#plugins,
        };
      } else {
        this.#editorCss = await loadEditorCssGraph(
          path.resolve(this.#host.root, cssEntrypoint(this.#host.config)),
          compilerOptions(this.#host.config),
          this.#css,
          this.#pluginInputs(),
        );
      }
    }
    return this.#editorCss;
  }

  /**
   * Creates a strict stylesheet editor and its lint reports.
   *
   * @param entry - Absolute stylesheet path in this host.
   * @returns Reports for that document and an editor using the same CSS definitions.
   * @throws If CSS loading, source reading, or plugin catalog loading fails.
   */
  async stylesheet(
    entry: string,
  ): Promise<{ reports: BindingReport[]; snapshot: StylesheetEditor }> {
    const loader = await this.load();
    const reports = loader.stylesheetReports().filter((report) => {
      const uri = report.primary.location.uri;
      return uri.startsWith("file:")
        ? sameFile(fileURLToPath(uri), entry)
        : uri === this.#host.css.documentUri(entry);
    });
    return {
      reports,
      snapshot: new StylesheetEditor(
        entry,
        await this.#host.css.read(entry),
        reports,
        loader.editorQuery(),
        this.#host.config,
        (await this.#plugins?.catalog()) ?? [],
        this.#plugins,
      ),
    };
  }

  /**
   * Creates an editor from original CSS, or from recovered definitions when strict loading fails.
   *
   * @param entry - Absolute stylesheet path. Files outside the linted CSS graph are skipped.
   * @returns The editor and whether recovery was needed, or undefined outside the graph.
   * @throws If CSS recovery, source reading, or plugin catalog loading fails.
   */
  async editorSnapshot(
    entry: string,
  ): Promise<{ snapshot: StylesheetEditor; recovered: boolean } | undefined> {
    const graph = await this.editor();
    if (!graph.lintSources.some((source) => sameFile(source, entry))) {
      return undefined;
    }
    const reports = graph.recovered
      ? []
      : graph.loader
          .stylesheetReports()
          .filter((report) => report.primary.location.uri === this.#host.css.documentUri(entry));
    return {
      recovered: graph.recovered,
      snapshot: new StylesheetEditor(
        entry,
        await this.#host.css.read(entry),
        reports,
        graph.loader.editorQuery(),
        this.#host.config,
        (await graph.plugins?.catalog()) ?? [],
        graph.plugins,
      ),
    };
  }
}

/**
 * Compares file paths as the host filesystem does.
 *
 * @param a - File path resolved against the process working directory when relative.
 * @param b - File path resolved in the same way.
 * @returns Whether both paths name the same file, ignoring case on Windows.
 */
export function sameFile(a: string, b: string): boolean {
  /**
   * Normalizes a path for comparison.
   *
   * @param file - File path.
   * @returns The absolute path, lowercased on Windows.
   */
  const key = (file: string) =>
    process.platform === "win32" ? path.resolve(file).toLowerCase() : path.resolve(file);
  return key(a) === key(b);
}
