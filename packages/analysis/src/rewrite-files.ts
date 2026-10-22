import { readFile } from "node:fs/promises";
import path from "node:path";

import { findConfiguration } from "@stillwind/config/node";
import type { WorkspaceResult } from "@stillwind/host";

import type { RewriteFileFailure, RewriteInput } from "./rewrite.ts";
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
  /** Targets that could not be read, rewritten, or saved. */
  failures: RewriteFileFailure[];
}

/**
 * Computes all replacements before optionally saving changed files. Fixes return final findings.
 * @param options - Configuration discovery and optional filters within configured target scopes.
 * @param operation - Utility sorting or converged lint fixes.
 * @param write - Whether to verify and save changed bytes after every replacement is planned.
 * @returns Configuration, sorted changed paths, findings, and independent target failures.
 * Saving reports only paths whose writes succeeded. Check mode reports planned changes.
 * @throws If selection is empty, configuration, worker communication, or cleanup fails.
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
    const files = await worker.targets(options.patterns, true);
    if (!files.length) {
      throw new Error("No analysis targets matched the configured scopes");
    }
    const failures: RewriteFileFailure[] = [];
    const originals = new Map<string, Buffer>();
    const inputs: RewriteInput[] = [];
    const reads = await Promise.allSettled(files.map((file) => readFile(file)));
    for (const [index, read] of reads.entries()) {
      const file = files[index];
      try {
        if (read.status === "rejected") {
          throw read.reason;
        }
        const source = read.value.toString("utf8");
        if (!Buffer.from(source).equals(read.value)) {
          throw new Error(`Source is not valid UTF-8: ${file}`);
        }
        originals.set(file, read.value);
        inputs.push({
          file,
          source,
        });
      } catch (error) {
        failures.push({
          file,
          message: error instanceof Error ? error.message : String(error),
          results: [],
        });
      }
    }
    const batch = await worker.rewrite(inputs, operation);
    failures.push(...batch.failures);
    const changes = batch.outputs.map((output) => ({
      file: output.file,
      before: originals.get(output.file)!,
      after: Buffer.from(output.code),
    }));
    let changed = changes
      .filter((change) => !change.before.equals(change.after))
      .map((change) => change.file);
    if (write) {
      const saved = await writeChangedFiles(changes);
      changed = saved.changed;
      failures.push(...saved.failures);
    }
    changed.sort();
    failures.sort((a, b) => a.file.localeCompare(b.file));
    const results = [...batch.outputs, ...batch.failures]
      .flatMap((output) => output.results)
      .sort((a, b) => a.project - b.project || a.result.entry.localeCompare(b.result.entry));
    return {
      config,
      changed,
      results,
      failures,
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
