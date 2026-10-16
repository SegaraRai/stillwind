import { spawn } from "node:child_process";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";

import { expect, test } from "vite-plus/test";

/** One JSON-RPC message from the language server. */
interface Message {
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
}

/**
 * Starts the language server over stdio in a new temporary project directory.
 * @param prefix - Name prefix of the temporary directory.
 * @returns The directory, the received messages, and operations to send requests, wait for a
 * message, and stop the server. `stop` waits for the process and removes the directory.
 */
async function startServer(prefix: string) {
  const root = await mkdtemp(path.resolve(import.meta.dirname, `../../host/${prefix}`));
  const child = spawn(
    process.execPath,
    [path.resolve(import.meta.dirname, "../dist/main.js"), "lsp", "--stdio"],
    { cwd: root, windowsHide: true },
  );
  const closed = new Promise<number | null>((resolve) => {
    child.on("close", resolve);
  });
  const state = { stderr: "", protocolError: undefined as string | undefined };
  child.stderr.on("data", (data: Buffer) => {
    state.stderr += data.toString();
  });
  const messages: Message[] = [];
  let buffer = Buffer.alloc(0);
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
        state.protocolError = header;
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
  const wait = async (predicate: (message: Message) => boolean) => {
    await expect
      .poll(() => state.protocolError ?? messages.some(predicate), {
        timeout: 15000,
        interval: 20,
      })
      .toBe(true);
    return messages.find(predicate)!;
  };
  const stop = async () => {
    if (child.exitCode === null) {
      child.kill();
    }
    await closed;
    await rm(root, { recursive: true, force: true });
  };
  return { root, state, messages, send, wait, closed, stop };
}

test("LSP publishes UTF-16 diagnostics for unsaved buffers, clears recovered errors, and keeps stdout framed", async () => {
  const { root, state, send, wait, closed, stop } = await startServer(".test-lsp-");
  try {
    await writeFile(
      path.join(root, "stillwind.config.ts"),
      'console.log("LSP config log");export default {projects:[{css:"root.css",files:["*.tsx"],lint:{rules:{"unknown-utility":"deny"}}}]};',
    );
    await writeFile(
      path.join(root, "root.css"),
      "@utility red{color:red} @stillwind-slot utilities;",
    );
    const uri = pathToFileURL(path.join(root, "entry.tsx")).href;
    send("initialize", { rootUri: pathToFileURL(root).href, capabilities: {} }, 1);
    const initialized = await wait((message) => message.id === 1);
    expect(initialized.error, state.stderr).toBeUndefined();
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
    expect(state.stderr).toContain("LSP config log");
    expect(state.protocolError).toBeUndefined();
    send("shutdown", null, 2);
    await wait((message) => message.id === 2);
    send("exit", null);
    expect(await closed).toBe(0);
  } finally {
    await stop();
  }
});

test("LSP publishes stylesheet diagnostics for unsaved CSS buffers and offers their fixes", async () => {
  const { root, state, send, wait, stop } = await startServer(".test-lsp-css-");
  try {
    await writeFile(
      path.join(root, "stillwind.config.ts"),
      'export default {projects:[{css:"root.css",files:["*.tsx"]}]};',
    );
    const head =
      "@custom-variant hover (&:hover); @utility red{color:red} @stillwind-slot utilities;\n";
    await writeFile(path.join(root, "root.css"), head);
    const uri = pathToFileURL(path.join(root, "root.css")).href;
    send("initialize", { rootUri: pathToFileURL(root).href, capabilities: {} }, 1);
    expect((await wait((message) => message.id === 1)).error, state.stderr).toBeUndefined();
    send("initialized", {});
    send("textDocument/didOpen", {
      textDocument: {
        uri,
        languageId: "css",
        version: 1,
        text: `${head}.card { @apply hover:hover:red; }\n`,
      },
    });
    const published = await wait(
      (message) =>
        message.method === "textDocument/publishDiagnostics" &&
        message.params?.uri === uri &&
        message.params.version === 1 &&
        Boolean(message.params.diagnostics?.length),
    );
    expect(published.params!.diagnostics).toMatchObject([
      {
        code: "redundant-variant",
        range: {
          start: {
            line: 1,
            character: 21,
          },
        },
      },
    ]);
    send(
      "textDocument/codeAction",
      {
        textDocument: { uri },
        range: {
          start: {
            line: 1,
            character: 0,
          },
          end: {
            line: 1,
            character: 30,
          },
        },
        context: {
          diagnostics: [],
          only: ["quickfix"],
        },
      },
      2,
    );
    const actions = await wait((message) => message.id === 2);
    expect(actions.result).toMatchObject([
      {
        title: "Remove the redundant variant",
        edit: {
          documentChanges: [
            {
              textDocument: {
                uri,
                version: 1,
              },
              edits: [
                {
                  range: {
                    start: {
                      line: 1,
                      character: 21,
                    },
                  },
                  newText: "",
                },
              ],
            },
          ],
        },
      },
    ]);
    expect(state.protocolError).toBeUndefined();
  } finally {
    await stop();
  }
});

test("LSP offers the nesting selector fix at a variant definition", async () => {
  const { root, state, send, wait, stop } = await startServer(".test-lsp-css-");
  try {
    await writeFile(
      path.join(root, "stillwind.config.ts"),
      'export default {projects:[{css:"root.css",files:["*.tsx"]}]};',
    );
    const head = "@stillwind-slot utilities;\n";
    await writeFile(path.join(root, "root.css"), head);
    const uri = pathToFileURL(path.join(root, "root.css")).href;
    send("initialize", { rootUri: pathToFileURL(root).href, capabilities: {} }, 1);
    expect((await wait((message) => message.id === 1)).error, state.stderr).toBeUndefined();
    send("initialized", {});
    send("textDocument/didOpen", {
      textDocument: {
        uri,
        languageId: "css",
        version: 1,
        text: `${head}@custom-variant kid (:hover, > .item);\n`,
      },
    });
    const published = await wait(
      (message) =>
        message.method === "textDocument/publishDiagnostics" &&
        message.params?.uri === uri &&
        message.params.version === 1 &&
        Boolean(message.params.diagnostics?.length),
    );
    expect(published.params!.diagnostics).toMatchObject([
      {
        code: "implicit-nesting-selector",
        range: {
          start: { line: 1, character: 21 },
          end: { line: 1, character: 36 },
        },
      },
    ]);
    send(
      "textDocument/codeAction",
      {
        textDocument: { uri },
        range: {
          start: { line: 1, character: 0 },
          end: { line: 1, character: 37 },
        },
        context: {
          diagnostics: [],
          only: ["quickfix"],
        },
      },
      2,
    );
    const actions = await wait((message) => message.id === 2);
    expect(actions.result).toMatchObject([
      {
        title: "Write the nesting selector `&`",
        edit: {
          documentChanges: [
            {
              textDocument: { uri, version: 1 },
              edits: [
                {
                  range: {
                    start: { line: 1, character: 21 },
                    end: { line: 1, character: 21 },
                  },
                  newText: "& ",
                },
                {
                  range: {
                    start: { line: 1, character: 29 },
                    end: { line: 1, character: 29 },
                  },
                  newText: "& ",
                },
              ],
            },
          ],
        },
      },
    ]);
    expect(state.protocolError).toBeUndefined();
  } finally {
    await stop();
  }
});

test("LSP offers the @variant fix for a header that a variant applies", async () => {
  const { root, state, send, wait, stop } = await startServer(".test-lsp-css-");
  try {
    await writeFile(
      path.join(root, "stillwind.config.ts"),
      'export default {projects:[{css:"root.css",files:["*.tsx"],lint:{rules:{"prefer-variant-directive":"warn"}}}]};',
    );
    const head = '@import "stillwind";\n';
    await writeFile(path.join(root, "root.css"), head);
    const uri = pathToFileURL(path.join(root, "root.css")).href;
    send(
      "initialize",
      {
        rootUri: pathToFileURL(root).href,
        capabilities: {},
      },
      1,
    );
    expect((await wait((message) => message.id === 1)).error, state.stderr).toBeUndefined();
    send("initialized", {});
    send("textDocument/didOpen", {
      textDocument: {
        uri,
        languageId: "css",
        version: 1,
        text: `${head}.card { @media (prefers-color-scheme: dark) { color: red; } }\n`,
      },
    });
    const published = await wait(
      (message) =>
        message.method === "textDocument/publishDiagnostics" &&
        message.params?.uri === uri &&
        message.params.version === 1 &&
        Boolean(message.params.diagnostics?.length),
    );
    const range = {
      start: {
        line: 1,
        character: 8,
      },
      end: {
        line: 1,
        character: 43,
      },
    };
    expect(published.params!.diagnostics).toMatchObject([
      {
        code: "prefer-variant-directive",
        range,
      },
    ]);
    send(
      "textDocument/codeAction",
      {
        textDocument: {
          uri,
        },
        range,
        context: {
          diagnostics: [],
          only: ["quickfix"],
        },
      },
      2,
    );
    const actions = await wait((message) => message.id === 2);
    expect(actions.result).toMatchObject([
      {
        title: "Write `@variant dark`",
        edit: {
          documentChanges: [
            {
              textDocument: {
                uri,
                version: 1,
              },
              edits: [
                {
                  range,
                  newText: "@variant dark",
                },
              ],
            },
          ],
        },
      },
    ]);
    expect(state.protocolError).toBeUndefined();
  } finally {
    await stop();
  }
});
