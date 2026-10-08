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
  config: string;
  /** Absolute paths whose original and final bytes differ, in sorted order. */
  changed: string[];
  /** Original findings for sorting and final findings for lint fixes, in project and target order. */
  results: WorkspaceResult[];
}

/**
 * Computes all replacements before optionally saving changed files. Fixes return final findings.
 * Throws on empty selection, invalid UTF-8, configuration or planning failures, or failed saving.
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
    const files = await worker.targets(options.patterns);
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

/** Resolves an explicit configuration or searches above cwd. Throws if discovery finds no configuration. */
export function resolveRewriteConfig(options: RewriteOptions): string {
  const cwd = options.cwd ?? process.cwd();
  const config = options.config ? path.resolve(cwd, options.config) : findConfiguration(cwd);
  if (!config) {
    throw new Error(`Cannot find Stillwind configuration above ${cwd}`);
  }
  return config;
}
