/**
 * Selects projects and resolves compiler settings through host-supplied path operations, without reading files.
 */

import * as v from "valibot";

import {
  compilerEntries,
  normalizeCompilerConfig,
  validateCompilerConfig,
  type BaseConfig,
  type BuildConfig,
  type BuildOptions,
  type NormalizedCompilerConfig,
} from "./compiler.ts";
import type { AnalysisProjectConfig, StillwindConfig } from "./configuration.ts";
import { mergeLintConfig } from "./lint.ts";
import { mergeConfig } from "./merge.ts";
import { nonemptyText, strictObject, validate } from "./validation.ts";

/**
 * Inline options that can pass through isolated loader workers.
 */
export type SerializableInlineConfig = Omit<InlineConfig, "naming" | "plugins" | "tailwind">;

/**
 * Build overrides and the selection of a configuration file and project.
 */
export interface InlineConfig extends BuildConfig {
  /**
   * Configuration file path, absolute or relative to the application root. An explicit path must
   * exist. Omission searches upward for stillwind.config.*. False disables file loading.
   */
  configFile?: string | false;
  /**
   * Exact configured project name. Omission selects the sole project or the unique project whose
   * root most closely contains the application root. Ambiguous selection fails during evaluation.
   */
  project?: string;
}

/**
 * Path operations supplied by the host's filesystem or virtual workspace.
 */
export interface ConfigPaths {
  /**
   * Combines path segments and removes relative traversal segments.
   *
   * @param paths - Path segments interpreted using the host's path syntax and working directory.
   * @returns A normalized absolute path. An absolute segment replaces preceding segments.
   */
  resolve(...paths: string[]): string;
  /**
   * Gets the parent directory used to walk configuration candidates.
   *
   * @param path - Normalized absolute path in the same syntax accepted by resolve.
   * @returns Its absolute parent directory. A filesystem root must return itself to end traversal.
   */
  dirname(path: string): string;
  /**
   * Computes the path used to determine project containment.
   *
   * @param from - Absolute project root.
   * @param to - Absolute application directory.
   * @returns A relative path, or an absolute path when the host cannot express a relative path.
   * The same location returns an empty string.
   */
  relative(from: string, to: string): string;
  /**
   * Recognizes absolute paths using the same syntax as the other operations.
   *
   * @param path - Path returned by a path operation or supplied by configuration.
   * @returns Whether the path is absolute for this host.
   */
  isAbsolute(path: string): boolean;
}

/** Filenames in precedence order within each searched directory. */
export const configurationFiles = ["ts", "mts", "cts", "js", "mjs", "cjs", "json"].map(
  (extension) => `stillwind.config.${extension}`,
);

/**
 * Enumerates configuration candidates from the nearest directory to the host's root.
 *
 * @param directory - Starting directory, normalized with the supplied resolve operation.
 * @param paths - Consistent host path operations. dirname must return the root unchanged.
 * @returns Absolute candidate paths in directory order, then configurationFiles order within each
 * directory. Existence is not checked.
 */
export function configurationCandidates(directory: string, paths: ConfigPaths): string[] {
  const candidates: string[] = [];
  let current = paths.resolve(directory);
  for (;;) {
    candidates.push(...configurationFiles.map((file) => paths.resolve(current, file)));
    const parent = paths.dirname(current);
    if (parent === current) {
      return candidates;
    }
    current = parent;
  }
}

/**
 * File access that expandProjects uses to follow references in a host's filesystem or virtual workspace.
 */
export interface ProjectReferenceHost {
  /**
   * Tests whether a path identifies an existing file.
   *
   * @param file - Normalized absolute path.
   * @returns False for directories and missing paths.
   */
  isFile(file: string): boolean;
  /**
   * Enumerates existing files that match any pattern.
   *
   * @param patterns - Globs relative to directory. Dotted path segments do not match wildcards.
   * @param ignore - Globs relative to directory that remove matches.
   * @param directory - Absolute directory of the configuration that declares the patterns.
   * @returns Absolute paths in the syntax that the host's path operations produce.
   */
  glob(
    patterns: readonly string[],
    ignore: readonly string[],
    directory: string,
  ): Promise<string[]>;
  /**
   * Evaluates and validates a referenced configuration file.
   *
   * @param file - Existing absolute path.
   * @returns Validated settings.
   * @throws If evaluation or validation fails.
   */
  load(file: string): Promise<StillwindConfig>;
}

/**
 * Finds the configuration files that a reference string selects.
 *
 * @param reference - Path or glob relative to directory, without a `!` prefix. An existing path
 * takes precedence over its glob meaning, so a directory such as `apps/(web)` stays usable.
 * @param negated - Paths and globs from `!` entries, relative to directory.
 * @param directory - Absolute directory of the configuration that declares the reference.
 * @param host - File access for the configuration's filesystem or virtual workspace.
 * @param paths - Host path operations.
 * @returns Absolute configuration paths. A glob returns at most one file per directory, chosen in
 * configurationFiles order, and skips directories that have no configuration file.
 * @throws If a reference without glob syntax does not identify a file or a configured directory.
 */
async function referencedFiles(
  reference: string,
  negated: readonly string[],
  directory: string,
  host: ProjectReferenceHost,
  paths: ConfigPaths,
): Promise<string[]> {
  const target = paths.resolve(directory, reference);
  const direct = [target, ...configurationFiles.map((file) => paths.resolve(target, file))].find(
    (file) => host.isFile(file),
  );
  if (direct) {
    return [direct];
  }
  if (!/[*?[\]{}()]/.test(reference)) {
    throw new Error(`Cannot find Stillwind configuration: ${target}`);
  }

  const pattern = globPath(reference);
  const matches = new Set(
    await host.glob(
      [pattern, ...configurationFiles.map((file) => `${pattern}/${file}`)],
      [
        "**/node_modules/**",
        "**/.git/**",
        ...negated.flatMap((path) => [globPath(path), `${globPath(path)}/**`]),
      ],
      directory,
    ),
  );
  return [...new Set([...matches].map((file) => paths.dirname(file)))].sort().flatMap((match) => {
    const file = configurationFiles
      .map((name) => paths.resolve(match, name))
      .find((candidate) => matches.has(candidate));
    return file ? [file] : [];
  });
}

/**
 * Removes path spellings that glob matchers would treat as literal segments.
 *
 * @param path - Authored relative path or glob.
 * @returns The path without leading `./` segments or trailing slashes.
 */
function globPath(path: string): string {
  return path.replace(/^(?:\.\/)+/, "").replace(/\/+$/, "");
}

/**
 * Replaces references with the projects of the files that they select.
 *
 * @param config - Validated settings of the configuration file.
 * @param file - Normalized absolute path of the configuration file.
 * @param host - File access for following references.
 * @param paths - Host path operations.
 * @returns Projects in authored order. Project objects of config keep their identity and relative
 * paths. Referenced projects are copies with an absolute root, so their relative paths stay valid
 * against any configuration directory. Each file contributes once, even if several references
 * select it, and a reference to file itself adds nothing.
 * @throws If a reference selects no file, a referenced file contains references, evaluation fails,
 * no project remains, or two projects share a name.
 */
export async function expandProjects(
  config: StillwindConfig,
  file: string,
  host: ProjectReferenceHost,
  paths: ConfigPaths,
): Promise<AnalysisProjectConfig[]> {
  const directory = paths.dirname(file);
  const negated = config.projects.flatMap((entry) =>
    typeof entry === "string" && entry.startsWith("!") ? [entry.slice(1)] : [],
  );
  const visited = new Set([file]);
  const projects: AnalysisProjectConfig[] = [];
  for (const entry of config.projects) {
    if (typeof entry !== "string") {
      projects.push(entry);
      continue;
    }
    if (entry.startsWith("!")) {
      continue;
    }
    for (const referenced of await referencedFiles(entry, negated, directory, host, paths)) {
      if (visited.has(referenced)) {
        continue;
      }
      visited.add(referenced);
      const base = paths.dirname(referenced);
      for (const project of (await host.load(referenced)).projects) {
        if (typeof project === "string") {
          throw new Error(
            `Stillwind configuration ${referenced} cannot contain references because ${file} references it`,
          );
        }
        projects.push({ ...project, root: paths.resolve(base, project.root ?? ".") });
      }
    }
  }

  if (!projects.length) {
    throw new Error(`Stillwind configuration ${file}: projects selects no project`);
  }
  const names = new Set<string>();
  for (const { name } of projects) {
    if (name !== undefined && names.has(name)) {
      throw new Error(`Duplicate Stillwind project name: ${name}`);
    }
    if (name !== undefined) {
      names.add(name);
    }
  }
  return projects;
}

/**
 * Resolves project paths from their authored root without changing the supplied settings.
 *
 * @param project - Validated project with a required CSS path and optional root and tsconfig paths.
 * @param directory - Absolute configuration directory used for a relative or omitted project root.
 * @param paths - Host operations used to resolve each path.
 * @returns A copied project with absolute root and CSS paths. A string tsconfig path is resolved
 * against that root, while false and omission are preserved. Other values retain input references.
 */
export function resolveProjectPaths(
  project: AnalysisProjectConfig,
  directory: string,
  paths: ConfigPaths,
): AnalysisProjectConfig & {
  /** Normalized absolute project directory. */
  root: string;
} {
  const root = paths.resolve(directory, project.root ?? ".");
  return {
    ...project,
    root,
    css: paths.resolve(root, project.css),
    ...(typeof project.tsconfig === "string"
      ? {
          tsconfig: paths.resolve(root, project.tsconfig),
        }
      : {}),
  };
}

/**
 * Removes analysis scope and providers while preserving compiler and delivery settings.
 *
 * @param project - Evaluated project whose names, file scopes, and providers are no longer needed.
 * @returns A shallow copy containing compiler fields and automatic CSS delivery settings. Paths
 * and retained nested objects are unchanged.
 */
export function compilerConfig(project: AnalysisProjectConfig): BaseConfig & BuildOptions {
  const {
    name: _name,
    files: _files,
    exclude: _exclude,
    sources: _sources,
    projection: _projection,
    typeFacts: _typeFacts,
    ...config
  } = project;
  return config;
}

const inlineSchema = strictObject({
  ...compilerEntries,
  configFile: v.optional(v.union([nonemptyText, v.literal(false)])),
  project: v.optional(nonemptyText),
  automatic: v.optional(v.boolean()),
});

/**
 * Validates inline selectors and compiler overrides before selection or path resolution.
 *
 * @param inline - File and project selectors with compiler and delivery overrides.
 * @throws If a field is unknown or invalid, or a lint preset has a cycle. Namespace collisions
 * and required effective lint options can remain deferred until settings are merged.
 */
export function validateInlineConfig(inline: InlineConfig): void {
  validate(inlineSchema, inline, "config");

  const { configFile: _configFile, project: _project, automatic: _automatic, ...compiler } = inline;
  validateCompilerConfig(compiler, "config", true);
}

/**
 * Selects a named project, the sole project, or the unique deepest containing project root.
 *
 * @param projects - Evaluated projects with normalized absolute roots. Entries are not modified.
 * @param applicationRoot - Absolute application directory used for containment when no name is supplied.
 * @param inline - Project name and optional root override. The override changes the directory tested for containment.
 * @param paths - Host operations used to resolve the target and compare it with project roots.
 * @returns The selected input project object. A sole project is selected regardless of containment.
 * @throws If the requested name or containing root does not identify exactly one project.
 */
export function selectProject(
  projects: (AnalysisProjectConfig & {
    /** Normalized absolute root used for containment and depth comparison. */
    root: string;
  })[],
  applicationRoot: string,
  inline: InlineConfig,
  paths: ConfigPaths,
): AnalysisProjectConfig & {
  /** Absolute root retained from the selected input project. */
  root: string;
} {
  let selected;
  if (inline.project !== undefined) {
    selected = projects.filter((project) => project.name === inline.project);
  } else if (projects.length === 1) {
    selected = projects;
  } else {
    const target = paths.resolve(applicationRoot, inline.root ?? ".");
    const matches = projects.filter((project) => {
      const relative = paths.relative(project.root, target).replaceAll("\\", "/");
      return (
        relative === "" ||
        (!paths.isAbsolute(relative) && relative !== ".." && !relative.startsWith("../"))
      );
    });
    const depth = Math.max(...matches.map((project) => project.root.length));
    selected = matches.filter((project) => project.root.length === depth);
  }
  if (selected.length !== 1) {
    throw new Error(
      inline.project !== undefined
        ? `Cannot select Stillwind project ${JSON.stringify(inline.project)}: expected one matching project`
        : "Cannot select one Stillwind project; specify project by name or use a distinct project root",
    );
  }
  return selected[0];
}

/**
 * Applies overrides and defaults, then resolves inline paths from the resulting project root.
 *
 * @param base - Project settings with file-authored root, CSS, and tsconfig paths already resolved.
 * @param inline - Validated selectors and compiler overrides. Selectors are discarded before merging.
 * @param applicationRoot - Absolute base directory for an inline root override or omitted project root.
 * @param paths - Host operations used to resolve paths without checking whether files exist.
 * @returns Normalized compiler and delivery settings with absolute root and CSS paths. Absolute
 * file-authored CSS and tsconfig paths retain their original locations when root is overridden.
 * @throws If effective compiler settings are invalid or no nonempty CSS path is supplied.
 */
export function resolveCompilerSettings(
  base: BaseConfig & BuildOptions,
  inline: InlineConfig,
  applicationRoot: string,
  paths: ConfigPaths,
): NormalizedCompilerConfig & {
  /** Absolute project directory after applying any inline root override. */
  root: string;
  /** Absolute design-system stylesheet path. Existence has not been checked. */
  css: string;
} {
  const { configFile: _file, project: _project, ...overrides } = inline;
  const root =
    overrides.root !== undefined
      ? paths.resolve(applicationRoot, overrides.root)
      : (base.root ?? applicationRoot);
  const config = normalizeCompilerConfig({
    ...mergeConfig(base, overrides),
    lint: mergeLintConfig(base.lint, overrides.lint),
  });
  if (!config.css) {
    throw new Error("config.css: must specify CSS as a nonempty path");
  }

  return {
    ...config,
    root,
    css: paths.resolve(root, config.css),
    ...(typeof config.tsconfig === "string"
      ? {
          tsconfig: paths.resolve(root, config.tsconfig),
        }
      : {}),
  };
}
