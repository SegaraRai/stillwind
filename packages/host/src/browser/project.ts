import { captureProjectConfig, type AnalysisProjectConfig } from "@stillwind/config";
import {
  NativeProjectSession,
  type BindingRejectedIdentifier,
  type BindingReport,
  type NativeCssResource,
  type NativeDesignSystemLoader,
} from "@stillwind/native";
import { dirname, relative, resolve } from "pathe";

import { analyzeModule } from "../compilation/compiler.ts";
import type { CompilerHost } from "../compilation/host.ts";
import { fileSourceKind } from "../compilation/module-source.ts";
import { CompilationProject } from "../compilation/project.ts";
import type { ModuleAnalysis } from "../compilation/request.ts";
import { finalizeCss } from "../css/banners.ts";
import {
  isStylesheet,
  loadCssGraph,
  loadEditorCssGraph,
  type CssHost,
  type EditorCssGraph,
} from "../css/css.ts";
import type { PluginHost } from "../css/plugins/host.ts";
import { completePluginPaths, pluginDirectory, queryPluginPaths } from "../css/plugins/paths.ts";
import type { PluginRuntime } from "../css/plugins/runtime.ts";
import { attachPlugins, prepareSourcePlugins } from "../css/plugins/session.ts";
import { EditorSnapshot, type EditorRequest, type EditorResult } from "../editor/index.ts";
import { compilerOptions, cssEntrypoint } from "../project/options.ts";
import { StylesheetEditor } from "../project/stylesheet-editor.ts";
import { BrowserTypeFactsService } from "../semantics/browser/service.ts";
import { resolveVirtualModule } from "./resolution.ts";

/** Files and evaluated configuration captured for one browser project. */
export interface BrowserProjectOptions {
  /** Absolute virtual directory used for relative compiler identities. */
  root: string;
  /** Evaluated project settings. Paths use the virtual workspace. */
  config: AnalysisProjectConfig;
  /** Original sources, package files, and declarations captured for this project revision. */
  files: ReadonlyMap<string, string>;
  /**
   * Maps an absolute virtual path to its original workspace URI.
   * @param id - Decoded virtual source path.
   * @returns URI used for diagnostic locations and source-map names.
   */
  documentUri?(id: string): string;
  /**
   * Loads plugin packages and plugin files for `@plugin` statements. Omission limits the project
   * to the plugins of its configuration.
   */
  plugins?: PluginHost;
}

/** Rendered browser CSS and the naming fallbacks observed during its assembly. */
export interface BrowserStylesheetOutput {
  /** CSS after local resources are embedded. */
  css: string;
  /** Generator rejection records without source locations, in request order. */
  rejectedNames: BindingRejectedIdentifier[];
}

/** One module's carrier and its ordered template fragments. */
export interface BrowserModuleCss extends BrowserStylesheetOutput {
  /** Fragments in stylesheet occurrence order, with their content fingerprints. */
  parts: {
    /** Content fingerprint supplied by the native emission plan. */
    key: string;
    /** Preserved notices required by this fragment. */
    banners?: readonly string[];
    /** Fragment CSS including enclosing conditions, after local resources are embedded. */
    css: string;
  }[];
}

type ReadonlyData<T> = T extends object
  ? {
      readonly [K in keyof T]: ReadonlyData<T[K]>;
    }
  : T;

/** Analysis records shared by cache reads. Use structuredClone() to obtain editable records. */
export type BrowserModuleAnalysis = ReadonlyData<ModuleAnalysis>;

/** Owns the design system, semantic checker, and compiler artifacts for one captured inventory. */
export class BrowserProject {
  /** Editor views retained for each analyzed entry in the captured inventory. */
  readonly #editors = new Map<string, EditorSnapshot | StylesheetEditor>();
  /** Owned configuration records and callbacks fixed for this project's lifetime. */
  readonly #options: BrowserProjectOptions;
  /** Copy of the original inventory used by source, CSS, and checker reads. */
  readonly #files: Map<string, string>;
  /** Owned checker for the captured virtual inventory. */
  readonly #facts: BrowserTypeFactsService;
  /** Prepared sources shared by analyses of different entries. */
  readonly #compilation = new CompilationProject();
  /** Virtual paths read by CSS or plugins. Changes to these files reload CSS. */
  readonly #cssDependencies = new Set<string>();
  /** Lazily loaded CSS graph shared by all entry analyses. */
  #loader?: NativeDesignSystemLoader;
  /** Original or recovered CSS graph retained for queries. Sorting and fixes require original CSS. */
  #editorCss?: EditorCssGraph;
  /** Native session retaining successful compiler artifacts across entry analyses. */
  #session?: NativeProjectSession;
  /** Serializes analyses and permits subsequent work after a failure. */
  #queue: Promise<unknown> = Promise.resolve();
  /** Prevents new analyses and suppresses editor answers after closure starts. */
  #closed = false;
  /** Resource placeholders supplied by the loaded CSS graph. */
  #resources: NativeCssResource[] = [];
  /** Paths that the loaded CSS graph marks for stylesheet lint, in load order. */
  #lintSources: string[] = [];
  /** Plugin runtime of the retained loader. */
  #plugins?: PluginRuntime;
  /** Analysis graphs keyed by full entry identity, invalidated when native artifacts are removed. */
  readonly #cache = new Map<string, BrowserModuleAnalysis>();

  /**
   * Copies the files and configuration without loading CSS or compiling sources.
   *
   * @param options - Captured files and configuration. Use replaceFiles between transactions to change sources.
   */
  constructor(options: BrowserProjectOptions) {
    this.#options = {
      ...options,
      config: captureProjectConfig(options.config),
    };
    this.#files = new Map(options.files);
    this.#facts = new BrowserTypeFactsService({
      root: options.root,
      files: this.#files,
      tsconfig: options.config.tsconfig,
    });
  }

  /**
   * Selects the document identity shared by compiler reports and CSS maps.
   *
   * @param file - Absolute path in the virtual filesystem.
   * @returns The configured URI, or the path prefixed with `file://`.
   */
  #documentUri(file: string): string {
    return this.#options.documentUri?.(file) ?? `file://${file}`;
  }

  /**
   * Reads captured text without accessing the physical filesystem.
   *
   * @param file - Exact path key in the virtual filesystem.
   * @returns The text currently stored under that key.
   * @throws If the virtual file is absent.
   */
  async #read(file: string): Promise<string> {
    const text = this.#files.get(file);
    if (text === undefined) {
      throw new Error(`Virtual file does not exist: ${file}`);
    }
    return text;
  }

  /**
   * Loads the configured CSS graph once and creates the native session that uses it.
   *
   * @returns The retained loader.
   * @throws If resolving, reading, or loading the CSS graph fails.
   */
  async #designSystem(): Promise<NativeDesignSystemLoader> {
    if (!this.#loader) {
      const { config, root } = this.#options;
      const loaded = await loadCssGraph(
        resolve(root, cssEntrypoint(config)),
        compilerOptions(config),
        this.#cssHost(),
        undefined,
        undefined,
        {
          plugins: config.plugins,
          tailwind: config.tailwind,
          important: config.important,
          root,
        },
      );
      this.#loader = loaded.loader;
      this.#resources = loaded.resources;
      this.#lintSources = loaded.lintSources;
      this.#plugins = loaded.plugins;
      this.#session = new NativeProjectSession(this.#loader, false, "development", config.naming);
      attachPlugins(this.#session, this.#plugins);
    }
    return this.#loader;
  }

  /**
   * Creates CSS loader operations backed by this project's captured virtual inventory.
   *
   * @returns A host that resolves style imports and uses document URIs for diagnostics and maps.
   * Its reads reject missing files, and its resolver rejects unresolved imports. Observed paths
   * determine when replacement files reload CSS.
   */
  #cssHost(): CssHost {
    /**
     * Selects the document identity of a virtual stylesheet.
     *
     * @param file - Absolute virtual path.
     * @returns The configured document URI.
     */
    const uri = (file: string) => this.#documentUri(file);
    return {
      resolve: async (specifier, importer) => {
        const resolved = resolveVirtualModule(this.#files, specifier, importer, "style");
        if (!resolved) {
          throw new Error(`Cannot resolve CSS ${specifier} from ${importer}`);
        }
        return resolved;
      },
      read: (file) => this.#read(file),
      documentUri: uri,
      sourceMapSource: uri,
      watch: (id) => {
        this.#cssDependencies.add(id);
      },
      plugins: {
        load: async (specifier, importer, watch) => {
          if (!this.#options.plugins) {
            throw new Error(`This host cannot load plugin ${specifier}`);
          }
          return this.#options.plugins.load(specifier, importer, watch);
        },
        resolve: async (specifier, importer) =>
          resolveVirtualModule(this.#files, specifier, importer) ??
          (await this.#options.plugins?.resolve(specifier, importer)),
        read: async (id) =>
          this.#files.get(id) ??
          (this.#options.plugins ? this.#options.plugins.read(id) : this.#read(id)),
        list: async (id) => {
          const entries = pluginDirectory(this.#files, id);
          if (!entries.length) {
            if (this.#options.plugins?.list) {
              return this.#options.plugins.list(id);
            }
            throw new Error(`Directory ${id} does not exist`);
          }
          return entries;
        },
      },
    };
  }

  /**
   * Reports stylesheet lint for the CSS files that this project lints.
   *
   * @returns Linted virtual paths in load order, and the reports whose primary location is one of
   * them.
   * @throws If the project is closed or the CSS graph cannot be loaded.
   */
  stylesheetLint(): Promise<{
    /** Virtual paths of the CSS entry and the imported files marked for lint. */
    sources: string[];
    /** Stylesheet lint reports with original document locations. */
    reports: BindingReport[];
  }> {
    if (this.#closed) {
      return Promise.reject(new Error("Browser project is closed"));
    }
    const task = this.#queue.then(async () => {
      const loader = await this.#designSystem();
      return {
        sources: [...this.#lintSources],
        reports: loader.stylesheetReports(),
      };
    });
    this.#queue = task.catch(() => {});
    return task;
  }

  /**
   * Creates compiler operations that read the current virtual files.
   *
   * @param onModule - Optional callback for each module the compiler attempts to analyze.
   * @returns A host using this project's configuration, files, and custom source provider.
   * Its reads reject missing files. Its resolver returns null for unresolved requests.
   */
  #host(onModule?: (moduleId: string) => void): CompilerHost {
    const { config, root } = this.#options;
    const uri = (file: string) => this.#documentUri(file);
    const read = (file: string) => this.#read(file);
    /**
     * Describes a virtual file for compilation and checker preparation.
     *
     * @param file - Absolute virtual path whose extension selects its source format.
     * @returns Metadata with project-relative module and projection names and an absolute checker path.
     */
    const descriptor = (file: string) => ({
      moduleId: relative(root, file),
      checkerFileName: file,
      projectionFileName: relative(root, file),
      document: {
        uri: uri(file),
        format: fileSourceKind(file).format,
      },
      syntax: fileSourceKind(file).syntax,
    });

    return {
      onModule,
      describe: descriptor,
      read,
      typeFactsResolution: config.typeFacts !== false,
      supports: (file) =>
        config.sources?.kind === "custom"
          ? config.sources.supports(file)
          : /\.[cm]?[jt]sx?$/.test(file) && !/\.d\.[cm]?ts$/.test(file),
      load: async (file) => {
        const input =
          config.sources?.kind === "custom"
            ? await config.sources.load(file, {
                root,
                documents: this.#files,
              })
            : {
                resolverId: file,
                ...descriptor(file),
                document: {
                  ...descriptor(file).document,
                  code: await read(file),
                },
              };
        return !("kind" in input) && config.projection?.kind === "custom"
          ? config.projection.prepare(input)
          : input;
      },
      resolve: async (specifier, importer) => {
        const custom =
          config.sources?.kind === "custom" && config.sources.resolve
            ? await config.sources.resolve(specifier, importer, {
                root,
                documents: this.#files,
              })
            : null;
        if (custom) {
          return {
            id: custom,
          };
        }
        const resolved =
          specifier.startsWith(".") || specifier.startsWith("/")
            ? resolveVirtualModule(this.#files, specifier, importer, "source")
            : null;
        return resolved
          ? {
              id: resolved,
            }
          : null;
      },
      watch: () => {},
      warn: () => {},
    };
  }

  /**
   * Replaces virtual files after earlier operations finish and clears cached analysis and editor results.
   * Source edits keep CSS loaded. CSS dependencies, package metadata, and file additions or deletions
   * cause the next operation to reload CSS.
   *
   * @param files - All replacement files, copied before the update is queued.
   * @returns Completion of the update. Unchanged files have no effect.
   * @throws If the project is closed when the update runs.
   */
  replaceFiles(files: ReadonlyMap<string, string>): Promise<void> {
    const captured = new Map(files);
    const task = this.#queue.then(() => {
      if (this.#closed) {
        throw new Error("Browser project is closed");
      }
      const changed = new Set(
        [...this.#files.keys(), ...captured.keys()].filter(
          (id) => this.#files.get(id) !== captured.get(id),
        ),
      );
      if (!changed.size) {
        return;
      }
      const resolutionChanged = [...changed].some(
        (id) => /\.json$/i.test(id) || this.#files.has(id) !== captured.has(id),
      );
      this.#compilation.invalidate(changed);
      this.#files.clear();
      for (const [id, text] of captured) {
        this.#files.set(id, text);
      }
      this.#cache.clear();
      this.#editors.clear();
      if (
        resolutionChanged ||
        [...changed].some((id) => isStylesheet(id) || this.#cssDependencies.has(id))
      ) {
        this.#loader = undefined;
        this.#editorCss = undefined;
        this.#session = undefined;
        this.#plugins = undefined;
        this.#resources = [];
        this.#lintSources = [];
        this.#cssDependencies.clear();
      }
    });
    this.#queue = task.catch(() => {});
    return task;
  }

  /**
   * Loads target sources together and prepares their plugin candidates for later analysis.
   *
   * @param ids - Full virtual resolver IDs of the targets.
   * @returns Completion of preparation, or no work after closure. Loading and plugin failures are
   * left for each target's analysis to report. A failed plugin runtime is discarded before retrying.
   */
  prepare(ids: readonly string[]): Promise<void> {
    const task = this.#queue.then(async () => {
      if (this.#closed) {
        return;
      }
      try {
        const loader = await this.#designSystem();
        await this.#compilation.prepare(ids, this.#host(), this.#session!);
        await prepareSourcePlugins(this.#session!, loader, this.#compilation.texts());
      } catch {
        this.#loader = undefined;
        this.#session = undefined;
        this.#plugins = undefined;
        // Entry analysis reports failures with its own source and project identity.
      }
    });
    this.#queue = task.catch(() => {});
    return task;
  }

  /**
   * Analyzes and transforms an entry with its demanded semantic dependencies.
   *
   * @param id - Absolute virtual source path, including any query selecting distinct source.
   * @returns Readonly reports, code, and dependency coverage, shared by cache reads.
   * @throws If the project is closed, CSS loading fails, or compilation fails.
   */
  analyze(id: string): Promise<BrowserModuleAnalysis> {
    if (this.#closed) {
      return Promise.reject(new Error("Browser project is closed"));
    }

    const task = this.#queue.then(async () => {
      const cached = this.#cache.get(id);
      if (cached) {
        return cached;
      }

      const { config } = this.#options;
      await this.#designSystem();
      const attempted = new Set<string>();
      const host = this.#host((id) => attempted.add(id));
      let result: ModuleAnalysis;
      try {
        result = await analyzeModule(
          this.#session!,
          () => (config.typeFacts === false ? undefined : this.#facts),
          id,
          host,
          {
            overlayFiles: this.#files,
          },
          this.#compilation,
        );
      } catch (error) {
        this.#invalidate(attempted);
        throw error;
      }
      if (result.generation === "blocked") {
        // A rejected native transaction can remove previously compiled dependencies.
        this.#invalidate(attempted);
      }
      result.sources = structuredClone(result.sources);
      const editor = new EditorSnapshot(
        result,
        this.#session!,
        this.#loader!,
        config,
        (await this.#plugins?.catalog()) ?? [],
        this.#plugins,
      );
      for (const report of editor.reports()) {
        const batch =
          result.batches.find((batch) =>
            batch.reports.some((item) => item.primary.location.uri === report.primary.location.uri),
          ) ?? result.batches[0];
        if (batch) {
          batch.reports.push(report);
        }
      }
      this.#editors.set(id, editor);
      const published: BrowserModuleAnalysis = result;
      this.#cache.set(id, published);
      return published;
    });

    this.#queue = task.catch(() => {});

    return task;
  }

  /**
   * Returns document CSS for successfully compiled graphs. automatic:false includes
   * only shared base CSS. Local text resources become data URLs; remote URLs retain
   * their authored values.
   *
   * @returns Document CSS and naming fallbacks, or empty CSS and reports before session creation.
   * @throws If an emitted local resource is absent from the captured files.
   */
  css(): BrowserStylesheetOutput {
    if (!this.#session) {
      return {
        css: "",
        rejectedNames: [],
      };
    }
    if (this.#options.config.automatic === false) {
      const output = this.#session.baseCss();
      return {
        css: this.#replaceResources(output.css, this.#resources),
        rejectedNames: output.rejectedNames,
      };
    }
    const plan = this.#session.planStylesheet({
      modules: this.#modules(),
      coverage: {
        kind: "observed",
      },
      placement: "document",
    });
    return {
      css: this.#replaceResources(finalizeCss(plan), [
        ...this.#resources,
        ...plan.resources.map((resource) => ({
          ...resource,
          sourceName: resource.moduleId,
        })),
      ]),
      rejectedNames: plan.rejectedNames,
    };
  }

  /**
   * Queries a captured source graph or a stylesheet in the project's CSS graph.
   * CSS completion, hover, resolution, and colors retry failed graph loads with recovered
   * definitions. CSS sorting and lint fixes require the original graph to load successfully.
   * Plugin path completion and links read captured files inside the project root without
   * loading the CSS graph or executing plugins.
   * Script queries first analyze their entry.
   *
   * @param id - Absolute virtual entry path, including a query that selects distinct source.
   * @param request - Operation with original UTF-16 offsets for that source.
   * @returns The editor answer, or undefined when no snapshot is available, the CSS graph
   * does not mark the stylesheet for a semantic query, or closure suppresses a pending answer.
   * @throws If a script query starts after closure, or if entry analysis, CSS graph loading,
   * source reading, or the editor query fails.
   */
  async query(id: string, request: EditorRequest): Promise<EditorResult | undefined> {
    if (isStylesheet(id)) {
      const task = this.#queue.then(async () => {
        if (this.#closed) {
          return undefined;
        }
        if (!this.#files.has(id)) {
          return undefined;
        }
        const navigation = await queryPluginPaths(id, await this.#read(id), request, {
          root: this.#options.root,
          names: this.#options.config.plugins?.map((plugin) => plugin.name) ?? [],
          documentUri: (file) => this.#documentUri(file),
          host: {
            resolve: async (specifier, importer) =>
              (await this.#options.plugins?.resolve(specifier, importer)) ??
              resolveVirtualModule(this.#files, specifier, importer),
            complete: async (prefix, importer, root) => [
              ...(await completePluginPaths(prefix, importer, root, {
                read: (file) => this.#read(file),
                directory: async (directory) => pluginDirectory(this.#files, directory),
              })),
              ...((await this.#options.plugins?.complete?.(prefix, importer, root)) ?? []),
            ],
          },
        });
        if (navigation) {
          return this.#closed ? undefined : navigation;
        }
        if (request.kind === "sort" || request.kind === "fixes") {
          await this.#designSystem();
          if (this.#editorCss?.recovered) {
            this.#editorCss = undefined;
            for (const [source, editor] of this.#editors) {
              if (editor instanceof StylesheetEditor) {
                this.#editors.delete(source);
              }
            }
          }
        }
        if (!this.#editorCss) {
          this.#editorCss = this.#loader
            ? {
                loader: this.#loader,
                resources: this.#resources,
                lintSources: this.#lintSources,
                recovered: false,
                plugins: this.#plugins,
              }
            : await loadEditorCssGraph(
                resolve(this.#options.root, cssEntrypoint(this.#options.config)),
                compilerOptions(this.#options.config),
                this.#cssHost(),
              );
        }
        if (!this.#editorCss.lintSources.includes(id) || this.#closed) {
          return undefined;
        }
        if (!this.#editors.has(id)) {
          const reports = this.#editorCss.recovered
            ? []
            : this.#editorCss.loader
                .stylesheetReports()
                .filter((report) => report.primary.location.uri === this.#documentUri(id));
          this.#editors.set(
            id,
            new StylesheetEditor(
              id,
              await this.#read(id),
              reports,
              this.#editorCss.loader.editorQuery(),
              this.#options.config,
              (await this.#editorCss.plugins?.catalog()) ?? [],
              this.#editorCss.plugins,
            ),
          );
        }
        const editor = this.#editors.get(id);
        await editor?.prepare(id, request);
        return this.#closed ? undefined : editor?.query(id, request);
      });
      this.#queue = task.catch(() => {});
      return task;
    }

    await this.analyze(id);
    if (this.#closed) {
      return undefined;
    }
    const editor = this.#editors.get(id);
    if (editor instanceof EditorSnapshot) {
      await editor.prepare(id, request);
    }
    return editor?.query(id, request);
  }

  /**
   * Returns CSS values keyed by absolute source path for explicit stillwind/module-css imports.
   * Parts preserve stylesheet occurrence order and include their content fingerprints.
   *
   * @returns A new map of module CSS records. No session produces an empty map.
   * @throws If an emitted local resource is absent from the captured files.
   */
  cssValues(): ReadonlyMap<string, BrowserModuleCss> {
    const values = new Map<string, BrowserModuleCss>();
    if (!this.#session) {
      return values;
    }
    for (const id of this.#modules()) {
      const plan = this.#session.planStylesheet({
        modules: [id],
        coverage: { kind: "observed" },
        placement: "carrier",
      });
      const resources = [
        ...this.#resources,
        ...plan.resources.map((resource) => ({
          ...resource,
          sourceName: resource.moduleId,
        })),
      ];
      values.set(resolve(this.#options.root, id), {
        css: this.#replaceResources(finalizeCss(plan), resources),
        rejectedNames: plan.rejectedNames,
        parts: plan.parts.map((part) => ({
          banners: part.banners,
          key: part.key,
          css: this.#replaceResources(part.css, resources),
        })),
      });
    }
    return values;
  }

  /**
   * Collects modules with generated code from retained successful graphs.
   *
   * @returns Distinct compiler identities in first graph occurrence order.
   */
  #modules(): string[] {
    return [
      ...new Set(
        [...this.#cache.values()]
          .filter((graph) => graph.generation === "complete")
          .flatMap((graph) =>
            graph.modules.filter((module) => module.code !== null).map((module) => module.moduleId),
          ),
      ),
    ];
  }

  /**
   * Removes cached graphs and editor snapshots affected by a rejected native transaction.
   *
   * @param modules - Compiler identities attempted by that transaction, including dependencies.
   */
  #invalidate(modules: ReadonlySet<string>): void {
    for (const [id, graph] of this.#cache) {
      if (
        modules.has(graph.entry) ||
        graph.sources.some((source) => modules.has(source.moduleId))
      ) {
        this.#cache.delete(id);
        this.#editors.delete(id);
      }
    }
  }

  /**
   * Replaces emitted CSS placeholders with authored remote URLs or embedded local text.
   *
   * @param css - Generated CSS containing zero or more resource placeholders.
   * @param resources - Placeholder records with source names relative to the project root or absolute.
   * @returns CSS with JSON-escaped URL contents. SVG uses image/svg+xml and other local text uses text/plain.
   * @throws If a referenced local file is absent from the captured inventory.
   */
  #replaceResources(
    css: string,
    resources: readonly {
      /** Exact emitted token to replace in CSS. */
      placeholder: string;
      /** Authored URL, resolved relative to its source when it is local. */
      specifier: string;
      /** Source path whose directory determines local resource resolution. */
      sourceName: string;
    }[],
  ): string {
    for (const resource of resources) {
      if (!css.includes(resource.placeholder)) {
        continue;
      }
      let url = resource.specifier;
      if (!/^(?:[a-z][a-z\d+.-]*:|\/\/|#)/i.test(url)) {
        const file = resolve(dirname(resolve(this.#options.root, resource.sourceName)), url);
        const source = this.#files.get(file);
        if (source === undefined) {
          throw new Error(`Browser CSS resource is missing: ${file}`);
        }
        const mime = file.endsWith(".svg") ? "image/svg+xml" : "text/plain";
        url = `data:${mime};charset=utf-8,${encodeURIComponent(source)}`;
      }
      css = css.replaceAll(resource.placeholder, JSON.stringify(url).slice(1, -1));
    }
    return css;
  }

  /**
   * Prevents new analyses, waits for queued work, and releases the checker, snapshots, and native session.
   *
   * @returns Completion of project cleanup.
   */
  async close(): Promise<void> {
    this.#closed = true;
    await this.#queue;
    this.#facts.close();
    this.#editors.clear();
    this.#session = undefined;
    this.#loader = undefined;
    this.#editorCss = undefined;
  }
}

/**
 * Converts a workspace URI to its decoded absolute path without filesystem access.
 *
 * @param uri - Absolute URI whose pathname identifies a file in the virtual inventory.
 * @returns The decoded pathname; the scheme and authority are omitted.
 * @throws If the URI or its percent encoding is invalid.
 */
export function virtualPath(uri: string): string {
  return decodeURIComponent(new URL(uri).pathname);
}
