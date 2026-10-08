import {
  mergeConfig,
  normalizeCompilerConfig,
  validateCompilerConfig,
  type AnalysisProjectConfig,
  type BaseConfig,
  type BuildOptions,
  type InlineConfig,
  type NormalizedCompilerConfig,
} from "./index.ts";

/** Path operations supplied by the host's filesystem or virtual workspace. */
export interface ConfigPaths {
  resolve(...paths: string[]): string;
  dirname(path: string): string;
  relative(from: string, to: string): string;
  isAbsolute(path: string): boolean;
}

/** Returns nearest-first configuration candidates up to the filesystem or virtual root. */
export function configurationCandidates(directory: string, paths: ConfigPaths): string[] {
  const candidates: string[] = [];
  let current = paths.resolve(directory);
  for (;;) {
    candidates.push(paths.resolve(current, "stillwind.config.ts"));
    const parent = paths.dirname(current);
    if (parent === current) {
      return candidates;
    }
    current = parent;
  }
}

/** Resolves project paths from their authored root without changing the supplied settings. */
export function resolveProjectPaths(
  project: AnalysisProjectConfig,
  directory: string,
  paths: ConfigPaths,
): AnalysisProjectConfig & { root: string } {
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

/** Removes analysis scope and providers while preserving compiler and delivery settings. */
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

/** Validates inline selectors and compiler overrides before selection or path resolution. */
export function validateInlineConfig(inline: InlineConfig): void {
  const { configFile, project, automatic, ...compiler } = inline;
  if (
    configFile !== undefined &&
    configFile !== false &&
    (typeof configFile !== "string" || !configFile.trim())
  ) {
    throw new Error("Stillwind configFile must be a path or false");
  }
  if (project !== undefined && (typeof project !== "string" || !project.trim())) {
    throw new Error("Stillwind project must be a nonempty name");
  }
  if (automatic !== undefined && typeof automatic !== "boolean") {
    throw new Error("config.automatic: must be boolean");
  }

  validateCompilerConfig(compiler, "config", true);
}

/** Selects one project by name or the deepest root containing the requested application directory. Throws on ambiguity. */
export function selectProject(
  projects: (AnalysisProjectConfig & { root: string })[],
  applicationRoot: string,
  inline: InlineConfig,
  paths: ConfigPaths,
): AnalysisProjectConfig & { root: string } {
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

/** Applies overrides and defaults. File-authored CSS and tsconfig paths retain their original project root. Inline paths use the resulting root. */
export function resolveCompilerSettings(
  base: BaseConfig & BuildOptions,
  inline: InlineConfig,
  applicationRoot: string,
  paths: ConfigPaths,
): NormalizedCompilerConfig & {
  root: string;
  css: string;
} {
  const { configFile: _file, project: _project, ...overrides } = inline;
  const root =
    overrides.root !== undefined
      ? paths.resolve(applicationRoot, overrides.root)
      : (base.root ?? applicationRoot);
  const config = normalizeCompilerConfig(mergeConfig(base, overrides));
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
