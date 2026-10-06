import { expect, test } from "vite-plus/test";

test("a browser Worker runs Stillwind LSP with WASM against unsaved in-memory documents", async () => {
  const worker = new Worker(new URL("./language-server.worker.ts", import.meta.url), {
    type: "module",
  });
  const messages: {
    id?: number;
    method?: string;
    error?: unknown;
    params?: {
      uri?: string;
      version?: number;
      diagnostics?: { code: string; range: { start: { character: number } } }[];
    };
  }[] = [];
  let error: string | undefined;
  worker.addEventListener("message", (event) => {
    messages.push(event.data);
  });
  worker.addEventListener("error", (event) => {
    error = event.message;
  });
  const send = (method: string, params: unknown, id?: number) =>
    worker.postMessage({ jsonrpc: "2.0", method, params, ...(id === undefined ? {} : { id }) });
  const wait = async (predicate: (message: (typeof messages)[number]) => boolean) => {
    await expect.poll(() => error ?? messages.some(predicate), { timeout: 20000 }).toBe(true);
    return messages.find(predicate)!;
  };
  try {
    send("initialize", { rootUri: "inmemory://play/", capabilities: {} }, 1);
    expect((await wait((message) => message.id === 1)).error).toBeUndefined();
    send("initialized", {});
    const uri = "inmemory://play/src/entry.tsx";
    const text =
      'import {style} from "stillwind/react";const 日本語="🧭";export const props=style("missing");';
    send("textDocument/didOpen", {
      textDocument: { uri, languageId: "typescriptreact", version: 1, text },
    });
    const result = await wait(
      (message) =>
        message.method === "textDocument/publishDiagnostics" &&
        message.params?.uri === uri &&
        message.params.version === 1 &&
        Boolean(message.params.diagnostics?.length),
    );
    expect(result.params!.diagnostics![0]).toMatchObject({
      code: "unknown-utility",
      range: { start: { character: text.indexOf("missing") } },
    });
    send("textDocument/didChange", {
      textDocument: { uri, version: 2 },
      contentChanges: [{ text: text.replace("missing", "red") }],
    });
    await wait(
      (message) =>
        message.method === "textDocument/publishDiagnostics" &&
        message.params?.uri === uri &&
        message.params.version === 2 &&
        message.params.diagnostics?.length === 0,
    );
    send("shutdown", null, 2);
    expect((await wait((message) => message.id === 2)).error).toBeUndefined();
    send("exit", null);
  } finally {
    worker.terminate();
  }
});
