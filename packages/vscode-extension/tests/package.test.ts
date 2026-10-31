import { spawn } from "node:child_process";
import { cp, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve, sep } from "node:path";
import { pathToFileURL } from "node:url";

import { expect, test } from "vite-plus/test";

import { extensionManifest } from "../src/manifest.ts";
import { cliServer } from "../src/project-cli.ts";

test("packaged extension contains its CSS custom data", async () => {
  for (const file of extensionManifest.contributes.css.customData) {
    const data = await readFile(resolve(import.meta.dirname, "../dist/desktop", file), "utf8");
    expect(JSON.parse(data).atDirectives).toContainEqual({
      name: "@utility",
    });
  }
});

test.each([
  { name: "an explicit configuration", explicit: true },
  { name: "a discovered configuration", explicit: false },
])(
  "packaged desktop server analyzes $name without repository dependencies",
  async ({ explicit }) => {
    const directory = await mkdtemp(join(tmpdir(), "stillwind-extension-"));
    if (!resolve(directory).startsWith(resolve(tmpdir()) + sep)) {
      throw new Error("Invalid test directory");
    }
    const runtime = join(directory, "runtime");
    const config = join(directory, explicit ? "custom.config.ts" : "stillwind.config.ts");
    const entry = join(directory, "entry.tsx");
    await cp(resolve(import.meta.dirname, "../dist/desktop/dist/node"), runtime, {
      recursive: true,
    });
    await writeFile(
      config,
      'export default {projects:[{css:"theme.css",files:["**/*.tsx"],typeFacts:false}]};',
    );
    await writeFile(
      join(directory, "theme.css"),
      '@import "stillwind"; @utility red { color:red; }',
    );
    const source = 'import {style} from "stillwind/react"; export const card = style("missing");';
    await writeFile(entry, source);
    const core = resolve(import.meta.dirname, "../../core");
    await cp(join(core, "dist"), join(directory, "node_modules/stillwind/dist"), {
      recursive: true,
    });
    await cp(join(core, "css"), join(directory, "node_modules/stillwind/css"), { recursive: true });
    await cp(join(core, "package.json"), join(directory, "node_modules/stillwind/package.json"));
    await cp(join(core, "index.css"), join(directory, "node_modules/stillwind/index.css"));
    const child = spawn(
      process.execPath,
      [
        join(runtime, "server.js"),
        ...(explicit ? ["--config", config] : []),
        "--stdio",
        `--clientProcessId=${process.pid}`,
      ],
      {
        cwd: directory,
        stdio: "pipe",
      },
    );
    const messages: {
      id?: number;
      error?: unknown;
      method?: string;
      params?: {
        uri?: string;
        diagnostics?: { code: string }[];
      };
    }[] = [];
    let buffer: Buffer = Buffer.alloc(0);
    let stderr = "";
    child.stderr.on("data", (data) => {
      stderr += String(data);
    });
    child.stdout.on("data", (chunk: Buffer) => {
      buffer = Buffer.concat([buffer, chunk]);
      while (true) {
        const end = buffer.indexOf("\r\n\r\n");
        if (end < 0) {
          return;
        }
        const length = Number(
          /Content-Length: (\d+)/i.exec(buffer.subarray(0, end).toString())?.[1],
        );
        if (buffer.length < end + 4 + length) {
          return;
        }
        messages.push(JSON.parse(buffer.subarray(end + 4, end + 4 + length).toString()));
        buffer = buffer.subarray(end + 4 + length);
      }
    });
    const send = (method: string, params: unknown, id?: number) => {
      const text = JSON.stringify({
        jsonrpc: "2.0",
        method,
        params,
        ...(id === undefined ? {} : { id }),
      });
      child.stdin.write(`Content-Length: ${Buffer.byteLength(text)}\r\n\r\n${text}`);
    };
    try {
      send(
        "initialize",
        {
          rootUri: pathToFileURL(directory + "/").href,
          capabilities: {},
        },
        1,
      );
      await expect
        .poll(
          () =>
            messages.find((message) => message.id === 1) ??
            (child.exitCode !== null ? stderr : undefined),
          { timeout: 30_000 },
        )
        .toMatchObject({ id: 1 });
      expect(messages.find((message) => message.id === 1)?.error).toBeUndefined();
      send("initialized", {});
      send("textDocument/didOpen", {
        textDocument: {
          uri: pathToFileURL(entry).href,
          languageId: "typescriptreact",
          version: 1,
          text: source,
        },
      });
      await expect
        .poll(() => messages.some((message) => message.params?.diagnostics?.length), {
          timeout: 30_000,
        })
        .toBe(true);
      const reports = messages.flatMap((message) => message.params?.diagnostics ?? []);
      expect(
        reports.map((report) => report.code),
        JSON.stringify(reports) + stderr,
      ).toContain("unknown-utility");
      send("shutdown", null, 2);
      await expect.poll(() => messages.some((message) => message.id === 2)).toBe(true);
      send("exit", null);
      await new Promise<void>((resolve) => child.once("close", resolve));
      expect(child.exitCode).toBe(0);
    } finally {
      if (child.exitCode === null) {
        const closed = new Promise<void>((resolve) => child.once("close", resolve));
        child.kill();
        await closed;
      }
      await rm(directory, { recursive: true, force: true });
    }
  },
);

test("CLI starts with the arguments and environment of a project server", async () => {
  const directory = await mkdtemp(join(tmpdir(), "stillwind-extension-cli-"));
  const config = join(directory, "stillwind.config.ts");
  await writeFile(config, "export default {projects:[]};");
  const { command, args, options } = cliServer(
    {
      module: resolve(import.meta.dirname, "../../cli/dist/main.js"),
      version: "",
    },
    config,
  );
  const child = spawn(command, args, {
    ...options,
    cwd: directory,
    stdio: "pipe",
  });
  const closed = new Promise<number | null>((resolve) => child.once("close", resolve));
  let stdout = "";
  let stderr = "";
  child.stdout.on("data", (data) => {
    stdout += String(data);
  });
  child.stderr.on("data", (data) => {
    stderr += String(data);
  });
  const text = JSON.stringify({
    jsonrpc: "2.0",
    id: 1,
    method: "initialize",
    params: {
      rootUri: pathToFileURL(directory).href,
      capabilities: {},
    },
  });
  child.stdin.write(`Content-Length: ${Buffer.byteLength(text)}\r\n\r\n${text}`);
  try {
    await expect
      .poll(() => (child.exitCode === null ? stdout : stderr), { timeout: 30_000 })
      .toContain("serverInfo");
  } finally {
    child.kill();
    await closed;
    await rm(directory, { recursive: true, force: true });
  }
});
