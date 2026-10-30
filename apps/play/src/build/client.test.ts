import { afterEach, expect, test, vi } from "vite-plus/test";
import type * as vscode from "vscode";

import { ApplicationBuilder, type ApplicationBuild } from "./client.ts";

const builders: ApplicationBuilder[] = [];

afterEach(() => {
  for (const builder of builders.splice(0)) {
    builder.dispose();
  }
  vi.unstubAllGlobals();
});

const snapshot = {
  files: [["/workspace/App.tsx", "export default () => null"]],
  revision: 1,
  generation: 1,
};

const output: ApplicationBuild = {
  code: "window.StillwindApplication = {};",
  css: "",
  transformed: [],
  moduleCss: [],
  rejectedNames: [],
};

class BuildWorker {
  onmessage: ((event: MessageEvent) => Promise<void>) | null = null;
  onerror: ((event: ErrorEvent) => void) | null = null;
  onmessageerror: ((event: MessageEvent) => void) | null = null;
  readonly postMessage = vi.fn<(message: Record<string, unknown>) => void>();
  readonly terminate = vi.fn();

  receive(data: unknown) {
    return this.onmessage?.({ data } as MessageEvent);
  }
}

function setup() {
  const workers: BuildWorker[] = [];
  vi.stubGlobal(
    "Worker",
    class extends BuildWorker {
      constructor() {
        super();
        workers.push(this);
      }
    },
  );
  const executeCommand = vi.fn(async (command: string, _input?: unknown): Promise<unknown> => {
    if (command === "stillwind.snapshot") {
      return snapshot;
    }
    expect.fail(`Unexpected command: ${command}`);
  });
  const builder = new ApplicationBuilder({
    commands: {
      executeCommand,
    },
  } as unknown as typeof vscode);
  builders.push(builder);
  return {
    builder,
    workers,
    executeCommand,
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((accept, fail) => {
    resolve = accept;
    reject = fail;
  });
  return {
    promise,
    resolve,
    reject,
  };
}

async function buildRequest(worker: BuildWorker) {
  return vi.waitFor(() => {
    const request = worker.postMessage.mock.calls.find(
      ([message]) => message.kind === "build",
    )?.[0];
    expect(request).toBeDefined();
    return request!;
  });
}

test("rejects when the worker fails during snapshot capture and recovers on the next build", async () => {
  const { builder, workers, executeCommand } = setup();
  const captured = deferred<typeof snapshot>();
  executeCommand.mockReturnValueOnce(captured.promise);
  const pending = builder.build();
  const failed = expect(pending).rejects.toThrow("Worker initialization failed");
  workers[0].onerror!({
    message: "Worker initialization failed",
  } as ErrorEvent);
  await failed;

  const next = builder.build();
  expect(workers).toHaveLength(2);
  const request = await buildRequest(workers[1]);
  captured.resolve(snapshot);
  await captured.promise;
  expect(workers[0].postMessage).not.toHaveBeenCalled();
  await workers[1].receive({
    kind: "built",
    id: request.id,
    ...output,
  });
  await expect(next).resolves.toMatchObject(output);
});

test.each(["error", "messageerror"] as const)(
  "rejects all active builds after a worker %s and replaces the worker for the next build",
  async (event) => {
    const { builder, workers } = setup();
    const pending = [builder.build(), builder.build()];
    const message =
      event === "error"
        ? "Worker failed while compiling revision 1"
        : "Cannot read a message from the application build worker";
    const failed = pending.map((build) => expect(build).rejects.toThrow(message));
    await vi.waitFor(() => expect(workers[0].postMessage).toHaveBeenCalledTimes(2));
    if (event === "error") {
      workers[0].onerror!({
        message,
      } as ErrorEvent);
    } else {
      workers[0].onmessageerror!({} as MessageEvent);
    }
    await Promise.all(failed);
    expect(workers[0].terminate).toHaveBeenCalledOnce();

    const next = builder.build();
    expect(workers).toHaveLength(2);
    const request = await buildRequest(workers[1]);
    await workers[1].receive({
      kind: "built",
      id: request.id,
      ...output,
    });
    await expect(next).resolves.toMatchObject(output);
  },
);

test.each(["resolve", "reject"] as const)(
  "discards a compile reply that settles with %s after its worker fails",
  async (settle) => {
    const { builder, workers, executeCommand } = setup();
    const compilation = deferred<unknown>();
    const pending = builder.build();
    const failed = expect(pending).rejects.toThrow("Worker failed");
    await buildRequest(workers[0]);
    executeCommand.mockReturnValueOnce(compilation.promise);
    const reply = workers[0].receive({
      kind: "compile",
      id: 1,
      uri: "file:///workspace/App.tsx",
      revision: 1,
      generation: 1,
    });
    workers[0].onerror!({
      message: "Worker failed",
    } as ErrorEvent);
    await failed;

    const next = builder.build();
    expect(workers).toHaveLength(2);
    const request = await buildRequest(workers[1]);
    if (settle === "resolve") {
      compilation.resolve({
        graph: {},
        css: "",
      });
    } else {
      compilation.reject(new Error("Obsolete compilation"));
    }
    await reply;
    expect(workers[0].postMessage).toHaveBeenCalledTimes(1);
    expect(workers[1].postMessage).toHaveBeenCalledTimes(1);
    await workers[1].receive({
      kind: "built",
      id: request.id,
      ...output,
    });
    await expect(next).resolves.toMatchObject(output);
  },
);

test("replaces a worker when posting a build request fails", async () => {
  const { builder, workers } = setup();
  const pending = builder.build();
  workers[0].postMessage.mockImplementationOnce(() => {
    throw new Error("Worker connection failed");
  });
  await expect(pending).rejects.toThrow("Worker connection failed");
  expect(workers[0].terminate).toHaveBeenCalledOnce();

  const next = builder.build();
  expect(workers).toHaveLength(2);
  const request = await buildRequest(workers[1]);
  await workers[1].receive({
    kind: "built",
    id: request.id,
    ...output,
  });
  await expect(next).resolves.toMatchObject(output);
});

test("disposal rejects a build during snapshot capture and prevents later builds", async () => {
  const { builder, workers, executeCommand } = setup();
  const captured = deferred<typeof snapshot>();
  executeCommand.mockReturnValueOnce(captured.promise);
  const pending = builder.build();
  const failed = expect(pending).rejects.toThrow("Application builder is closed");
  builder.dispose();
  await failed;
  captured.resolve(snapshot);
  await captured.promise;
  expect(workers[0].postMessage).not.toHaveBeenCalled();
  await expect(builder.build()).rejects.toThrow("Application builder is closed");
  expect(workers).toHaveLength(1);
});

test("retries an obsolete snapshot with the current worker without reloading settings again", async () => {
  const { builder, workers, executeCommand } = setup();
  const pending = builder.build(true);
  const request = await buildRequest(workers[0]);
  await workers[0].receive({
    kind: "failed",
    id: request.id,
    error: "Obsolete workspace snapshot",
  });
  await vi.waitFor(() => expect(workers[0].postMessage).toHaveBeenCalledTimes(2));
  const retried = workers[0].postMessage.mock.calls[1][0];
  expect(executeCommand.mock.calls).toEqual([
    ["stillwind.snapshot", { reload: true, metadataOnly: true }],
    ["stillwind.snapshot", { reload: false, metadataOnly: true }],
  ]);
  await workers[0].receive({
    kind: "built",
    id: retried.id,
    ...output,
  });
  await expect(pending).resolves.toMatchObject(output);
});

test.each([
  ["read", "stillwind.readFile"],
  ["compile", "stillwind.compile"],
])(
  "forwards %s requests with the captured revision and replies to the owning worker",
  async (kind, command) => {
    const { builder, workers, executeCommand } = setup();
    const build = builder.build();
    const request = await buildRequest(workers[0]);
    executeCommand.mockResolvedValueOnce("requested contents");
    const input = {
      kind,
      id: 37,
      uri: "file:///workspace/data.json",
      revision: 3,
      generation: 2,
    };
    await workers[0].receive(input);
    expect(executeCommand).toHaveBeenLastCalledWith(command, {
      uri: input.uri,
      revision: 3,
      generation: 2,
    });
    expect(workers[0].postMessage).toHaveBeenLastCalledWith({
      ...input,
      kind: `${kind}-reply`,
      value: "requested contents",
    });
    await workers[0].receive({
      kind: "built",
      id: request.id,
      ...output,
    });
    await expect(build).resolves.toEqual(expect.objectContaining(output));
  },
);
