import { createHash } from "node:crypto";
import { realpathSync } from "node:fs";
import { readFile, realpath } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { pathToFileURL } from "node:url";

import { cleanModuleId, cssEntrypoint, loadCssGraph } from "@stillwind/host";
import type { NativeDesignSystemLoader, NativeProjectSession } from "@stillwind/native";

import { buildCompilerOptions, IntegrationSession } from "../session.ts";
import { readableFile } from "./file-access.ts";
import type { IntegrationConfig, ProjectSettings } from "./project.ts";
import { resolvedFile, type StylesheetResource } from "./resources.ts";

/** Vite operations needed to resolve design-system CSS and record files used by compilation. */
export interface CssContext {
  /**
   * Resolves an import through the other Vite plugins.
   *
   * @param specifier - Import text from CSS or a producer.
   * @param importer - Resolver identity of the importing file.
   * @param options - Resolution options. Stillwind passes `skipSelf: true` to avoid resolving its
   * own request again.
   * @returns A resolved identity with optional external status, or `null` when resolution fails.
   */
  resolve: (
    specifier: string,
    importer: string,
    options: {
      /** Avoids re-entering the Stillwind plugin that initiated resolution. */
      skipSelf: boolean;
    },
  ) => Promise<{
    /** Full resolved identity, including any resource query. */
    id: string;
    /** Truthy values mark an external resolution that requires a readable filesystem identity. */
    external?: boolean | string;
  } | null>;
  /**
   * Registers a file as a dependency of the current Vite operation.
   *
   * @param id - Resolved file identity to observe for changes.
   */
  addWatchFile: (id: string) => void;
  /**
   * Reports compiler warnings, including generator fallbacks without source locations.
   *
   * @param message - Formatted warning text.
   **/
  warn: (message: string) => void;
}

/**
 * Read-only progress of design-system loading, used to schedule refreshes and report retained
 * failures.
 */
export interface DesignSystemStatus {
  /** Files required by the last successful design-system load, stored as absolute paths. */
  readonly cssFiles: ReadonlySet<string>;
  /**
   * Absolute paths observed during loading, including candidates whose failed reads must be retried
   * after changes.
   */
  readonly cssObservedFiles: ReadonlySet<string>;
  /** Number of observed file-change events. Several events can advance this before one reload. */
  readonly cssChange: number;
  /** Change counter captured by the last successful load or reload. */
  readonly cssApplied: number;
  /**
   * Change counter captured by the latest load or reload attempt. Starts at -1 before
   * initialization.
   */
  readonly cssAttempted: number;
  /** Failure from the latest refresh attempt. Successful loads and reloads clear it. */
  readonly cssError?: unknown;
}

/** Owns one environment's compiler, design-system resources, and CSS reload state. */
export class DesignSystem {
  /** Compiler session retained across CSS reloads and replaced when initialization restarts. */
  #compiler?: IntegrationSession;
  /** Whether the compiler is ready for producer compilation and stylesheet planning. */
  #initialized = false;

  /**
   * Resource placeholders mapped to resolved file identities and readable paths from the last
   * successful load.
   */
  resources = new Map<string, StylesheetResource>();
  /**
   * Absolute paths required by the last successful design-system load, including referenced
   * resource files.
   */
  cssFiles = new Set<string>();
  /**
   * Absolute paths tracked for changes, including unresolved relative candidates discovered during
   * failed loads.
   */
  cssObservedFiles = new Set<string>();
  /** Content fingerprint used to detect changes when development runs without a CSS watcher. */
  cssVersion?: string;
  /** Counter advanced by changes to observed files. */
  cssChange = 0;
  /** Change counter last applied successfully to the compiler. */
  cssApplied = 0;
  /**
   * Change counter last attempted. Starts at -1 so the first watched revision cannot be skipped.
   */
  cssAttempted = -1;
  /**
   * Latest refresh error. Recording an error does not clear the initialized compiler session.
   */
  cssError?: unknown;

  /**
   * Creates design-system state without loading CSS or opening a compiler session.
   *
   * @param settings - Shared project settings configured before initialization.
   * @param config - Compiler options that identify the CSS entrypoint.
   */
  constructor(
    private readonly settings: ProjectSettings,
    private readonly config: IntegrationConfig,
  ) {}

  /**
   * Obtains the initialized compiler session.
   *
   * @returns The session currently holding the loaded design system.
   * @throws If `ensure` has not initialized the session or `reset` has invalidated it.
   */
  get compiler(): IntegrationSession {
    if (!this.#initialized || !this.#compiler) {
      throw new Error("Stillwind environment has not been initialized");
    }
    return this.#compiler;
  }

  /**
   * Reports whether producer compilation can use the current session.
   *
   * @returns True after successful initialization and until the next reset.
   */
  get initialized(): boolean {
    return this.#initialized;
  }

  /**
   * Invalidates initialization so the next `ensure` reloads CSS and replaces the retained compiler
   * session.
   */
  reset(): void {
    this.#initialized = false;
  }

  /**
   * Closes the compiler's type-facts service while retaining native compilation state and CSS
   * reload metadata.
   */
  close(): void {
    this.#compiler?.close();
  }

  /**
   * Marks a design-system revision pending when an observed file changes.
   *
   * @param file - Absolute path of the changed file. Paths outside `cssObservedFiles` have no
   * effect.
   */
  changed(file: string): void {
    if (this.cssObservedFiles.has(file)) {
      this.cssChange++;
    }
  }

  /**
   * Loads a stable design-system revision and creates the compiler if initialization is pending.
   *
   * While watching, changes observed during loading cause the load to retry.
   *
   * @param context - Vite resolver and watch operations for the current build or server.
   * @param onInitialize - Called after the new session and CSS state are installed.
   * @returns A promise that resolves when the compiler is ready. An initialized session is reused.
   * @throws If loading CSS, resolving resources, or creating the compiler fails.
   */
  async ensure(context: CssContext, onInitialize: () => void): Promise<void> {
    if (this.#initialized) {
      return;
    }
    while (true) {
      const change = this.cssChange;
      let loaded: DesignSystemRevision;
      try {
        loaded = await this.#loadRevision(context);
      } catch (error) {
        if (this.settings.cssWatching && change !== this.cssChange) {
          continue;
        }
        throw error;
      }
      const version =
        this.settings.watching && !this.settings.cssWatching
          ? await designSystemVersion(loaded.dependencies)
          : undefined;
      if (this.settings.cssWatching && change !== this.cssChange) {
        continue;
      }
      this.#compiler?.close();
      this.#compiler = new IntegrationSession(
        this.settings.projectRoot,
        loaded.loader,
        this.config,
        this.settings.watching,
        this.settings.compilerMode,
      );
      this.#initialized = true;
      this.resources = loaded.resources;
      this.cssFiles = loaded.dependencies;
      this.cssObservedFiles = loaded.observed;
      this.cssVersion = version;
      this.cssApplied = this.cssAttempted = change;
      this.cssError = undefined;
      onInitialize();
      return;
    }
  }

  /**
   * Reloads changed development CSS and records failed refresh attempts.
   *
   * Production returns after initialization. Development uses observed change counters or file
   * fingerprints. Refresh failures are recorded and warned. If loading the next CSS revision
   * fails, the compiler keeps the previously loaded design system.
   *
   * @param context - Vite resolver and watch operations for loading the next design-system
   * revision.
   * @param onInitialize - Called if initialization must create a compiler session.
   * @param onReload - Receives compiler module identities whose generated JavaScript changed after
   * a reload.
   * @returns A promise that resolves after initialization and any required refresh.
   * @throws If initial loading fails. Failures during a later refresh are recorded in `cssError`.
   */
  async refresh(
    context: CssContext,
    onInitialize: () => void,
    onReload: (modules: string[]) => void,
  ): Promise<void> {
    await this.ensure(context, onInitialize);
    if (!this.settings.watching) {
      return;
    }
    while (true) {
      const change = this.cssChange;
      if (this.settings.cssWatching && this.cssAttempted === change) {
        return;
      }
      try {
        const version = this.settings.cssWatching
          ? undefined
          : await designSystemVersion(this.cssFiles);
        if (!this.settings.cssWatching && version === this.cssVersion) {
          return;
        }
        const loaded = await this.#loadRevision(context);
        const nextVersion = this.settings.cssWatching
          ? undefined
          : await designSystemVersion(loaded.dependencies);
        if (
          this.settings.cssWatching
            ? change !== this.cssChange
            : version !== (await designSystemVersion(this.cssFiles))
        ) {
          continue;
        }
        const update = this.compiler.native.reload(loaded.loader);
        onReload(update.transformedModules);
        this.resources = loaded.resources;
        this.cssFiles = loaded.dependencies;
        this.cssObservedFiles = loaded.observed;
        this.cssVersion = nextVersion;
        this.cssApplied = this.cssAttempted = change;
        this.cssError = undefined;
        return;
      } catch (error) {
        if (this.settings.cssWatching && change !== this.cssChange) {
          continue;
        }
        this.cssAttempted = change;
        this.cssError = error;
        this.settings.devServerRef?.config.logger.warn(
          `Stillwind update failed; retaining published styles: ${String(error)}`,
        );
        return;
      }
    }
  }

  /**
   * Loads CSS and resources while recording successful dependencies and attempted file reads.
   *
   * Files are registered with Vite and the development watcher as they are discovered. Failed loads
   * retain newly observed paths so later edits can trigger another attempt.
   *
   * @param context - Vite resolver and watch operations.
   * @returns The loaded design system, resolved resources, required file set, and observed file
   * set.
   * @throws If CSS loading or resource resolution fails.
   */
  async #loadRevision(context: CssContext): Promise<DesignSystemRevision> {
    const dependencies = new Set<string>();
    const observed = new Set<string>();
    // Keep this set so reads that finish later cannot change the next revision's file set.
    const tracking = this.cssObservedFiles;
    /**
     * Records a dependency once for this load and registers it with Vite's watchers.
     * @param file - Attempted or resolved file path, normalized to an absolute path.
     */
    const watch = (file: string) => {
      const path = resolve(file);
      if (observed.has(path)) {
        return;
      }
      observed.add(path);
      tracking.add(path);
      context.addWatchFile(path);
      this.settings.devServerRef?.watcher.add(path);
    };
    const result = await loadDesignSystem(
      this.settings,
      this.config,
      async (specifier, importer) =>
        resolvedFile(
          await context.resolve(specifier, importer, { skipSelf: true }),
          `CSS ${specifier} from ${importer}`,
        ),
      dependencies,
      this.#initialized ? this.#compiler?.native : undefined,
      watch,
    );
    return { ...result, dependencies, observed };
  }
}

/** Loaded design system and resolved resource files for one Vite environment. */
export interface LoadedDesignSystem {
  /** Native loader containing the complete CSS graph and compiler settings. */
  loader: NativeDesignSystemLoader;
  /** Resource placeholder tokens mapped to resolver identities and checked filesystem paths. */
  resources: Map<string, StylesheetResource>;
}

/** Loaded design system with the file sets needed to watch and apply its revision. */
export interface DesignSystemRevision extends LoadedDesignSystem {
  /** Absolute paths required by the successful load, including referenced assets. */
  dependencies: Set<string>;
  /** Absolute paths observed during loading, including unresolved relative candidates. */
  observed: Set<string>;
}

/**
 * Loads the configured CSS graph and resolves each referenced resource to a readable file.
 *
 * @param settings - Configured project root and optional development server used to check resource
 * access.
 * @param config - Compiler settings and the CSS entrypoint, resolved against the project root.
 * @param resolveImport - Resolves a specifier relative to its importer and returns a file identity.
 * @param dependencies - Mutable set receiving files required by the CSS graph and its resources.
 * @param session - Existing native session used to create a compatible loader during reload.
 * @param watch - Records attempted and resolved file paths, including unresolved relative
 * candidates.
 * @returns A native design-system loader and a map from resource placeholders to resolved files.
 * @throws If the entrypoint is missing, an import fails, or a referenced resource is not readable.
 */
async function loadDesignSystem(
  settings: ProjectSettings,
  config: IntegrationConfig,
  resolveImport: (specifier: string, importer: string) => Promise<string>,
  dependencies: Set<string>,
  session?: NativeProjectSession,
  watch: (file: string) => void = () => {},
): Promise<LoadedDesignSystem> {
  const entry = resolve(settings.projectRoot, cssEntrypoint(config));
  watch(entry);
  const root = realpathSync(entry);
  /**
   * Watches relative candidates before resolution and the resolved file after success.
   * @param specifier - CSS import or resource request passed to the configured resolver.
   * @param importer - Full identity of the source containing the request.
   * @returns The resolver's identity with its query and fragment retained.
   * @throws If resolution or a watch callback fails.
   */
  const resolveCss = async (specifier: string, importer: string) => {
    if (specifier.startsWith(".")) {
      watch(resolve(dirname(importer), cleanModuleId(specifier)));
    }
    const file = await resolveImport(specifier, importer);
    watch(cleanModuleId(file));
    return file;
  };
  const nativeOptions = buildCompilerOptions(config);
  const { loader, resources: resourceReferences } = await loadCssGraph(
    root,
    nativeOptions,
    {
      resolve: async (specifier, importer) => realpath(await resolveCss(specifier, importer)),
      read: (name) => readFile(name, "utf8"),
      documentUri: (name) => pathToFileURL(name).href,
      sourceMapSource: (name) => pathToFileURL(name).href,
      watch: (name) => {
        dependencies.add(name);
        watch(name);
      },
    },
    undefined,
    session?.newLoader(nativeOptions),
  );
  const references = new Map<string, StylesheetResource>();
  for (const resource of resourceReferences) {
    const resolved = await resolveCss(resource.specifier, resource.sourceName);
    const file = await readableFile(resolved, settings.devServerRef);
    references.set(resource.placeholder, {
      id: resolved,
      file,
    });
    dependencies.add(cleanModuleId(resolved));
  }
  return {
    loader,
    resources: references,
  };
}

/**
 * Computes a stable fingerprint of the paths and contents used by the design system.
 *
 * @param files - Absolute dependency paths. Iteration order does not affect the fingerprint.
 * @returns A SHA-256 hexadecimal fingerprint. Missing files contribute a fixed marker.
 * @throws If reading a file fails for a reason other than `ENOENT`.
 */
async function designSystemVersion(files: Set<string>) {
  const hash = createHash("sha256");
  const ordered = [...files].sort();
  for (let offset = 0; offset < ordered.length; offset += 64) {
    const batch = ordered.slice(offset, offset + 64);
    const contents = await Promise.all(
      batch.map(async (file) => {
        try {
          return await readFile(file);
        } catch (error) {
          if ((error as NodeJS.ErrnoException).code !== "ENOENT") {
            throw error;
          }
          return "\0missing";
        }
      }),
    );
    for (const [index, file] of batch.entries()) {
      hash.update(file).update("\0").update(contents[index]).update("\0");
    }
  }
  return hash.digest("hex");
}
