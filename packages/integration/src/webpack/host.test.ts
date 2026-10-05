import path from "node:path";

import { expect, test, vi } from "vite-plus/test";

import type { ProducerInput } from "./emission.ts";
import { createCompilationHost, createLoaderHost, type ResolverHost } from "./host.ts";

function loaderContext() {
  return {
    fs: {
      readFile: vi.fn<ResolverHost["fs"]["readFile"]>(),
    },
    getResolve: vi.fn<ResolverHost["getResolve"]>(
      () => (_directory, specifier, callback) => callback(null, specifier),
    ),
    loaders: [
      { request: "post-loader" },
      { request: "stillwind-loader" },
      { request: "framework-loader" },
    ],
    loaderIndex: 1,
    importModule: vi.fn<(request: string) => Promise<unknown>>(),
    addDependency: vi.fn<(id: string) => void>(),
    emitWarning: vi.fn<(error: Error) => void>(),
  };
}

test("captures each query through preceding loaders and retains its upstream map", async () => {
  const context = loaderContext();
  const first = path.resolve("dependency.ts") + "?first";
  const second = path.resolve("dependency.ts") + "?second";
  const map = { version: 3, sources: ["dependency.ts"], mappings: "AAAA" };
  context.importModule.mockImplementation(async (request) => ({
    source: request.endsWith(first) ? "first source" : "second source",
    map,
  }));
  const host = createLoaderHost(context, path.resolve("loader.js"));
  expect(await Promise.all([host.readDependency(first), host.readDependency(first)])).toEqual([
    "first source",
    "first source",
  ]);
  expect(await host.readDependency(second)).toBe("second source");
  expect(context.importModule.mock.calls.map(([request]) => request.split("!").slice(-2))).toEqual([
    ["framework-loader", first],
    ["framework-loader", second],
  ]);
  expect(await host.dependencySources.get(first)).toEqual({ source: "first source", map });
  expect(context.fs.readFile).not.toHaveBeenCalled();
});

test("reconstruction uses captured query sources and rejects uncaptured derivatives", async () => {
  const context = loaderContext();
  const file = path.resolve("dependency.ts");
  context.fs.readFile.mockImplementation((_id, callback) => callback(null, "file source"));
  const input: ProducerInput = {
    id: path.resolve("module.ts") + "?generated",
    source: "producer source",
    root: path.resolve("."),
    options: { css: "./root.css" },
    manual: false,
    dependencies: [{ id: file + "?first", source: "captured source" }],
  };
  const host = createCompilationHost(context);
  const read = host.dependencyReader([input]);
  expect(await read(input.id)).toBe(input.source);
  expect(await read(file + "?first")).toBe("captured source");
  await expect(read(file + "?missing")).rejects.toThrow("No transformed source captured");
  expect(context.fs.readFile).not.toHaveBeenCalled();
  expect(await read(file)).toBe("file source");
  expect(context.fs.readFile).toHaveBeenCalledWith(file, expect.any(Function));
});

test("loader resolution handles builtins and keeps its extension policy separate from reconstruction", async () => {
  const context = loaderContext();
  const importer = path.resolve("module.ts");
  const host = createLoaderHost(context, path.resolve("loader.js"));
  expect(context.getResolve.mock.calls[1][0].extensions).toContain(".json");
  expect(await host.resolve("node:fs", importer)).toEqual({ id: "node:fs", external: true });
  expect(await host.resolve("./dependency.json", importer)).toEqual({ id: "./dependency.json" });
  const reconstruction = loaderContext();
  createCompilationHost(reconstruction);
  const settings = reconstruction.getResolve.mock.calls.find(
    ([settings]) => settings.dependencyType === "esm",
  )?.[0];
  expect(settings?.extensions).toEqual(expect.arrayContaining([".vue", ".svelte", ".astro"]));
  expect(settings?.extensions).not.toContain(".json");
});
