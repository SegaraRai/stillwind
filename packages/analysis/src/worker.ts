import process from "node:process";

import { AnalysisWorkspace } from "@stillwind/host";

import { RewriteFailure, SourceRewriter } from "./rewrite.ts";
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
      return rewriter!.rewrite(request.inputs, request.operation);

    case "lint":
      return workspace.lint(request.patterns ? [...request.patterns] : undefined);

    case "analyze":
      return workspace.analyze(request.file);

    case "prepare":
      return workspace.prepare(request.files);

    case "query":
      return workspace.query(request.file, request.request);

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
