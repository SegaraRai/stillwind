import { captureProjectConfig, type AnalysisProjectConfig } from "@stillwind/config";
import {
  NativeProjectSession,
  type BindingRejectedIdentifier,
  type NativeCssResource,
  type NativeDesignSystemLoader,
} from "@stillwind/native";
import { dirname, relative, resolve } from "pathe";

import { analyzeModule, type CompilerHost, type ModuleAnalysis } from "../compilation/compiler.ts";
import { fileSourceKind } from "../compilation/module-source.ts";
import { loadCssGraph } from "../css/css.ts";
import { EditorSnapshot, type EditorRequest, type EditorResult } from "../editor/index.ts";
import { compilerOptions, cssEntrypoint } from "../project/options.ts";
import { BrowserTypeFactsService } from "../semantics/browser.ts";
import { resolveVirtualModule } from "./resolution.ts";

export { TypeFactsError } from "../semantics/backend.ts";
export type {
  AvailableFacts,
  TypeFactsBackend,
  TypeFactSource,
  UnverifiedRange,
} from "../semantics/backend.ts";
export { BrowserTypeFactsService, type BrowserCheckerOptions } from "../semantics/browser.ts";
export { resolveVirtualModule } from "./resolution.ts";

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
    key: string;
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
  readonly #editors = new Map<string, EditorSnapshot>();
  /** Owned configuration records and callbacks fixed for this project's lifetime. */
  readonly #options: BrowserProjectOptions;
  /** Copy of the original inventory used by source, CSS, and checker reads. */
  readonly #files: Map<string, string>;
  /** Owned checker for the captured virtual inventory. */
  readonly #facts: BrowserTypeFactsService;
  /** Lazily loaded CSS graph shared by all entry analyses. */
  #loader?: NativeDesignSystemLoader;
  /** Native session retaining successful compiler artifacts across entry analyses. */
  #session?: NativeProjectSession;
  /** Serializes analyses and permits subsequent work after a failure. */
  #queue: Promise<unknown> = Promise.resolve();
  /** Prevents new analyses and suppresses editor answers after closure starts. */
  #closed = false;
  /** Resource placeholders supplied by the loaded CSS graph. */
  #resources: NativeCssResource[] = [];
  /** Analysis graphs keyed by full entry identity, invalidated when native artifacts are removed. */
  readonly #cache = new Map<string, BrowserModuleAnalysis>();

  /** @param options - Captured files and configuration. Source mutations require a new project. */
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
   * Analyzes and transforms an entry with its demanded semantic dependencies.
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

      const { config, root } = this.#options;

      /**
       * Selects the document identity shared by compiler reports and CSS maps.
       * @param file - Absolute path in the virtual filesystem.
       * @returns The configured URI, or the path prefixed with `file://`.
       */
      const uri = (file: string) => this.#options.documentUri?.(file) ?? `file://${file}`;
      /**
       * Reads captured text without accessing the physical filesystem.
       * @param file - Exact path key in the virtual filesystem.
       * @returns The text currently stored under that key.
       * @throws If the virtual file is absent.
       */
      const read = async (file: string) => {
        const text = this.#files.get(file);
        if (text === undefined) {
          throw new Error(`Virtual file does not exist: ${file}`);
        }
        return text;
      };

      if (!this.#loader) {
        const css = resolve(root, cssEntrypoint(config));
        const loaded = await loadCssGraph(css, compilerOptions(config), {
          resolve: async (specifier, importer) => {
            const resolved = resolveVirtualModule(this.#files, specifier, importer, "style");
            if (!resolved) {
              throw new Error(`Cannot resolve CSS ${specifier} from ${importer}`);
            }
            return resolved;
          },
          read,
          documentUri: uri,
          sourceMapSource: uri,
          watch: () => {},
        });
        this.#loader = loaded.loader;
        this.#resources = loaded.resources;
        this.#session = new NativeProjectSession(this.#loader, false, "development", config.naming);
      }

      /**
       * Describes a virtual file for compilation and checker preparation.
       * @param file - Absolute virtual path whose extension selects its source format.
       * @returns Metadata with project-relative module and projection names and an absolute checker path.
       */
      const descriptor = (file: string) => ({
        moduleId: relative(root, file),
        checkerFileName: file,
        projectionFileName: relative(root, file),
        document: { uri: uri(file), format: fileSourceKind(file).format },
        syntax: fileSourceKind(file).syntax,
      });

      const attempted = new Set<string>();
      const host: CompilerHost = {
        onModule: (moduleId) => {
          attempted.add(moduleId);
        },
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
      const editor = new EditorSnapshot(result, this.#session!, this.#loader!, config);
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
      css: this.#replaceResources(plan.css, [
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
   * Queries the captured source graph after ensuring the entry has been analyzed.
   * @param id - Absolute virtual entry path, including a query that selects distinct source.
   * @param request - Operation with original UTF-16 offsets for that source.
   * @returns The editor answer, or undefined after closure or when no snapshot is available.
   * @throws If entry analysis fails.
   */
  async query(id: string, request: EditorRequest): Promise<EditorResult | undefined> {
    await this.analyze(id);
    if (this.#closed) {
      return undefined;
    }
    return this.#editors.get(id)?.query(id, request);
  }

  /**
   * Returns CSS values keyed by absolute source path for explicit stillwind/module-css imports.
   * Parts preserve stylesheet occurrence order and include their content fingerprints.
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
        ...plan.resources.map((resource) => ({ ...resource, sourceName: resource.moduleId })),
      ];
      values.set(resolve(this.#options.root, id), {
        css: this.#replaceResources(plan.css, resources),
        rejectedNames: plan.rejectedNames,
        parts: plan.parts.map((part) => ({
          key: part.key,
          css: this.#replaceResources(part.css, resources),
        })),
      });
    }
    return values;
  }

  /**
   * Collects modules with generated code from retained successful graphs.
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
   * @returns Completion of project cleanup.
   */
  async close(): Promise<void> {
    this.#closed = true;
    await this.#queue;
    this.#facts.close();
    this.#editors.clear();
    this.#session = undefined;
    this.#loader = undefined;
  }
}

/**
 * Converts a workspace URI to its decoded absolute path without filesystem access.
 * @param uri - Absolute URI whose pathname identifies a file in the virtual inventory.
 * @returns The decoded pathname; the scheme and authority are omitted.
 * @throws If the URI or its percent encoding is invalid.
 */
export function virtualPath(uri: string): string {
  return decodeURIComponent(new URL(uri).pathname);
}
