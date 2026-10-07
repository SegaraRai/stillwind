import path from "node:path";

import { findConfiguration } from "@stillwind/config/node";

import type { RewriteInput, RewriteResult } from "./rewrite.ts";
import { AnalysisWorker } from "./worker-client.ts";

/** Reuses an isolated process while refreshing trusted configuration for each formatting request. */
export class SourceFormatter {
  readonly #worker = new AnalysisWorker();
  #queue: Promise<unknown> = Promise.resolve();

  constructor(private readonly config: string) {}

  /**
   * Sorts the supplied buffer with current project settings. Relative file paths use cwd.
   * Files outside all configured scopes return unchanged text. Throws when parsing, analysis,
   * or project agreement fails. Requests are serialized and never write files.
   */
  format(input: RewriteInput): Promise<RewriteResult> {
    const task = this.#queue.then(async () => {
      await this.#worker.open(this.config);
      const results = await this.#worker.rewrite(
        [
          {
            ...input,
            file: path.resolve(input.file),
          },
        ],
        "sort",
      );
      return results[0];
    });
    this.#queue = task.catch(() => {});
    return task;
  }

  /** Waits for pending formatting and releases the child process. */
  async close(): Promise<void> {
    await this.#queue;
    await this.#worker.close();
  }
}

/** Finds a formatter configuration above file. An explicit config is resolved from cwd. */
export function findFormatterConfig(file: string, config?: string): string | undefined {
  if (config) {
    return path.resolve(config);
  }
  return findConfiguration(path.dirname(path.resolve(file)));
}
