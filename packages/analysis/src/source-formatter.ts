import path from "node:path";

import { findConfiguration } from "@stillwind/config/node";

import { RewriteFailure, type RewriteInput, type RewriteResult } from "./rewrite.ts";
import { AnalysisWorker } from "./worker-client.ts";

/** Reuses an isolated process while refreshing trusted configuration for each formatting request. */
export class SourceFormatter {
  /** Child process reused across formatting requests and owned until close completes. */
  readonly #worker = new AnalysisWorker();
  /** Serializes requests while allowing a new request after a preceding failure. */
  #queue: Promise<unknown> = Promise.resolve();

  /**
   * Starts the isolated worker that evaluates configuration on each formatting request.
   * @param config - Configuration path resolved in the worker's inherited working directory.
   */
  constructor(private readonly config: string) {}

  /**
   * Sorts the supplied buffer with current project settings.
   * Files outside all configured scopes return unchanged text. Requests are serialized and never write files.
   * @param input - Original buffer and optional protected ranges. Relative paths are resolved from cwd.
   * @returns The sorted buffer and original findings after configuration is reopened.
   * @throws If configuration loading, parsing, analysis, project agreement, rewriting, or worker communication fails.
   */
  format(input: RewriteInput): Promise<RewriteResult> {
    const task = this.#queue.then(async () => {
      await this.#worker.open(this.config);
      const { outputs, failures } = await this.#worker.rewrite(
        [
          {
            ...input,
            file: path.resolve(input.file),
          },
        ],
        "sort",
      );
      if (failures.length) {
        throw new RewriteFailure(failures[0].message, failures[0].results);
      }
      return outputs[0];
    });
    this.#queue = task.catch(() => {});
    return task;
  }

  /**
   * Waits for queued formatting and releases the child process. Stop submitting requests before closure.
   * @returns Completion of worker cleanup.
   * @throws If the worker's cleanup request fails.
   */
  async close(): Promise<void> {
    await this.#queue;
    await this.#worker.close();
  }
}

/**
 * Finds a configuration above a source file or resolves an explicit path from cwd.
 * @param file - Source path whose parent starts upward discovery. Relative paths use cwd.
 * @param config - Optional explicit configuration path, returned without an existence check.
 * @returns The absolute configuration path, or undefined when discovery finds none.
 */
export function findFormatterConfig(file: string, config?: string): string | undefined {
  if (config) {
    return path.resolve(config);
  }
  return findConfiguration(path.dirname(path.resolve(file)));
}
