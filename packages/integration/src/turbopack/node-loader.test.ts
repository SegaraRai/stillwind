import { mkdtemp, rm, writeFile } from "node:fs/promises";
import path from "node:path";

import { expect, onTestFinished, test, vi } from "vite-plus/test";
import type { LoaderContext } from "webpack";

import nodeLoader, { type NodeLoaderOptions } from "./node-loader.ts";

async function implementationFile() {
  const root = await mkdtemp(path.join(import.meta.dirname, ".test-node-loader-"));
  onTestFinished(() => rm(root, { recursive: true, force: true }));
  return path.join(root, "loader.mjs");
}

test("forwards the loader context, source, and upstream map", async () => {
  const implementation = await implementationFile();
  await writeFile(
    implementation,
    "export default function(source, map) { this.async()(null, this.resourcePath + source, map); }",
  );
  const callback = vi.fn();
  const context = {
    getOptions: () => ({
      implementation,
    }),
    async: () => callback,
    resourcePath: "/Component.tsx",
  } as unknown as LoaderContext<NodeLoaderOptions>;
  const map = { version: 3 as const, sources: ["input.tsx"], names: [], mappings: "" };
  nodeLoader.call(context, "source", map);
  await vi.waitFor(() => expect(callback).toHaveBeenCalledOnce());
  expect(callback).toHaveBeenCalledWith(null, "/Component.tsxsource", map);
});

test("completes the callback when the implementation cannot be imported", async () => {
  const implementation = await implementationFile();
  const callback = vi.fn();
  const context = {
    getOptions: () => ({
      implementation,
    }),
    async: () => callback,
  } as unknown as LoaderContext<NodeLoaderOptions>;
  nodeLoader.call(context, "source", undefined);
  await vi.waitFor(() => expect(callback).toHaveBeenCalledOnce());
  expect(callback).toHaveBeenCalledWith(expect.any(Error));
});
