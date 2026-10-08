import * as paths from "pathe";
import { resolve, dirname } from "pathe";

import { resolveConfig, type AnalysisProjectConfig, type InlineConfig } from "./index.ts";
import {
  configurationCandidates,
  compilerConfig,
  resolveProjectPaths,
  resolveCompilerSettings,
  selectProject,
  validateInlineConfig,
} from "./resolution.ts";

/** Configuration evaluation supplied by a browser module runtime. */
export interface BrowserConfigHost {
  /** Tests whether an absolute virtual path exists in the captured workspace. */
  exists(id: string): boolean;
  /** Evaluates a module and returns its default export without invoking a configuration factory. */
  evaluate(id: string): Promise<unknown>;
}

/**
 * Discovers and evaluates browser configuration using the same export contract as Node.
 * @param root - Absolute virtual application directory.
 * @param host - Captured file inventory and browser module evaluator.
 * @param inline - Explicit file, project selection, and compiler overrides.
 * @returns The selected project, configuration identity, and every evaluated project.
 * @throws If an explicit file is missing, evaluation fails, or project selection is ambiguous.
 */
export async function resolveBrowserConfig(
  root: string,
  host: BrowserConfigHost,
  inline: InlineConfig = {},
): Promise<{
  config: AnalysisProjectConfig;
  configFile?: string;
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
    ? selectProject(projects as (AnalysisProjectConfig & { root: string })[], root, inline, paths)
    : undefined;
  const settings = resolveCompilerSettings(
    selected ? compilerConfig(selected) : { root },
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
 * @param requested - File relative to root; false disables discovery.
 * @returns Evaluated projects with absolute roots, or undefined when discovery finds no file.
 * @throws If an explicit path is invalid or missing, or evaluation or validation fails.
 */
export async function loadBrowserConfig(
  root: string,
  host: BrowserConfigHost,
  requested?: string | false,
): Promise<{ configFile: string; projects: AnalysisProjectConfig[] } | undefined> {
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
