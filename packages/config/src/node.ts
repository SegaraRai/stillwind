/**
 * Node configuration discovery, evaluation, and project selection. `loadConfiguration` opens an
 * explicit file. `resolveIntegrationConfig` also discovers files and applies build overrides.
 * `evaluateModule` bundles and evaluates other local modules, such as plugin files. Consumers
 * retaining compiler state use the returned dependency paths for change tracking.
 */

import { existsSync, statSync } from "node:fs";
import { readFile } from "node:fs/promises";
import { isBuiltin } from "node:module";
import path from "node:path";
import { pathToFileURL } from "node:url";

import { createJiti } from "jiti";
import { rolldown } from "rolldown";
import { glob } from "tinyglobby";

import type { BaseConfig, NormalizedCompilerConfig } from "./compiler.ts";
import { resolveConfig, type AnalysisProjectConfig } from "./configuration.ts";
import {
  configurationCandidates as candidates,
  compilerConfig,
  documentConfiguration,
  expandProjects,
  resolveCompilerSettings,
  resolveProjectPaths,
  selectProject,
  validateInlineConfig,
  type InlineConfig,
} from "./resolution.ts";

/**
 * Evaluated settings and files that consumers must watch when retaining a compiler session.
 */
export interface LoadedConfiguration {
  /** Validated settings after awaiting the module export and invoking its factory, if supplied. */
  config: {
    /**
     * Projects in authored order with references replaced. Referenced projects have an absolute
     * root. Other projects keep their authored paths.
     */
    projects: AnalysisProjectConfig[];
  };
  /** Absolute path of the evaluated configuration file. */
  configFile: string;
  /**
   * Distinct absolute paths of the configuration, referenced configurations, bundled local imports,
   * and resolved external imports. A file that a glob reference would newly match is not included.
   */
  dependencies: string[];
}

/**
 * Settings for one build integration after project selection and inline overrides.
 */
export interface ResolvedIntegrationConfig {
  /** Selected compiler settings after inline overrides and defaults. Filesystem paths are absolute. */
  config: NormalizedCompilerConfig & {
    /** Absolute selected project directory after applying inline overrides. */
    root: string;
    /** Absolute design-system stylesheet path, without an existence check. */
    css: string;
  };
  /** Absolute evaluated configuration path, or undefined when no file was loaded. */
  configFile?: string;
  /** Existing configuration dependency paths to watch. Empty when settings are entirely inline. */
  dependencies: string[];
  /** Absolute missing candidate paths before the selected file, nearest directory first. Watch their creation. */
  missing: string[];
}

/**
 * Finds the nearest supported stillwind.config.* file at or above an application directory.
 *
 * @param directory - Directory to start from. Relative paths use the process working directory.
 * @returns The first existing candidate's absolute path, or undefined if none exists.
 */
export function findConfiguration(directory: string): string | undefined {
  return candidates(directory, path).find((file) => existsSync(file));
}

/**
 * Finds the nearest configuration of a document in a workspace folder, without evaluating it.
 *
 * @param file - Absolute document path.
 * @param folders - Absolute workspace folders.
 * @returns The configuration path, or undefined when the document lies outside the folders or in
 * `node_modules`, or the folder has no configuration at or above the document.
 */
export function findDocumentConfiguration(
  file: string,
  folders: readonly string[],
): string | undefined {
  return documentConfiguration(file, folders, existsSync, path);
}

/**
 * An evaluated module and the files that its evaluation read.
 */
export interface EvaluatedModule {
  /** Default export, or the module value when it has no default export. */
  exported: unknown;
  /** Distinct absolute paths of the entry, bundled local imports, and resolved external imports. */
  dependencies: string[];
  /** Text of each bundled local file, keyed by absolute path, as the bundler read it. */
  sources: Map<string, string>;
}

/**
 * Bundles local imports of a module in memory and evaluates the bundle without module
 * caches. Local sources are read again on each call, so a changed local import takes
 * effect. External packages use runtime module loading, and Node keeps them cached. The
 * module runs as trusted Node code and can perform side effects. Evaluation is
 * available before a framework generates its TypeScript configuration.
 *
 * `import.meta.url`, `import.meta.filename`, `import.meta.dirname`, `__filename`, and
 * `__dirname` describe the original source file of each bundled module.
 *
 * @param file - JS, TS, or JSON module path. Relative paths use the process working
 * directory.
 * @returns The export and the file paths observed while resolving and bundling imports.
 * Runtime file reads and imports that cannot be statically resolved are not added to
 * dependencies.
 * @throws If a package cannot be resolved, or bundling or module evaluation fails.
 */
export async function evaluateModule(file: string): Promise<EvaluatedModule> {
  file = path.resolve(file);

  const dependencies = new Set([file]);
  const sources = new Map<string, string>();
  const bundle = await rolldown({
    input: file,
    cwd: path.dirname(file),
    logLevel: "silent",
    platform: "node",
    treeshake: false,
    tsconfig: false,
    transform: {
      target: "node22",
      define: {
        "import.meta.url": "__stillwind_module_url",
        "import.meta.filename": "__stillwind_module_filename",
        "import.meta.dirname": "__stillwind_module_dirname",
        __filename: "__stillwind_module_filename",
        __dirname: "__stillwind_module_dirname",
      },
    },
    plugins: [
      {
        name: "stillwind-module",
        /**
         * Resolves external packages from their importer and records their paths for watching.
         *
         * @param id - Import request before resolution.
         * @param importer - Importing file, or undefined for the entry.
         * @param options - Import kind used to select package export conditions.
         * @returns An external module URL or require path, or undefined for bundled local sources.
         * @throws If a package cannot be resolved.
         */
        async resolveId(id, importer, options) {
          if (!importer || id.startsWith(".") || path.isAbsolute(id)) {
            return;
          }
          if (isBuiltin(id)) {
            return {
              id,
              external: true,
            };
          }
          const resolved = await this.resolve(id, importer, {
            skipSelf: true,
            kind: options.kind,
          });
          if (!resolved) {
            throw new Error(`Cannot resolve import ${id} from ${importer}`);
          }
          dependencies.add(resolved.id);
          return {
            id: options.kind === "require-call" ? resolved.id : pathToFileURL(resolved.id).href,
            external: "absolute",
          };
        },
        /**
         * Reads a bundled local file and records its text.
         *
         * @param id - Absolute path of a local file, or a virtual module ID.
         * @returns The file's text, or undefined for a virtual module.
         */
        async load(id) {
          if (id.startsWith("\0")) {
            return;
          }
          const code = await readFile(id, "utf8");
          sources.set(id, code);
          return code;
        },
        transform: {
          filter: {
            id: /\.[cm]?[jt]sx?$/,
          },
          /**
           * Keeps file location expressions relative to each original source.
           *
           * @param code - Local module source.
           * @param id - Absolute source path.
           * @returns Source with its file location constants before ordinary syntax lowering.
           */
          handler(code, id) {
            return `const __stillwind_module_url = ${JSON.stringify(pathToFileURL(id).href)};
const __stillwind_module_filename = ${JSON.stringify(id)};
const __stillwind_module_dirname = ${JSON.stringify(path.dirname(id))};
${code}`;
          },
        },
      },
    ],
  });

  let code: string;
  try {
    const result = await bundle.generate({
      format: "esm",
      codeSplitting: false,
    });
    const entry = result.output.find((output) => output.type === "chunk" && output.isEntry);
    if (entry?.type !== "chunk") {
      throw new Error(`Module bundle has no entry: ${file}`);
    }
    for (const id of entry.moduleIds) {
      if (!id.startsWith("\0")) {
        dependencies.add(id);
      }
    }
    code = entry.code;
  } finally {
    await bundle.close();
  }

  const jiti = createJiti(pathToFileURL(file).href, {
    fsCache: false,
    moduleCache: false,
  });
  const value = await jiti.evalModule(code, {
    filename: file.replaceAll("\\", "/"),
    async: true,
    forceTranspile: true,
  });
  return {
    exported:
      value !== null && typeof value === "object" && "default" in value ? value.default : value,
    dependencies: [...dependencies],
    sources,
  };
}

/**
 * Evaluates a configuration module with {@link evaluateModule}, validates its settings, and
 * evaluates the configuration files that it references. Configuration runs as trusted Node code
 * and can perform side effects.
 *
 * @param file - JS, TS, or JSON configuration path. Relative paths use the process working directory.
 * @returns Evaluated settings and the file paths observed while resolving and bundling imports.
 * Runtime file reads and imports that cannot be statically resolved are not added to dependencies.
 * @throws If bundling, module evaluation, factory execution, configuration validation, or reference
 * expansion fails.
 */
export async function loadConfiguration(file: string): Promise<LoadedConfiguration> {
  file = path.resolve(file);

  const dependencies = new Set<string>();
  /**
   * Evaluates one configuration file and records the files that its evaluation read.
   *
   * @param id - Absolute configuration path.
   * @returns Validated settings.
   * @throws If module evaluation or configuration validation fails.
   */
  const load = async (id: string) => {
    const evaluated = await evaluateModule(id);
    for (const dependency of evaluated.dependencies) {
      dependencies.add(dependency);
    }
    return resolveConfig(evaluated.exported);
  };

  const projects = await expandProjects(
    await load(file),
    file,
    {
      isFile: (id) => statSync(id, { throwIfNoEntry: false })?.isFile() ?? false,
      glob: async (patterns, ignore, directory) =>
        (
          await glob(patterns, {
            cwd: directory,
            absolute: true,
            expandDirectories: false,
            ignore,
          })
        ).map((match) => path.resolve(match)),
      load,
    },
    path,
  );
  return {
    config: {
      projects,
    },
    configFile: file,
    dependencies: [...dependencies],
  };
}

/**
 * Resolves project paths and removes analysis scope and provider settings.
 *
 * @param project - Evaluated project configuration with paths relative to its root.
 * @param directory - Base directory for a relative project root, normally the configuration directory.
 * @returns Compiler settings with absolute root, CSS, and tsconfig paths. The supplied project is not modified.
 * A false or omitted tsconfig setting is preserved.
 * @throws If compiler normalization fails or the project does not supply a valid CSS path.
 */
export function resolveProjectConfig(
  project: AnalysisProjectConfig,
  directory: string,
): NormalizedCompilerConfig & {
  /** Absolute root resolved against directory. */
  root: string;
  /** Absolute CSS entrypoint resolved against the project's root. */
  css: string;
} {
  const resolved = resolveProjectPaths(project, directory, path);
  return resolveCompilerSettings(compilerConfig(resolved), {}, resolved.root, path);
}

/**
 * Loads build settings and merges inline overrides into one selected project's compiler settings.
 * Without an explicit name, selection uses the sole project or the unique deepest root containing
 * the application directory after its inline root override. Arrays are replaced during merging.
 *
 * @param inline - File selection, optional project name, and compiler overrides. An inline root
 * uses applicationRoot. A configured root uses the configuration file's directory.
 * @param applicationRoot - Build directory used for discovery and relative inline paths.
 * Relative paths use the process working directory.
 * @param defaults - Browser targets used when the selected project and inline settings omit target.
 * @returns Validated compiler settings with an absolute root and paths to watch. When discovery
 * finds no file, inline settings must supply a CSS entrypoint.
 * @throws If an explicit file is missing, project selection is ambiguous or unsuccessful,
 * configuration evaluation fails, or merged compiler settings are invalid.
 */
export async function resolveIntegrationConfig(
  inline: InlineConfig,
  applicationRoot: string,
  defaults: Pick<BaseConfig, "target"> = {},
): Promise<ResolvedIntegrationConfig> {
  applicationRoot = path.resolve(applicationRoot);

  validateInlineConfig(inline);
  const requested = inline.configFile;
  const candidates =
    requested === false
      ? []
      : requested
        ? [path.resolve(applicationRoot, requested)]
        : configurationCandidates(applicationRoot);
  const file = candidates.find((candidate) => existsSync(candidate));
  const missing = candidates.slice(0, file ? candidates.indexOf(file) : candidates.length);
  if (requested && !file) {
    throw new Error(`Cannot find Stillwind configuration: ${candidates[0]}`);
  }

  const loaded = file ? await loadConfiguration(file) : undefined;
  const projects = loaded?.config.projects.map((project) =>
    resolveProjectPaths(project, path.dirname(file!), path),
  );
  if (!projects && inline.project !== undefined) {
    throw new Error("Cannot select a Stillwind project without a configuration file");
  }

  const selected = projects ? selectProject(projects, applicationRoot, inline, path) : undefined;
  const base = selected
    ? compilerConfig(selected)
    : {
        root: applicationRoot,
        lint: inline.lint,
      };
  const config = resolveCompilerSettings(
    { ...base, target: base.target ?? defaults.target },
    inline,
    applicationRoot,
    path,
  );

  return {
    config,
    configFile: file,
    dependencies: loaded?.dependencies ?? [],
    missing,
  };
}

/**
 * Enumerates Node configuration candidates without checking existence.
 *
 * @param directory - Starting directory. Relative paths use the process working directory.
 * @returns Absolute paths ordered from the nearest directory to the filesystem root, with
 * configurationFiles precedence within each directory.
 */
function configurationCandidates(directory: string): string[] {
  return candidates(directory, path);
}
