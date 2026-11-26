import type { AnalysisHook, HookCommand, HookContext, HookMode } from "@stillwind/config";
import type { WorkspaceProject } from "@stillwind/host";

import type { RewriteBatchResult, RewriteFileFailure, RewriteResult } from "./rewrite.ts";

/** Options selecting which project hooks run for a worker rewrite operation. */
export interface HookRunOptions {
  /** CLI command invoking the hooks. */
  command: HookCommand;
  /** Whether modified buffers will be written to disk or only checked. */
  mode: HookMode;
  /**
   * Zero-based indices of the projects whose hooks this rewrite sets up. Omission selects every
   * project. Each project must be selected in exactly one concurrent rewrite, so its `setup` and
   * `teardown` run once per command.
   */
  projects?: readonly number[];
}

/**
 * Tests whether any project in the workspace declares at least one hook.
 *
 * @param projects - Opened workspace projects.
 * @returns True when at least one project has a configured hook.
 */
export function hasActiveHooks(projects: readonly WorkspaceProject[]): boolean {
  return projects.some((project) => (project.config.hooks?.length ?? 0) > 0);
}

/**
 * Manages per-project hook contexts and `setup` / `teardown` lifetimes for one command run.
 */
export class ProjectHookSession {
  readonly #contexts: readonly HookContext[];
  readonly #started: { hook: AnalysisHook; context: HookContext }[] = [];

  private constructor(contexts: readonly HookContext[]) {
    this.#contexts = contexts;
  }

  /**
   * Creates contexts for all projects and runs `setup` of the selected projects in project and
   * array order. If a `setup` callback throws, earlier initialized hooks are torn down before
   * rethrowing.
   *
   * @param projects - Opened workspace projects.
   * @param command - Invoking CLI command.
   * @param mode - Write or check mode.
   * @param selected - Zero-based indices of the projects whose hooks run. Omission selects every
   * project.
   * @returns An open session whose `close()` must be awaited in a `finally` block.
   * @throws If any hook's `setup` callback throws.
   */
  static async open(
    projects: readonly WorkspaceProject[],
    command: HookCommand,
    mode: HookMode,
    selected?: readonly number[],
  ): Promise<ProjectHookSession> {
    const controller = new AbortController();
    const contexts: HookContext[] = projects.map((project) => ({
      root: project.analysis.host.root,
      ...(project.config.name !== undefined ? { project: project.config.name } : {}),
      command,
      mode,
      signal: controller.signal,
    }));
    const session = new ProjectHookSession(contexts);
    try {
      for (const [index, project] of projects.entries()) {
        if (selected && !selected.includes(index)) {
          continue;
        }
        const context = contexts[index];
        for (const hook of project.config.hooks ?? []) {
          if (hook.setup) {
            try {
              await hook.setup(context);
            } catch (error) {
              throw new Error(
                `Hook "${hook.name}" setup failed: ${error instanceof Error ? error.message : String(error)}`,
                { cause: error },
              );
            }
          }
          session.#started.push({ hook, context });
        }
      }
      return session;
    } catch (error) {
      await session.close().catch(() => undefined);
      throw error;
    }
  }

  /**
   * Returns the immutable hook context for a project index.
   *
   * @param projectIndex - Zero-based project index in the workspace.
   * @returns The project's hook context.
   */
  context(projectIndex: number): HookContext {
    return this.#contexts[projectIndex];
  }

  /**
   * Runs `teardown` in reverse initialization order for all hooks whose `setup` succeeded.
   *
   * @throws The first teardown error after attempting to tear down every remaining hook.
   */
  async close(): Promise<void> {
    let firstError: unknown;
    for (let index = this.#started.length - 1; index >= 0; index -= 1) {
      const { hook, context } = this.#started[index];
      if (!hook.teardown) {
        continue;
      }
      try {
        await hook.teardown(context);
      } catch (error) {
        firstError ??= new Error(
          `Hook "${hook.name}" teardown failed: ${error instanceof Error ? error.message : String(error)}`,
          { cause: error },
        );
      }
    }
    this.#started.length = 0;
    if (firstError !== undefined) {
      throw firstError;
    }
  }
}

/**
 * Tests whether two hook arrays contain the exact same hook object references in the same order.
 */
function sameHookList(a: readonly AnalysisHook[], b: readonly AnalysisHook[]): boolean {
  return a.length === b.length && a.every((hook, index) => hook === b[index]);
}

/**
 * Runs one project's format hook pipeline over a single buffer.
 */
async function runProjectFormatHooks(
  hooks: readonly AnalysisHook[],
  file: string,
  initialCode: string,
  original: string,
  context: HookContext,
): Promise<string> {
  let code = initialCode;
  for (const hook of hooks) {
    let next: unknown;
    try {
      next = await hook.format!(
        {
          path: file,
          code,
          original,
          changed: code !== original,
        },
        context,
      );
    } catch (error) {
      throw new Error(
        `Hook "${hook.name}" failed for ${file}: ${error instanceof Error ? error.message : String(error)}`,
        { cause: error },
      );
    }
    if (typeof next === "string") {
      code = next;
    } else if (next !== undefined) {
      throw new Error(`Hook "${hook.name}" format must return a string or undefined`);
    }
  }
  return code;
}

/**
 * Applies project `format` hooks in memory to surviving rewrite outputs.
 *
 * When a file belongs to multiple projects:
 * - If every owning project has the exact same format hook list (by object identity), the pipeline
 *   runs once.
 * - Otherwise, each owning project runs its own pipeline and conflicting results fail that file.
 *
 * @param projects - Opened workspace projects.
 * @param batch - Rewrite outputs and existing failures.
 * @param originals - Original file text read from disk, keyed by absolute file path.
 * @param session - Open hook session supplying per-project contexts.
 * @returns Updated outputs, combined failures, and whether any output buffer changed.
 */
export async function applyFormatHooks(
  projects: readonly WorkspaceProject[],
  batch: RewriteBatchResult,
  originals: ReadonlyMap<string, string>,
  session: ProjectHookSession,
): Promise<{
  outputs: RewriteResult[];
  failures: RewriteFileFailure[];
  changed: boolean;
}> {
  const outputs: RewriteResult[] = [];
  const failures: RewriteFileFailure[] = [...batch.failures];
  let changed = false;

  for (const output of batch.outputs) {
    const projectIndices = [...new Set(output.results.map((entry) => entry.project))];
    const pipelines = projectIndices.map((projectIndex) => ({
      projectIndex,
      hooks: (projects[projectIndex]?.config.hooks ?? []).filter(
        (hook) => typeof hook.format === "function",
      ),
    }));
    if (!pipelines.some((entry) => entry.hooks.length > 0)) {
      outputs.push(output);
      continue;
    }

    const original = originals.get(output.file) ?? output.code;
    try {
      let nextCode: string;
      const first = pipelines[0];
      if (pipelines.every((entry) => sameHookList(entry.hooks, first.hooks))) {
        nextCode = await runProjectFormatHooks(
          first.hooks,
          output.file,
          output.code,
          original,
          session.context(first.projectIndex),
        );
      } else {
        const candidates: string[] = [];
        for (const entry of pipelines) {
          candidates.push(
            entry.hooks.length
              ? await runProjectFormatHooks(
                  entry.hooks,
                  output.file,
                  output.code,
                  original,
                  session.context(entry.projectIndex),
                )
              : output.code,
          );
        }
        nextCode = candidates[0];
        if (candidates.some((candidate) => candidate !== nextCode)) {
          throw new Error(`Conflicting project format hooks for ${output.file}`);
        }
      }
      if (nextCode !== output.code) {
        changed = true;
      }
      outputs.push({
        ...output,
        code: nextCode,
        edits: nextCode === output.code ? output.edits : [],
      });
    } catch (error) {
      failures.push({
        file: output.file,
        message: error instanceof Error ? error.message : String(error),
        results: output.results,
      });
    }
  }

  return { outputs, failures, changed };
}
