import fs from "node:fs";
import { isBuiltin } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { cleanModuleId, type UpstreamSourceMap } from "@stillwind/host";

import { nativeBindingFiles } from "../binding-files.ts";
import type { ProducerInput } from "./emission.ts";

/** Resolver policies passed to webpack or Rspack for CSS, script, and asset imports. */
export interface ResolveSettings {
  /** Dependency category used to select bundler resolution behavior, such as `esm`. */
  dependencyType?: string;
  /** Package export conditions considered when resolving a request. */
  conditionNames?: string[];
  /** File extensions tested for extensionless requests, in resolution order. */
  extensions?: string[];
  /** Package metadata fields considered in resolution order. */
  mainFields?: string[];
}

/**
 * Completion handler for webpack or Rspack resolution.
 *
 * @param error - Resolution failure, or null or undefined when no error occurred.
 * @param id - Resolved resource identity. A missing or false ID declines the request.
 */
export type ResolveCallback = (error?: Error | null, id?: string | false) => void;

/**
 * Resolver bound to the bundler's configured conditions, extensions, and package fields.
 *
 * @param directory - Absolute directory of the importing file.
 * @param specifier - Import or resource request text, including any query suffix.
 * @param callback - Receives the resolution failure or resulting resource identity.
 */
export type Resolver = (directory: string, specifier: string, callback: ResolveCallback) => void;

/** Bundler filesystem and resolver operations shared by producer loaders and CSS reconstruction. */
export interface ResolverHost {
  /** Bundler input filesystem. Reads may return UTF-8 text or bytes through the callback. */
  fs: {
    /**
     * Reads through the bundler filesystem rather than bypassing its virtual files.
     * @param id - Filesystem identity without a resource query.
     * @param callback - Receives an error or file contents. Callers convert successful contents to text.
     */
    readFile(id: string, callback: (error?: Error | null, value?: string | Buffer) => void): void;
  };
  /**
   * Creates a callback resolver with the requested dependency policy.
   *
   * @param settings - Conditions, extensions, and package fields used for this resolver.
   * @returns A resolver accepting the importer directory, request text, and a completion callback.
   */
  getResolve(settings: ResolveSettings): Resolver;
}

/**
 * Loader operations required to resolve producers, capture transformed dependencies, and report
 * compilation inputs.
 */
interface LoaderHostContext extends ResolverHost {
  /**
   * Complete loader chain. Entries after `loaderIndex` run before Stillwind in the normal loader
   * phase.
   */
  loaders: readonly {
    /** Complete inline loader request, including its options, reused for dependency capture. */
    request: string;
  }[];
  /** Index of the currently executing Stillwind loader within `loaders`. */
  loaderIndex: number;
  /**
   * Evaluates a generated capture module through the specified loader chain.
   *
   * @param request - Inline request whose capture loader exports source and map instead of
   * executing application code.
   * @returns A promise for the generated capture module exports.
   */
  importModule(request: string): Promise<unknown>;
  /**
   * Registers a source or compiler file as a dependency of this loader result.
   *
   * @param id - Absolute filesystem path without a resource query.
   */
  addDependency(id: string): void;
  /**
   * Reports a compilation warning through the loader context.
   *
   * @param error - Warning represented as an Error instance.
   */
  emitWarning(error: Error): void;
}

/** Source and map exported by the capture loader after preceding framework transforms. */
export interface CapturedDependency {
  /** Transformed source text consumed by the compiler, without executing its application code. */
  source: string;
  /** Upstream map supplied by preceding loaders, or null when none was supplied. */
  map: UpstreamSourceMap;
}

/** Compiler operations and retained dependency captures for one producer loader invocation. */
export interface LoaderHost {
  /**
   * Reads a resource through the bundler input filesystem.
   *
   * @param id - Readable filesystem path without a query.
   * @returns A promise for the file contents converted to a string.
   */
  read: (id: string) => Promise<string>;
  /**
   * Reads ordinary files or captures query-qualified source through preceding loaders.
   *
   * @param dependency - Full resolver identity, including any query suffix.
   * @returns A promise for source text, rejecting if reading or capture fails.
   */
  readDependency: (dependency: string) => Promise<string>;
  /**
   * Pending transformed-source captures keyed by full resolver identity, retained for emission
   * metadata.
   */
  dependencySources: Map<string, Promise<CapturedDependency>>;
  /**
   * Resolves a CSS import using style export conditions and CSS extensions.
   *
   * @param specifier - CSS import text.
   * @param importer - Absolute path of the importing stylesheet.
   * @returns A promise for the resolved identity, or an empty string when the resolver declines it.
   */
  resolveCss: (specifier: string, importer: string) => Promise<string>;
  /**
   * Resolves an asset using the bundler's default resolution policy.
   *
   * @param specifier - Asset request text.
   * @param importer - Absolute identity of the resource owner.
   * @returns A promise for the resolved identity, or an empty string when the resolver declines it.
   */
  resolveAsset: (specifier: string, importer: string) => Promise<string>;
  /**
   * Resolves a script dependency and marks Node builtins as external.
   *
   * @param specifier - Script import text, optionally naming a Node builtin.
   * @param importer - Absolute identity of the importing producer.
   * @returns A promise for the resolved identity and optional external status, or null when the
   * resolver declines the request. Other resolution errors reject the promise.
   */
  resolve: (
    specifier: string,
    importer: string,
  ) => Promise<{
    /** Full resolved source identity or Node builtin name. */
    id: string;
    /** Whether semantic traversal must skip loading the dependency as source. */
    external?: boolean;
  } | null>;
  /**
   * Registers the underlying file of a resolver identity as a loader dependency.
   *
   * @param id - File identity, optionally including a resource query that is removed before
   * registration.
   */
  watch: (id: string) => void;
  /**
   * Reports a compiler warning through the loader context.
   *
   * @param message - Warning text converted to an Error instance for the bundler.
   */
  warn: (message: string) => void;
}

/**
 * Creates compiler operations from a producer loader context and tracks the loader itself as a
 * dependency.
 *
 * Ordinary dependencies are read from the filesystem. Query-qualified dependencies pass through
 * preceding loaders and the capture loader. Pending captures are reused by their full resolver
 * identity.
 *
 * @param context - Active loader context with a complete loader chain and an `importModule`
 * implementation for query-qualified dependencies.
 * @param loaderPath - Absolute path of the Stillwind loader to register as a dependency.
 * @returns A host that resolves imports, reads files, captures transformed dependencies, and
 * forwards warnings.
 */
export function createLoaderHost(context: LoaderHostContext, loaderPath: string): LoaderHost {
  const read = createReadFile(context.fs);
  const dependencySources = new Map<string, Promise<CapturedDependency>>();
  /**
   * Reads an ordinary dependency or captures transformed text through preceding loaders.
   * @param dependency - Full resolver identity. A query or fragment requires a loader capture.
   * @returns File text or captured text. Capture promises, including failures, are retained by identity.
   * @throws If filesystem reading or the dependency's loader chain fails.
   */
  const readDependency = async (dependency: string) => {
    if (cleanModuleId(dependency) === dependency) {
      return read(dependency);
    }
    let pending = dependencySources.get(dependency);
    if (!pending) {
      const capture = fileURLToPath(
        new URL("./source-loader.js", import.meta.resolve("@stillwind/integration/webpack")),
      );
      const preceding = context.loaders
        .slice(context.loaderIndex + 1)
        .map((loader) => loader.request);
      pending = context.importModule(
        `!!${[capture, ...preceding, dependency].join("!")}`,
      ) as Promise<CapturedDependency>;
      dependencySources.set(dependency, pending);
    }
    return (await pending).source;
  };
  /**
   * Adapts loader resolution to requests relative to a source file.
   * @param settings - Resolution policy passed to the loader's resolver factory.
   * @returns An asynchronous resolver accepting a specifier and full importer identity. Resolver
   * errors reject its promise, and a missing result becomes an empty string.
   */
  const resolver = (settings: ResolveSettings) => {
    const resolve = context.getResolve(settings);
    return (specifier: string, importer: string) =>
      new Promise<string>((done, reject) =>
        resolve(path.dirname(cleanModuleId(importer)), specifier, (error, id) =>
          error ? reject(error) : done(id || ""),
        ),
      );
  };
  const resolveCss = resolver(cssResolveOptions);
  const resolveJs = resolver({
    dependencyType: "esm",
    extensions: [".tsx", ".ts", ".jsx", ".js", ".mjs", ".cjs", ".json"],
  });
  const resolveAsset = resolver({});
  /**
   * Registers a filesystem dependency for loader invalidation.
   * @param id - Full resolver identity whose query and fragment are removed before registration.
   */
  const watch = (id: string) => context.addDependency(cleanModuleId(id));
  watch(loaderPath);
  return {
    read,
    readDependency,
    dependencySources,
    resolveCss,
    resolveAsset,
    resolve: async (specifier: string, importer: string) => {
      if (specifier.startsWith("node:") && isBuiltin(specifier)) {
        return { id: specifier, external: true };
      }
      try {
        const id = await resolveJs(specifier, importer);
        return id ? { id } : null;
      } catch (error) {
        if (isBuiltin(specifier)) {
          return { id: specifier, external: true };
        }
        throw error;
      }
    },
    watch,
    warn: (message: string) => context.emitWarning(new Error(message)),
  };
}

/**
 * Registers compiler runtime files so changes invalidate cached loader results.
 *
 * The function includes native binding files and JavaScript files beside the published host
 * entrypoint.
 *
 * @param watch - Registers an absolute file path as a loader dependency.
 * @throws If the host runtime directory cannot be read.
 */
export function watchCompilerFiles(watch: (id: string) => void): void {
  for (const file of nativeBindingFiles()) {
    watch(file);
  }
  const directory = path.dirname(fileURLToPath(import.meta.resolve("@stillwind/host")));
  for (const file of fs.readdirSync(directory)) {
    if (file.endsWith(".js")) {
      watch(path.join(directory, file));
    }
  }
}

/** File and resolver operations used after producer modules have completed compilation. */
export interface CompilationHost {
  /**
   * Reads the underlying file through the bundler input filesystem.
   *
   * @param id - Resolver identity whose query suffix is removed before reading.
   * @returns A promise for the file contents converted to a string.
   */
  read: (id: string) => Promise<string>;
  /**
   * Creates a reader that prefers captured producer and dependency sources over filesystem reads.
   *
   * @param producers - Producer metadata collected from the completed compilation graph.
   * @returns A reader keyed by full resolver identity. Missing query-qualified captures reject.
   * Ordinary uncaptured files are read through the filesystem.
   */
  dependencyReader: (producers: readonly ProducerInput[]) => (id: string) => Promise<string>;
  /**
   * Resolves an import from the design-system CSS graph.
   *
   * @param specifier - CSS import text.
   * @param importer - Absolute path of the importing stylesheet.
   * @returns A promise for the resolver's file identity, rejecting on resolution errors.
   */
  resolveCss: (specifier: string, importer: string) => Promise<string>;
  /**
   * Resolves a producer dependency, including supported framework source extensions.
   *
   * @param specifier - Script or framework import text.
   * @param importer - Absolute identity of the importing producer.
   * @returns A promise for the resolver's identity, retaining resource queries.
   */
  resolveJs: (specifier: string, importer: string) => Promise<string>;
  /**
   * Resolves a stylesheet asset using the bundler's default resolution policy.
   *
   * @param specifier - Asset request text.
   * @param importer - Absolute identity of the stylesheet or producer that owns the resource.
   * @returns A promise for the resolver's identity, rejecting on resolution errors.
   */
  resolveAsset: (specifier: string, importer: string) => Promise<string>;
}

/**
 * Creates filesystem and resolution operations for reconstructing a completed compilation.
 *
 * Reconstruction resolves framework source files as well as scripts. Query-qualified dependency
 * reads require captured source from producer metadata.
 *
 * @param context - Bundler input filesystem and resolver factory operations.
 * @returns A host that reads files and builds dependency readers from retained producer metadata.
 */
export function createCompilationHost(context: ResolverHost): CompilationHost {
  const read = createReadFile(context.fs, cleanModuleId);
  /**
   * Adapts compilation resolution to requests relative to a source file.
   * @param settings - Resolution policy passed to the bundler's resolver factory.
   * @returns An asynchronous resolver accepting a specifier and full importer identity. It forwards
   * successful resolver results without checking for an absent identity, and rejects resolver errors.
   */
  const resolver = (settings: ResolveSettings) => {
    const resolve = context.getResolve(settings);
    return (specifier: string, importer: string) =>
      new Promise<string>((done, reject) =>
        resolve(path.dirname(cleanModuleId(importer)), specifier, (error, id) =>
          error ? reject(error) : done(id as string),
        ),
      );
  };
  return {
    read,
    /**
     * Creates a reader using source captured by the completed module graph.
     * @param producers - Producer and dependency records. Later records replace earlier text for the same identity.
     * @returns A reader that uses exact captured identities before falling back to ordinary filesystem reads.
     */
    dependencyReader(producers: readonly ProducerInput[]) {
      const sources = new Map<string, string>();
      for (const input of producers) {
        sources.set(input.id, input.source);
        for (const dependency of input.dependencies ?? []) {
          sources.set(dependency.id, dependency.source);
        }
      }
      return (id: string) => {
        if (sources.has(id)) {
          return Promise.resolve(sources.get(id)!);
        }
        if (cleanModuleId(id) !== id) {
          return Promise.reject(new Error(`No transformed source captured for ${id}`));
        }
        return read(id);
      };
    },
    resolveCss: resolver(cssResolveOptions),
    resolveJs: resolver({
      dependencyType: "esm",
      extensions: [".tsx", ".ts", ".jsx", ".js", ".mjs", ".cjs", ".vue", ".svelte", ".astro"],
    }),
    resolveAsset: resolver({}),
  };
}

/**
 * CSS resolution policy that prefers style exports and metadata fields before ordinary package
 * entrypoints.
 */
const cssResolveOptions = {
  conditionNames: ["style", "import", "default"],
  extensions: [".css"],
  mainFields: ["style", "main"],
};

/**
 * Adapts the bundler filesystem to asynchronous string reads.
 *
 * @param filesystem - Input filesystem that completes reads through a callback.
 * @param fileId - Maps resolver identities to readable paths. Defaults to keeping the identity
 * unchanged.
 * @returns A reader that resolves to the filesystem value converted to a string, or rejects on read
 * failure.
 */
function createReadFile(filesystem: ResolverHost["fs"], fileId = (id: string) => id) {
  return (id: string) =>
    new Promise<string>((resolve, reject) =>
      filesystem.readFile(fileId(id), (error, value) =>
        error ? reject(error) : resolve(String(value)),
      ),
    );
}
