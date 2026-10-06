/**
 * Node configuration discovery, evaluation, and project selection. `loadConfiguration` opens an
 * explicit file. `resolveIntegrationConfig` also discovers files and applies build overrides.
 * Consumers retaining compiler state use the returned dependency paths for change tracking.
 */

import { existsSync, readFileSync } from "node:fs";
import { isBuiltin } from "node:module";
import path from "node:path";
import { pathToFileURL } from "node:url";

import { build } from "esbuild";
import { createJiti } from "jiti";

import {
  mergeConfig,
  resolveConfig,
  validateConfig,
  type AnalysisProjectConfig,
  type InlineConfig,
  type StillwindConfig,
  type BaseConfig,
} from "./index.ts";

/** Evaluated settings and files that consumers must watch when retaining a compiler session. */
export interface LoadedConfiguration {
  /** Validated settings after awaiting the module export and invoking its factory, if supplied. */
  config: StillwindConfig;
  /** Absolute path of the evaluated configuration file. */
  configFile: string;
  /** Distinct absolute paths of the configuration, bundled local imports, and resolved external imports. */
  dependencies: string[];
}

/** Settings for one build integration after project selection and inline overrides. */
export interface ResolvedIntegrationConfig {
  /** Selected compiler settings after inline overrides. root is absolute. Relative compiler paths use that root. */
  config: BaseConfig & { root: string };
  /** Absolute evaluated configuration path. Omitted when no file was loaded. */
  configFile?: string;
  /** Existing configuration dependency paths to watch. Empty when settings are entirely inline. */
  dependencies: string[];
  /** Absolute missing candidate paths before the selected file, nearest directory first. Watch their creation. */
  missing: string[];
}

/**
 * Finds the nearest stillwind.config.ts at or above an application directory.
 *
 * @param directory - Directory to start from. Relative paths use the process working directory.
 * @returns The first existing candidate's absolute path, or undefined if none exists.
 */
export function findConfiguration(directory: string): string | undefined {
  return configurationCandidates(directory).find((file) => existsSync(file));
}

/**
 * Bundles local configuration imports in memory, evaluates the export, and validates its settings.
 * Local sources are read again on each call. External packages use runtime module loading.
 * Configuration runs as trusted Node code and can perform side effects.
 *
 * @param file - JS/TS configuration path. Relative paths use the process working directory.
 * @returns Evaluated settings and the file paths observed while resolving and bundling imports.
 * Runtime file reads and imports that cannot be statically resolved are not added to dependencies.
 * @throws If bundling, module evaluation, factory execution, or configuration validation fails.
 */
export async function loadConfiguration(file: string): Promise<LoadedConfiguration> {
  file = path.resolve(file);

  const dependencies = new Set([file]);
  const bundled = await build({
    entryPoints: [file],
    absWorkingDir: path.dirname(file),
    bundle: true,
    write: false,
    metafile: true,
    platform: "node",
    format: "esm",
    target: "node22",
    logLevel: "silent",
    define: {
      "import.meta.url": "__stillwind_config_url",
      "import.meta.filename": "__stillwind_config_filename",
      "import.meta.dirname": "__stillwind_config_dirname",
      __filename: "__stillwind_config_filename",
      __dirname: "__stillwind_config_dirname",
    },
    plugins: [
      {
        name: "stillwind-configuration",
        setup(builder) {
          builder.onResolve({ filter: /.*/ }, async (args) => {
            if (
              args.pluginData === "resolved" ||
              args.kind === "entry-point" ||
              args.path.startsWith(".") ||
              path.isAbsolute(args.path)
            ) {
              return;
            }
            if (isBuiltin(args.path)) {
              return {
                path: args.path,
                external: true,
              };
            }
            const resolved = await builder.resolve(args.path, {
              resolveDir: args.resolveDir,
              kind: args.kind,
              pluginData: "resolved",
            });
            if (resolved.errors.length) {
              return {
                errors: resolved.errors,
              };
            }
            dependencies.add(resolved.path);
            return {
              path: pathToFileURL(resolved.path).href,
              external: true,
            };
          });
          builder.onLoad({ filter: /\.[cm]?[jt]sx?$/ }, (args) => ({
            loader: args.path.endsWith("x")
              ? /\.[cm]?tsx$/.test(args.path)
                ? "tsx"
                : "jsx"
              : /\.[cm]?ts$/.test(args.path)
                ? "ts"
                : "js",
            contents: `const __stillwind_config_url = ${JSON.stringify(pathToFileURL(args.path).href)};
const __stillwind_config_filename = ${JSON.stringify(args.path)};
const __stillwind_config_dirname = ${JSON.stringify(path.dirname(args.path))};
${readFileSync(args.path, "utf8")}`,
          }));
        },
      },
    ],
  });
  for (const input of Object.keys(bundled.metafile.inputs)) {
    dependencies.add(path.resolve(path.dirname(file), input));
  }

  const jiti = createJiti(pathToFileURL(file).href, { fsCache: false, moduleCache: false });
  const value = await jiti.evalModule(bundled.outputFiles[0].text, {
    filename: file.replaceAll("\\", "/"),
    async: true,
    forceTranspile: true,
  });
  const exported =
    value !== null && typeof value === "object" && "default" in value ? value.default : value;

  return {
    config: await resolveConfig(exported),
    configFile: file,
    dependencies: [...dependencies],
  };
}

/**
 * Makes the project root absolute and removes analysis scope and provider settings.
 *
 * @param project - Evaluated project configuration. Other compiler paths are kept unchanged.
 * @param directory - Base directory for a relative project root, normally the configuration directory.
 * @returns Compiler settings with an absolute root. The supplied project is not modified.
 */
export function resolveProjectConfig(
  project: AnalysisProjectConfig,
  directory: string,
): BaseConfig & { root: string } {
  const {
    name: _name,
    files: _files,
    exclude: _exclude,
    sources: _sources,
    projection: _projection,
    typeFacts: _typeFacts,
    ...config
  } = project;

  return {
    ...config,
    root: path.resolve(directory, project.root ?? "."),
  };
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
 * @returns Validated compiler settings with an absolute root and paths to watch. When discovery
 * finds no file, inline settings must supply a CSS entrypoint.
 * @throws If an explicit file is missing, project selection is ambiguous or unsuccessful,
 * configuration evaluation fails, or merged compiler settings are invalid.
 */
export async function resolveIntegrationConfig(
  inline: InlineConfig,
  applicationRoot: string,
): Promise<ResolvedIntegrationConfig> {
  applicationRoot = path.resolve(applicationRoot);

  const { configFile: requested, project: name, ...overrides } = inline;
  if (
    requested !== undefined &&
    requested !== false &&
    (typeof requested !== "string" || !requested.trim())
  ) {
    throw new Error("Stillwind configFile must be a path or false");
  }
  if (name !== undefined && (typeof name !== "string" || !name.trim())) {
    throw new Error("Stillwind project must be a nonempty name");
  }

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

  let base: BaseConfig & { root: string } = { root: applicationRoot };
  let loaded: LoadedConfiguration | undefined;
  if (file) {
    loaded = await loadConfiguration(file);
    const directory = path.dirname(file);
    const projects = loaded.config.projects;
    let selected: AnalysisProjectConfig[];
    if (name !== undefined) {
      selected = projects.filter((project) => project.name === name);
    } else if (projects.length === 1) {
      selected = projects;
    } else {
      const targetRoot = path.resolve(applicationRoot, overrides.root ?? ".");
      const matches = projects
        .map((project) => ({ project, root: path.resolve(directory, project.root ?? ".") }))
        .filter(({ root }) => {
          const relative = path.relative(root, targetRoot);
          return (
            relative === "" ||
            (!path.isAbsolute(relative) &&
              relative !== ".." &&
              !relative.startsWith(`..${path.sep}`))
          );
        });
      const depth = Math.max(...matches.map(({ root }) => root.length));
      selected = matches.filter(({ root }) => root.length === depth).map(({ project }) => project);
    }
    if (selected.length !== 1) {
      throw new Error(
        name !== undefined
          ? `Cannot select Stillwind project ${JSON.stringify(name)}: expected one matching project`
          : "Cannot select one Stillwind project; specify project by name or use a distinct project root",
      );
    }
    base = resolveProjectConfig(selected[0], directory);
  } else if (name !== undefined) {
    throw new Error("Cannot select a Stillwind project without a configuration file");
  }

  const config = mergeConfig(base, overrides);
  if (overrides.root !== undefined) {
    config.root = path.resolve(applicationRoot, overrides.root);
  }

  validateConfig({
    projects: [
      {
        ...config,
        files: ["**/*"],
      },
    ],
  });

  return {
    config,
    configFile: file,
    dependencies: loaded?.dependencies ?? [],
    missing,
  };
}

function configurationCandidates(directory: string): string[] {
  const candidates: string[] = [];
  let current = path.resolve(directory);
  for (;;) {
    candidates.push(path.join(current, "stillwind.config.ts"));
    const parent = path.dirname(current);
    if (parent === current) {
      return candidates;
    }
    current = parent;
  }
}
