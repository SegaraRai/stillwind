import { AnalysisWorkspace } from "@stillwind/host";

import type { WorkerRequest } from "./worker-client.ts";

let workspace: AnalysisWorkspace | undefined;

process.on("message", (message: { id: number; request: WorkerRequest }) => {
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
        error: String(error),
      });
    },
  );
});

process.on("disconnect", () => {
  void Promise.resolve(workspace?.close()).finally(() => {
    process.exit();
  });
});

async function handle(request: WorkerRequest): Promise<unknown> {
  if (request.method === "open") {
    await workspace?.close();
    workspace = undefined;
    workspace = await AnalysisWorkspace.open(request.config);
    return undefined;
  }

  if (!workspace) {
    if (request.method === "close") {
      return undefined;
    }
    throw new Error("Stillwind workspace has not been opened");
  }

  switch (request.method) {
    case "lint":
      return workspace.lint(request.patterns);

    case "analyze":
      return workspace.analyze(request.file);

    case "query":
      return workspace.query(request.file, request.request);

    case "update":
      workspace.update(request.file, request.source, request.version);
      return undefined;

    case "closeDocument":
      workspace.closeDocument(request.file);
      return undefined;

    case "invalidate":
      workspace.invalidate();
      return undefined;

    case "close":
      await workspace.close();
      workspace = undefined;
      return undefined;
  }
}
