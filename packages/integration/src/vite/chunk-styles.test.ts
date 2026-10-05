import { runInNewContext } from "node:vm";

import type { Rollup } from "vite";
import { expect, test } from "vite-plus/test";

import type { BuildOutput, StylesheetManifest } from "./build.ts";
import { renderChunkStyles } from "./chunk-styles.ts";
import { producerPrefix } from "./module-ids.ts";
import type { ViteProject } from "./project.ts";

test.each(["/app/", "./", ""])(
  "split chunks select static dependencies and preserve repeated parts with base %j",
  (base) => {
    const { project, context, chunk } = fixture(base);
    const result = renderChunkStyles(project, "client", context, "entry();", chunk);

    expect(result?.map).toBeNull();
    expect(result?.code.startsWith("entry();\n\n/* Stillwind CSS Loader */")).toBe(true);
    const parts = loaderParts(result!.code);
    expect(parts.map((part) => part.id)).toEqual(["0:red", "1:blue", "2:red"]);
    expect(parts.map((part) => part.order)).toEqual([0, 1, 2]);
    expect(parts.map((part) => part.href)).toEqual(
      base === "/app/"
        ? ["/app/assets/red.css", "/app/assets/blue.css", "/app/assets/red.css"]
        : ["../red.css", "../blue.css", "../red.css"],
    );
    expect(parts.every((part) => part.relative === (base !== "/app/"))).toBe(true);
    expect(chunk.viteMetadata?.importedCss).toEqual(new Set(["assets/red.css", "assets/blue.css"]));
    expect(chunk.viteMetadata?.importedAssets).toEqual(new Set(["image.svg"]));
  },
);

test("the generated loader inserts missing links in manifest order and keeps existing occurrences", () => {
  const { project, context, chunk } = fixture("./");
  const code = renderChunkStyles(project, "client", context, "", chunk)!.code;
  const links = [
    {
      dataset: {
        stillwindPart: "2:red",
        stillwindOrder: "2",
      },
      href: "existing.css",
    },
  ];
  const document = {
    querySelectorAll: () => links,
    createElement: () => ({ dataset: {}, href: "" }),
    head: {
      insertBefore(link: (typeof links)[number], before: (typeof links)[number] | null) {
        links.splice(before ? links.indexOf(before) : links.length, 0, link);
      },
    },
  };
  const runnable = code.replace(
    "import.meta.url",
    JSON.stringify("https://example.test/app/assets/chunks/entry.js"),
  );

  runInNewContext(runnable, { document, URL });
  runInNewContext(runnable, { document, URL });

  expect(links.map((link) => link.dataset.stillwindPart)).toEqual(["0:red", "1:blue", "2:red"]);
  expect(links.map((link) => link.href)).toEqual([
    "https://example.test/app/assets/red.css",
    "https://example.test/app/assets/blue.css",
    "existing.css",
  ]);
  expect(() => runInNewContext(runnable, { URL })).not.toThrow();
});

test("single emission attaches its CSS asset without adding a loader", () => {
  const { project, context, chunk } = fixture("/app/", { buildCss: "single-reference" });
  project.options.emission = "single";

  expect(renderChunkStyles(project, "client", context, "entry();", chunk)).toBeNull();
  expect(chunk.viteMetadata?.importedCss).toEqual(new Set(["assets/single.css"]));
});

test("a chunk without selected split styles keeps its code and metadata", () => {
  const { project, context, chunk } = fixture("/app/", {
    manifest: { version: 1, revision: "revision", parts: [], modules: {} },
  });

  expect(renderChunkStyles(project, "client", context, "entry();", chunk)).toBeNull();
  expect(chunk.viteMetadata?.importedCss).toEqual(new Set());
});

test("a chunk rendered before build output exists needs no CSS metadata", () => {
  const { project, context, chunk } = fixture("/app/");
  project.buildOutput = () => ({});
  delete chunk.viteMetadata;

  expect(renderChunkStyles(project, "client", context, "entry();", chunk)).toBeNull();
  expect(chunk.viteMetadata).toBeUndefined();
});

function fixture(base: string, output?: BuildOutput) {
  const manifest: StylesheetManifest = {
    version: 1,
    revision: "revision",
    parts: [
      { id: "0:red", href: `${base}assets/red.css` },
      { id: "1:blue", href: `${base}assets/blue.css` },
      { id: "2:red", href: `${base}assets/red.css` },
      { id: "3:lazy", href: `${base}assets/lazy.css` },
    ],
    modules: {
      main: ["0:red", "2:red"],
      dependency: ["1:blue"],
      lazy: ["3:lazy"],
    },
  };
  const registryId = `${producerPrefix}cached`;
  const project = {
    options: { emission: "split" },
    settings: { publicBase: base },
    buildOutput: (name: string) => {
      expect(name).toBe("client");
      return output ?? { manifest };
    },
    inputs: () => new Map([["/main.ts", { moduleId: "main" }]]),
    moduleRegistry: () => new Map([[registryId, { kind: "producer", moduleId: "main" }]]),
  } as unknown as ViteProject;
  const infos = new Map([
    [
      "/facade.ts",
      {
        importedIds: ["/main.ts"],
        meta: {},
      },
    ],
    [
      "/main.ts",
      {
        importedIds: ["/dependency.ts"],
        dynamicallyImportedIds: ["/lazy.ts"],
        meta: {},
      },
    ],
    [
      "/dependency.ts",
      {
        importedIds: ["/facade.ts"],
        meta: {
          stillwind: {
            moduleId: "dependency",
          },
        },
      },
    ],
  ]);
  const context = {
    getModuleInfo: (id: string) => infos.get(id) ?? null,
    getFileName: () => "assets/single.css",
  } as unknown as Parameters<typeof renderChunkStyles>[2];
  const chunk = {
    modules: { [registryId]: {} },
    facadeModuleId: "/facade.ts",
    fileName: "assets/chunks/entry.js",
    viteMetadata: { importedCss: new Set<string>(), importedAssets: new Set(["image.svg"]) },
  } as unknown as Rollup.RenderedChunk;
  return { project, context, chunk };
}

function loaderParts(
  code: string,
): { id: string; order: number; href: string; relative: boolean }[] {
  return JSON.parse(code.match(/for \(const p of (\[.*\])\) \{/)![1]);
}
