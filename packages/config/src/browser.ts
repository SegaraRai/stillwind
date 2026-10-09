/** Evaluates and selects configuration from virtual paths through an embedding's captured files and module runtime. */

import * as paths from "pathe";
import { dirname, resolve } from "pathe";

import { resolveConfig, type AnalysisProjectConfig } from "./configuration.ts";
import {
  compilerConfig,
  configurationCandidates,
  resolveCompilerSettings,
  resolveProjectPaths,
  selectProject,
  validateInlineConfig,
  type InlineConfig,
} from "./resolution.ts";

/** Configuration evaluation supplied by a browser module runtime. */
export interface BrowserConfigHost {
  /**
   * Tests whether a configuration candidate exists in the captured workspace.
   * @param id - Absolute virtual path supplied by discovery or an explicit file selection.
   * @returns Whether the evaluator can load this path from the captured inventory.
   */
  exists(id: string): boolean;
  /**
   * Evaluates a configuration module without invoking its exported factory.
   * @param id - Existing absolute virtual path selected from the same captured inventory.
   * @returns The module's default export, which resolveConfig subsequently evaluates and validates.
   * @throws If loading or evaluating the module fails.
   */
  evaluate(id: string): Promise<unknown>;
}

/**
 * Discovers and evaluates browser configuration using the same export contract as Node.
 * @param root - Absolute virtual application directory.
 * @param host - Captured file inventory and browser module evaluator.
 * @param inline - Explicit file, project selection, and compiler overrides.
 * @returns The selected project, configuration identity, and every evaluated project.
 * @throws If an explicit file is missing, evaluation or validation fails, project selection is
 * unsuccessful or ambiguous, or the effective settings lack a nonempty CSS path.
 */
export async function resolveBrowserConfig(
  root: string,
  host: BrowserConfigHost,
  inline: InlineConfig = {},
): Promise<{
  /** Selected project with merged overrides, compiler defaults, and absolute paths. */
  config: AnalysisProjectConfig;
  /** Absolute virtual configuration path, or undefined when no file was loaded. */
  configFile?: string;
  /** Projects in authored order, with the selected entry replaced by config. Inline-only settings return one project. */
  projects: AnalysisProjectConfig[];
}> {
  root = resolve(root);

  validateInlineConfig(inline);

  const loaded = await loadBrowserConfig(root, host, inline.configFile);
  const file = loaded?.configFile;
  const projects = loaded?.projects ?? [];
  if (!file && inline.project !== undefined) {
    throw new Error("Cannot select a Stillwind project without a configuration file");
  }

  const selected = file
    ? selectProject(
        projects as (AnalysisProjectConfig & {
          /** Absolute virtual root resolved from the configuration directory. */
          root: string;
        })[],
        root,
        inline,
        paths,
      )
    : undefined;
  const settings = resolveCompilerSettings(
    selected
      ? compilerConfig(selected)
      : {
          root,
          lint: inline.lint,
        },
    inline,
    root,
    paths,
  );
  const config: AnalysisProjectConfig = {
    ...selected,
    files: selected?.files ?? ["**/*"],
    ...settings,
  };

  return {
    config,
    configFile: file,
    projects: projects.length
      ? projects.map((project) => (project === selected ? config : project))
      : [config],
  };
}

/**
 * Loads every configured project for analysis across a workspace.
 * @param root - Absolute virtual directory where discovery starts.
 * @param host - Captured files and module evaluator.
 * @param requested - Absolute path or nonempty path relative to root. False disables loading, and
 * omission searches root and its ancestors in configurationFiles order within each directory.
 * @returns Evaluated projects with absolute roots, or undefined when loading is disabled or discovery finds no file.
 * @throws If an explicit path is invalid or missing, or evaluation or validation fails.
 */
export async function loadBrowserConfig(
  root: string,
  host: BrowserConfigHost,
  requested?: string | false,
): Promise<
  | {
      /** Absolute virtual path selected for evaluation. */
      configFile: string;
      /** Validated projects in authored order with root, CSS, and string tsconfig paths resolved. */
      projects: AnalysisProjectConfig[];
    }
  | undefined
> {
  if (requested === false) {
    return undefined;
  }
  if (requested !== undefined && (typeof requested !== "string" || !requested.trim())) {
    throw new Error("Stillwind configFile must be a path or false");
  }

  root = resolve(root);

  let file: string | undefined;
  if (requested) {
    file = resolve(root, requested);
    if (!host.exists(file)) {
      throw new Error(`Cannot find Stillwind configuration: ${file}`);
    }
  } else {
    file = configurationCandidates(root, paths).find((candidate) => host.exists(candidate));
  }
  if (!file) {
    return undefined;
  }

  const config = await resolveConfig(await host.evaluate(file));
  return {
    configFile: file,
    projects: config.projects.map((project) => resolveProjectPaths(project, dirname(file!), paths)),
  };
}
