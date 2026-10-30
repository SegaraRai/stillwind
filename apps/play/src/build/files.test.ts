import { expect, test, vi } from "vite-plus/test";

import { BuildFiles } from "./files.ts";

test("runtime imports resolve before their bodies are read and reads are shared within a build", async () => {
  const manifest = '{"exports":{"browser":"./data.json","node":"./node.js"}}';
  const read = vi.fn(async (file: string) => (file.endsWith(".json") ? '{"value":1}' : ""));
  const files = new BuildFiles(
    [
      ["/workspace/node_modules/demo/package.json", manifest],
      ["/workspace/node_modules/demo/data.json", null],
      ["/workspace/empty.d.ts", null],
      ["/workspace/unused.js", null],
    ],
    read,
  );
  const target = files.resolve("demo", "/workspace/App.tsx");
  expect(target).toBe("/workspace/node_modules/demo/data.json");
  expect(read).not.toHaveBeenCalled();
  expect(await files.read("/workspace/node_modules/demo/package.json")).toBe(manifest);
  await expect(Promise.all([files.read(target!), files.read(target!)])).resolves.toEqual([
    '{"value":1}',
    '{"value":1}',
  ]);
  expect(await files.read("/workspace/empty.d.ts")).toBe("");
  expect(read.mock.calls.map(([file]) => file)).toEqual([target, "/workspace/empty.d.ts"]);
  await expect(files.read("/workspace/missing.js")).rejects.toThrow("Missing virtual file");
});

test("new builds read edited contents and propagate obsolete revision errors", async () => {
  const entries = [["/workspace/App.tsx", null]] as const;
  const previous = new BuildFiles(entries, async () => "previous");
  const current = new BuildFiles(entries, async () => "current");
  expect(await previous.read(entries[0][0])).toBe("previous");
  expect(await current.read(entries[0][0])).toBe("current");
  const obsolete = new BuildFiles(entries, async () => {
    throw new Error("Obsolete workspace revision");
  });
  await expect(obsolete.read(entries[0][0])).rejects.toThrow("Obsolete workspace revision");
});
