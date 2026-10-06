import path from "node:path";

import type { NativeCompilerOptions } from "@stillwind/native";
import type { AnalysisProjectConfig, StillwindOptions } from "stillwind/config";

/** Returns the configured stylesheet request. The caller resolves it against its project root. */
export function cssEntrypoint(options: StillwindOptions): string {
  const entry = options.css ?? options.theme;
  if (!entry) {
    throw new Error("Stillwind requires a CSS-first entrypoint (`css`)");
  }
  return entry;
}

/** Projects host defaults into the native compiler settings. CSS resolves definition settings. */
export function compilerOptions(options: StillwindOptions): NativeCompilerOptions {
  return {
    prefix: options.prefix ?? "--sw-",
    tailwindPrefix: options.tailwindPrefix ?? "--tw-",
    classNamespace: options.classNamespace ?? "",
    useCssIf: options.useCssIf ?? false,
    namingRevision: options.namingRevision ?? "",
    hashLength: options.hashLength,
  };
}

/** Resolves shared project paths before passing these settings to any build integration. */
export function resolveProjectOptions(
  project: AnalysisProjectConfig,
  directory: string,
): StillwindOptions & { root: string } {
  const {
    files: _files,
    exclude: _exclude,
    sources: _sources,
    projection: _projection,
    typeFacts: _typeFacts,
    ...options
  } = project;
  return {
    ...options,
    root: path.resolve(directory, project.root ?? "."),
  };
}
