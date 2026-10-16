import { readFile } from "node:fs/promises";
import path from "node:path";

import { findConfiguration } from "@stillwind/config/node";
import type { WorkspaceResult } from "@stillwind/host";

import { AnalysisWorker } from "./worker-client.ts";
import { writeChangedFiles } from "./write.ts";

/** Selects configured source files for CLI analysis and rewriting. */
export interface RewriteOptions {
  /** Explicit configuration path, absolute or relative to cwd. Omission searches upward from cwd. */
  config?: string;
  /** Directory for configuration discovery and relative config paths. Defaults to process.cwd(). */
  cwd?: string;
  /** Additional globs relative to each project root. Omission or an empty list retains all configured targets. */
  patterns?: string[];
}

/** A completed rewrite batch with findings kept separately for each owning project. */
interface RewriteFilesResult {
  /** Absolute configuration path used by the isolated worker. */
  config: string;
  /** Absolute paths whose original and final bytes differ, in sorted order. */
  changed: string[];
  /** Original findings for sorting and final findings for lint fixes, in project and target order. */
  results: WorkspaceResult[];
}

/**
 * Computes all replacements before optionally saving changed files. Fixes return final findings.
 * @param options - Configuration discovery and optional filters within configured target scopes.
 * @param operation - Utility sorting or converged lint fixes.
 * @param write - Whether to verify and save changed bytes after every replacement is planned.
 * @returns Configuration, sorted changed paths, and findings grouped in project order.
 * @throws If selection is empty, a source is not valid UTF-8, or discovery, reading, rewriting,
 * saving, or worker cleanup fails.
 */
export async function rewriteFiles(
  options: RewriteOptions,
  operation: "sort" | "fixes",
  write: boolean,
): Promise<RewriteFilesResult> {
  const config = resolveRewriteConfig(options);
  const worker = new AnalysisWorker();
  try {
    await worker.open(config);
    // Lint fixes also edit the stylesheets that each project lints.
    const files = await worker.targets(options.patterns, operation === "fixes");
    if (!files.length) {
      throw new Error("No analysis targets matched the configured scopes");
    }
    const originals = await Promise.all(
      files.map(async (file) => ({ file, before: await readFile(file) })),
    );
    const inputs = originals.map(({ file, before }) => {
      const source = before.toString("utf8");
      if (!Buffer.from(source).equals(before)) {
        throw new Error(`Source is not valid UTF-8: ${file}`);
      }
      return { file, source };
    });
    const outputs = await worker.rewrite(inputs, operation);
    const changes = originals.map((original, index) => ({
      ...original,
      after: Buffer.from(outputs[index].code),
    }));
    if (write) {
      await writeChangedFiles(changes);
    }
    return {
      config,
      changed: changes
        .filter((change) => !change.before.equals(change.after))
        .map((change) => change.file),
      results: outputs.flatMap((output) => output.results).sort((a, b) => a.project - b.project),
    };
  } finally {
    await worker.close();
  }
}

/**
 * Resolves an explicit configuration path or searches above the chosen working directory.
 * @param options - Optional config path and cwd, defaulting to process.cwd().
 * @returns An absolute path. Explicit paths are resolved without evaluating the configuration.
 * @throws If discovery finds no configuration.
 */
export function resolveRewriteConfig(options: RewriteOptions): string {
  const cwd = options.cwd ?? process.cwd();
  const config = options.config ? path.resolve(cwd, options.config) : findConfiguration(cwd);
  if (!config) {
    throw new Error(`Cannot find Stillwind configuration above ${cwd}`);
  }
  return config;
}
