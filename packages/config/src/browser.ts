import { dirname, resolve, relative } from "pathe";

import {
  mergeConfig,
  resolveConfig,
  validateConfig,
  type AnalysisProjectConfig,
  type InlineConfig,
} from "./index.ts";

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

  const loaded = await loadBrowserConfig(root, host, requested);
  const file = loaded?.configFile;
  const projects = loaded?.projects ?? [];
  let selected: AnalysisProjectConfig = {
    root,
    css: overrides.css ?? overrides.theme ?? "",
    files: ["**/*"],
  };
  if (file) {
    const target = resolve(root, overrides.root ?? ".");
    const matches =
      name !== undefined
        ? projects.filter((project) => project.name === name)
        : projects.length === 1
          ? projects
          : projects.filter((project) => {
              const path = relative(project.root!, target);
              return (
                path === "" || (!path.startsWith("../") && path !== ".." && !path.startsWith("/"))
              );
            });
    const depth = Math.max(...matches.map((project) => project.root!.length));
    const closest =
      name !== undefined || projects.length === 1
        ? matches
        : matches.filter((project) => project.root!.length === depth);
    if (closest.length !== 1) {
      throw new Error(
        name !== undefined
          ? `Cannot select Stillwind project ${JSON.stringify(name)}: expected one matching project`
          : "Cannot select one Stillwind project; specify project by name or use a distinct project root",
      );
    }
    selected = closest[0];
  } else if (name !== undefined) {
    throw new Error("Cannot select a Stillwind project without a configuration file");
  }

  const config = mergeConfig(selected, overrides);
  if (overrides.root !== undefined) {
    config.root = resolve(root, overrides.root);
  }

  validateConfig({
    projects: [config],
  });

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
    let directory = root;
    while (true) {
      const candidate = resolve(directory, "stillwind.config.ts");
      if (host.exists(candidate)) {
        file = candidate;
        break;
      }
      const parent = dirname(directory);
      if (parent === directory) {
        break;
      }
      directory = parent;
    }
  }
  if (!file) {
    return undefined;
  }
  const config = await resolveConfig(await host.evaluate(file));
  return {
    configFile: file,
    projects: config.projects.map((project) => ({
      ...project,
      root: resolve(dirname(file!), project.root ?? "."),
    })),
  };
}
