import type { AnalysisProjectConfig } from "@stillwind/config";
import {
  NativeProjectSession,
  type NativeDesignSystemLoader,
  type NativeCssResource,
} from "@stillwind/native";
import { relative, resolve, dirname } from "pathe";

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
  /** Maps an absolute virtual path to its original workspace URI. */
  documentUri?(id: string): string;
}

/** Owns the design system, semantic checker, and compiler artifacts for one immutable snapshot. */
export class BrowserProject {
  readonly #editors = new Map<string, EditorSnapshot>();
  readonly #options: BrowserProjectOptions;
  readonly #files: Map<string, string>;
  readonly #facts: BrowserTypeFactsService;
  #loader?: NativeDesignSystemLoader;
  #session?: NativeProjectSession;
  #queue: Promise<unknown> = Promise.resolve();
  #closed = false;
  #resources: NativeCssResource[] = [];
  readonly #cache = new Map<string, ModuleAnalysis>();

  /** @param options - Captured files and configuration. Source mutations require a new project. */
  constructor(options: BrowserProjectOptions) {
    this.#options = options;
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
   * @returns Reports, code, and dependency coverage for the captured snapshot.
   */
  analyze(id: string): Promise<ModuleAnalysis> {
    if (this.#closed) {
      return Promise.reject(new Error("Browser project is closed"));
    }

    const task = this.#queue.then(async () => {
      const cached = this.#cache.get(id);
      if (cached) {
        return cached;
      }

      const { config, root } = this.#options;

      const uri = (file: string) => this.#options.documentUri?.(file) ?? `file://${file}`;
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
        if (config.types !== false) {
          this.#files.set(
            resolve(root, config.types ?? "src/stillwind.generated.d.ts"),
            this.#loader.generateVocabularyTypes(),
          );
        }
        this.#session = new NativeProjectSession(this.#loader, false, "development", config.naming);
      }

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
              ? await config.sources.load(file, { root, documents: this.#files })
              : {
                  resolverId: file,
                  ...descriptor(file),
                  document: { ...descriptor(file).document, code: await read(file) },
                };
          return !("kind" in input) && config.projection?.kind === "custom"
            ? config.projection.prepare(input)
            : input;
        },
        resolve: async (specifier, importer) => {
          const custom =
            config.sources?.kind === "custom" && config.sources.resolve
              ? await config.sources.resolve(specifier, importer, { root, documents: this.#files })
              : null;
          if (custom) {
            return { id: custom };
          }
          const resolved =
            specifier.startsWith(".") || specifier.startsWith("/")
              ? resolveVirtualModule(this.#files, specifier, importer, "source")
              : null;
          return resolved ? { id: resolved } : null;
        },
        watch: () => {},
        warn: () => {},
      };
      const ambientDeclarations = new Map(
        [...this.#files].filter(
          ([file]) =>
            config.types !== false &&
            file === resolve(root, config.types ?? "src/stillwind.generated.d.ts"),
        ),
      );
      let result: ModuleAnalysis;
      try {
        result = await analyzeModule(
          this.#session!,
          () => (config.typeFacts === false ? undefined : this.#facts),
          id,
          host,
          {
            overlayFiles: this.#files,
            ambientDeclarations,
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
      this.#cache.set(id, result);
      return result;
    });

    this.#queue = task.catch(() => {});

    return task;
  }

  /**
   * Returns document CSS for successfully compiled graphs. automatic:false includes
   * only shared base CSS. Local text resources become data URLs; remote URLs retain
   * their authored values.
   * @throws If an emitted local resource is absent from the captured files.
   */
  css(): string {
    if (!this.#session) {
      return "";
    }
    if (this.#options.config.automatic === false) {
      return this.#replaceResources(this.#session.baseCss(), this.#resources);
    }
    const plan = this.#session.planStylesheet({
      modules: this.#modules(),
      coverage: { kind: "observed" },
      placement: "document",
    });
    return this.#replaceResources(plan.css, [
      ...this.#resources,
      ...plan.resources.map((resource) => ({ ...resource, sourceName: resource.moduleId })),
    ]);
  }

  /** Queries the immutable source graph after ensuring the entry has been analyzed. */
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
   * @throws If an emitted local resource is absent from the captured files.
   */
  cssValues(): ReadonlyMap<string, { css: string; parts: { key: string; css: string }[] }> {
    const values = new Map<string, { css: string; parts: { key: string; css: string }[] }>();
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
        parts: plan.parts.map((part) => ({
          key: part.key,
          css: this.#replaceResources(part.css, resources),
        })),
      });
    }
    return values;
  }

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

  #replaceResources(
    css: string,
    resources: readonly { placeholder: string; specifier: string; sourceName: string }[],
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

  /** Returns generated vocabulary declarations after the design system has loaded. */
  declarations(): ReadonlyMap<string, string> {
    if (this.#options.config.types === false) {
      return new Map();
    }
    return new Map(
      [...this.#files].filter(
        ([file]) =>
          file ===
          resolve(this.#options.root, this.#options.config.types || "src/stillwind.generated.d.ts"),
      ),
    );
  }

  /** Waits for pending work and releases the checker. */
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
