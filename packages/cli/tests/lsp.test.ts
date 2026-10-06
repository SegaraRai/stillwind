import { spawn } from "node:child_process";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";

import { expect, test } from "vite-plus/test";

test("LSP publishes UTF-16 diagnostics for unsaved buffers, clears recovered errors, and keeps stdout framed", async () => {
  const root = await mkdtemp(path.resolve(import.meta.dirname, "../../host/.test-lsp-"));
  const child = spawn(
    process.execPath,
    [path.resolve(import.meta.dirname, "../dist/main.js"), "lsp", "--stdio"],
    { cwd: root, windowsHide: true },
  );
  const closed = new Promise<number | null>((resolve) => {
    child.on("close", resolve);
  });
  let stderr = "";
  child.stderr.on("data", (data: Buffer) => {
    stderr += data.toString();
  });
  const messages: {
    id?: number;
    method?: string;
    params?: {
      uri?: string;
      version?: number;
      diagnostics?: {
        code?: string;
        severity?: number;
        range: {
          start: {
            line: number;
            character: number;
          };
        };
      }[];
    };
    result?: unknown;
    error?: unknown;
  }[] = [];
  let buffer = Buffer.alloc(0);
  let protocolError: string | undefined;
  child.stdout.on("data", (data: Buffer) => {
    buffer = Buffer.concat([buffer, data]);
    for (;;) {
      const boundary = buffer.indexOf("\r\n\r\n");
      if (boundary < 0) {
        return;
      }
      const header = buffer.subarray(0, boundary).toString();
      const length = /^Content-Length: (\d+)$/im.exec(header);
      if (!length || !header.startsWith("Content-Length:")) {
        protocolError = header;
        return;
      }
      const end = boundary + 4 + Number(length[1]);
      if (buffer.length < end) {
        return;
      }
      messages.push(JSON.parse(buffer.subarray(boundary + 4, end).toString()));
      buffer = buffer.subarray(end);
    }
  });
  const send = (method: string, params: unknown, id?: number) => {
    const body = JSON.stringify({
      jsonrpc: "2.0",
      method,
      params,
      ...(id !== undefined ? { id } : {}),
    });
    child.stdin.write(`Content-Length: ${Buffer.byteLength(body)}\r\n\r\n${body}`);
  };
  const wait = async (predicate: (message: (typeof messages)[number]) => boolean) => {
    await expect
      .poll(() => protocolError ?? messages.some(predicate), { timeout: 15000, interval: 20 })
      .toBe(true);
    return messages.find(predicate)!;
  };
  try {
    await writeFile(
      path.join(root, "stillwind.config.ts"),
      'console.log("LSP config log");export default {projects:[{css:"root.css",files:["*.tsx"]}]};',
    );
    await writeFile(
      path.join(root, "root.css"),
      "@utility red{color:red} @stillwind-slot utilities; @stillwind{unknown-utility:deny;}",
    );
    const uri = pathToFileURL(path.join(root, "entry.tsx")).href;
    send("initialize", { rootUri: pathToFileURL(root).href, capabilities: {} }, 1);
    const initialized = await wait((message) => message.id === 1);
    expect(initialized.error, stderr).toBeUndefined();
    send("initialized", {});
    const source =
      'import {style} from "stillwind/react";\nconst 日本語="🧭"; export const props=style("missing");';
    send("textDocument/didOpen", {
      textDocument: { uri, languageId: "typescriptreact", version: 1, text: source },
    });
    const first = await wait(
      (message) =>
        message.method === "textDocument/publishDiagnostics" &&
        message.params?.uri === uri &&
        message.params.version === 1 &&
        Boolean(message.params.diagnostics?.length),
    );
    expect(first.params!.diagnostics![0]).toMatchObject({
      code: "unknown-utility",
      range: { start: { line: 1, character: source.split("\n")[1].indexOf("missing") } },
    });
    send("textDocument/didChange", {
      textDocument: { uri, version: 2 },
      contentChanges: [{ text: source.replace("missing", "red") }],
    });
    await wait(
      (message) =>
        message.method === "textDocument/publishDiagnostics" &&
        message.params?.uri === uri &&
        message.params.version === 2 &&
        message.params.diagnostics?.length === 0,
    );
    send("textDocument/didChange", {
      textDocument: { uri, version: 3 },
      contentChanges: [{ text: "const broken=(" }],
    });
    await wait(
      (message) =>
        message.method === "textDocument/publishDiagnostics" &&
        message.params?.version === 3 &&
        Boolean(message.params.diagnostics?.length),
    );
    send("textDocument/didChange", {
      textDocument: { uri, version: 4 },
      contentChanges: [{ text: source.replace("missing", "red") }],
    });
    await wait(
      (message) =>
        message.method === "textDocument/publishDiagnostics" &&
        message.params?.version === 4 &&
        message.params.diagnostics?.length === 0,
    );
    send("textDocument/didChange", {
      textDocument: {
        uri,
        version: 5,
      },
      contentChanges: [
        {
          text: 'import {style} from "stillwind/react";import {external} from "./missing";export const a=style(external);export const local=style("missing");',
        },
      ],
    });
    const partial = await wait(
      (message) =>
        message.method === "textDocument/publishDiagnostics" &&
        message.params?.version === 5 &&
        Boolean(
          message.params.diagnostics?.some((diagnostic) => diagnostic.code === "SW_UNVERIFIED"),
        ),
    );
    expect(partial.params!.diagnostics!.map((diagnostic) => diagnostic.code)).toEqual([
      "unknown-utility",
      "SW_UNVERIFIED",
    ]);
    expect(
      partial.params!.diagnostics!.find((diagnostic) => diagnostic.code === "SW_UNVERIFIED")
        ?.severity,
    ).toBe(3);
    expect(stderr).toContain("LSP config log");
    expect(protocolError).toBeUndefined();
    send("shutdown", null, 2);
    await wait((message) => message.id === 2);
    send("exit", null);
    expect(await closed).toBe(0);
  } finally {
    if (child.exitCode === null) {
      child.kill();
    }
    await closed;
    await rm(root, { recursive: true, force: true });
  }
});
