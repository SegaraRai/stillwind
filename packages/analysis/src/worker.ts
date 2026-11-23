import path from "node:path";
import process from "node:process";

import { resolveLintConfig } from "@stillwind/config";
import { AnalysisWorkspace } from "@stillwind/host";

import {
  applyFormatHooks,
  hasActiveHooks,
  ProjectHookSession,
  type HookRunOptions,
} from "./hooks.ts";
import type { ActiveLintRule, ProjectInfo } from "./info.ts";
import {
  RewriteFailure,
  SourceRewriter,
  type RewriteBatchResult,
  type RewriteInput,
  type RewriteOperation,
} from "./rewrite.ts";
import type { WorkerRequest } from "./worker-client.ts";

let workspace: AnalysisWorkspace | undefined;
let rewriter: SourceRewriter | undefined;

process.on(
  "message",
  (message: {
    /** Sequence number copied into the IPC reply. */
    id: number;
    /** Operation payload sent by AnalysisWorker. Callers serialize workspace replacement and rewrites. */
    request: WorkerRequest;
  }) => {
    void handle(message.request).then(
      (value) => {
        process.send?.({ id: message.id, value }, () => {
          if (message.request.method === "close") {
            process.disconnect?.();
          }
        });
      },
      (error: unknown) => {
        process.send?.({
          id: message.id,
          error: {
            message: error instanceof Error ? error.message : String(error),
            results: error instanceof RewriteFailure ? error.results : undefined,
          },
        });
      },
    );
  },
);

process.on("disconnect", () => {
  void Promise.resolve(workspace?.close()).finally(() => {
    process.exit();
  });
});

/**
 * Dispatches one IPC operation against the child-owned workspace and rewriter.
 *
 * @param request - Operation payload. Open must complete before other operations except close.
 * @returns The operation result, including a boolean for refresh, or undefined for state changes.
 * The targets operation returns sorted, distinct paths.
 * @throws If no workspace is open or the selected workspace operation fails.
 */
async function handle(request: WorkerRequest): Promise<unknown> {
  if (request.method === "open") {
    await workspace?.close();
    workspace = undefined;
    workspace = await AnalysisWorkspace.open(request.config);
    rewriter = new SourceRewriter(workspace);
    return undefined;
  }

  if (!workspace) {
    if (request.method === "close") {
      return undefined;
    }
    throw new Error("Stillwind workspace has not been opened");
  }

  switch (request.method) {
    case "targets": {
      const files = await Promise.all(
        workspace.projects.map((project) =>
          workspace!.targets(project, request.patterns ? [...request.patterns] : undefined, {
            stylesheets: request.stylesheets,
          }),
        ),
      );
      return [...new Set(files.flat())].sort();
    }

    case "rewrite":
      return runRewriteWithHooks(
        workspace,
        rewriter!,
        request.inputs,
        request.operation,
        request.hooks,
      );

    case "lint":
      return workspace.lint(request.patterns ? [...request.patterns] : undefined);

    case "analyze":
      return workspace.analyze(request.file);

    case "prepare":
      return workspace.prepare(request.files);

    case "query":
      return workspace.query(request.file, request.request);

    case "info": {
      const projects: ProjectInfo[] = [];
      for (const [index, project] of workspace.projects.entries()) {
        const matched = await workspace.targets(project);
        const resolved = resolveLintConfig(project.config.lint).rules;
        const lint: Record<string, ActiveLintRule> = {};
        for (const rule of Object.keys(resolved).sort()) {
          const setting = resolved[rule as keyof typeof resolved];
          if (typeof setting === "string") {
            if (setting !== "off") {
              lint[rule] = setting;
            }
          } else if (setting.severity !== "off") {
            lint[rule] = setting as ActiveLintRule;
          }
        }
        projects.push({
          project: index,
          ...(project.config.name ? { name: project.config.name } : {}),
          root: project.analysis.host.root,
          css: path.resolve(project.analysis.host.root, project.config.css),
          files: [...project.config.files],
          exclude: [...(project.config.exclude ?? [])],
          matchedFiles: matched.length,
          plugins: (project.config.plugins ?? []).map((plugin) => plugin.name),
          hooks: (project.config.hooks ?? []).map((hook) => hook.name),
          sort: project.config.sort !== false,
          typeFacts: project.config.typeFacts !== false,
          lint,
        });
      }
      return projects;
    }

    case "designSystem":
      return workspace.designSystem(request.request);

    case "update":
      workspace.update(request.file, request.source, request.version);
      return undefined;

    case "closeDocument":
      workspace.closeDocument(request.file);
      return undefined;

    case "invalidate":
      workspace.invalidate();
      return undefined;

    case "refresh":
      return workspace.refresh(request.files);

    case "affects":
      return workspace.affects(request.files);

    case "close":
      await workspace.close();
      workspace = undefined;
      return undefined;
  }
}

/**
 * Runs Stillwind rewriting followed by configured project `format` hooks for CLI `fmt` and `check`
 * commands.
 */
async function runRewriteWithHooks(
  currentWorkspace: AnalysisWorkspace,
  currentRewriter: SourceRewriter,
  inputs: readonly RewriteInput[],
  operation: RewriteOperation,
  hooks: HookRunOptions | undefined,
): Promise<RewriteBatchResult> {
  const batch = await currentRewriter.rewrite(inputs, operation);
  if (!hooks || !hasActiveHooks(currentWorkspace.projects)) {
    return batch;
  }

  const originals = new Map(inputs.map((input) => [input.file, input.source]));
  const session = await ProjectHookSession.open(
    currentWorkspace.projects,
    hooks.command,
    hooks.mode,
  );
  try {
    const formatted = await applyFormatHooks(currentWorkspace.projects, batch, originals, session);
    if (
      hooks.mode === "write" &&
      hooks.command === "check" &&
      formatted.changed &&
      formatted.outputs.length
    ) {
      await currentRewriter.reanalyze(inputs, formatted.outputs);
    }
    return {
      outputs: formatted.outputs,
      failures: formatted.failures,
    };
  } finally {
    await session.close();
  }
}
