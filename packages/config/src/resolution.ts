/** Selects projects and resolves compiler settings through host-supplied path operations, without reading files. */

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
import type { AnalysisProjectConfig } from "./configuration.ts";
import { mergeLintConfig } from "./lint.ts";
import { mergeConfig } from "./merge.ts";
import { nonemptyText, strictObject, validate } from "./validation.ts";

/** Inline options that can pass through isolated loader workers. */
export type SerializableInlineConfig = Omit<InlineConfig, "naming" | "plugins" | "tailwind">;

/** Build overrides and the selection of a configuration file and project. */
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

/** Path operations supplied by the host's filesystem or virtual workspace. */
export interface ConfigPaths {
  /**
   * Combines path segments and removes relative traversal segments.
   * @param paths - Path segments interpreted using the host's path syntax and working directory.
   * @returns A normalized absolute path. An absolute segment replaces preceding segments.
   */
  resolve(...paths: string[]): string;
  /**
   * Gets the parent directory used to walk configuration candidates.
   * @param path - Normalized absolute path in the same syntax accepted by resolve.
   * @returns Its absolute parent directory. A filesystem root must return itself to end traversal.
   */
  dirname(path: string): string;
  /**
   * Computes the path used to determine project containment.
   * @param from - Absolute project root.
   * @param to - Absolute application directory.
   * @returns A relative path, or an absolute path when the host cannot express a relative path.
   * The same location returns an empty string.
   */
  relative(from: string, to: string): string;
  /**
   * Recognizes absolute paths using the same syntax as the other operations.
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
 * Resolves project paths from their authored root without changing the supplied settings.
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
