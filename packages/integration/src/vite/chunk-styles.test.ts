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
      /**
       * Inserts a stylesheet occurrence into the simulated document order.
       * @param link - New link whose occurrence metadata is inspected by the generated loader.
       * @param before - Existing link to precede, or null to append.
       */
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

test.each(["/app/", "./", ""])("single emission loads client CSS once with base %j", (base) => {
  const { project, context, chunk } = fixture(base, {
    buildCss: "single-reference",
  });
  project.config.emission = "single";
  const result = renderChunkStyles(project, "client", context, "", chunk, true)!;
  const links: {
    href: string;
  }[] = [];
  const document = {
    baseURI: "https://example.test/app/",
    querySelectorAll: () => links,
    createElement: () => ({
      href: "",
    }),
    head: {
      append: (link: { href: string }) => {
        links.push(link);
      },
    },
  };
  const runnable = result.code.replace(
    "import.meta.url",
    JSON.stringify("https://example.test/app/assets/chunks/entry.js"),
  );

  runInNewContext(runnable, {
    document,
    URL,
  });
  runInNewContext(runnable, {
    document,
    URL,
  });
  expect(links).toEqual([
    {
      rel: "stylesheet",
      href: "https://example.test/app/assets/single.css",
    },
  ]);
  expect(() => runInNewContext(runnable, { URL })).not.toThrow();
  expect(chunk.viteMetadata?.importedCss).toEqual(new Set(["assets/single.css"]));
});

test("single emission associates CSS with hosts that deliver chunk metadata", () => {
  const { project, context, chunk } = fixture("/app/", {
    buildCss: "single-reference",
  });
  project.config.emission = "single";
  expect(renderChunkStyles(project, "client", context, "entry();", chunk)).toBeNull();
  expect(chunk.viteMetadata?.importedCss).toEqual(new Set(["assets/single.css"]));
});

test("a chunk without selected split styles keeps its code and metadata", () => {
  const { project, context, chunk } = fixture("/app/", {
    manifest: {
      version: 1,
      revision: "revision",
      parts: [],
      modules: {},
    },
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

/**
 * Creates chunk metadata with cyclic static dependencies and a separate lazy stylesheet.
 * @param base - Vite public base used in manifest URLs and loader URL classification.
 * @param output - Optional build output replacing the default split manifest.
 * @returns Project, graph lookup context, and a rendered entry chunk with existing asset metadata.
 */
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
    config: {
      emission: "split",
    },
    settings: {
      publicBase: base,
    },
    buildOutput: (name: string) => {
      expect(name).toBe("client");
      return (
        output ?? {
          manifest,
        }
      );
    },
    inputs: () =>
      new Map([
        [
          "/main.ts",
          {
            moduleId: "main",
          },
        ],
      ]),
    moduleRegistry: () =>
      new Map([
        [
          registryId,
          {
            kind: "producer",
            moduleId: "main",
          },
        ],
      ]),
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
    viteMetadata: {
      importedCss: new Set<string>(),
      importedAssets: new Set(["image.svg"]),
    },
  } as unknown as Rollup.RenderedChunk;
  return { project, context, chunk };
}

/**
 * Reads the serialized stylesheet occurrences embedded in a generated chunk loader.
 * @param code - Generated chunk JavaScript containing the expected for-of loader loop.
 * @returns Occurrence descriptors in the order emitted by the chunk renderer.
 * @throws If the expected loop is missing or its array is invalid JSON.
 */
function loaderParts(code: string): {
  /** Manifest occurrence identity, distinct for repeated stylesheet content. */
  id: string;
  /** Zero-based manifest position used to insert links in cascade order. */
  order: number;
  /** Public or chunk-relative stylesheet URL. */
  href: string;
  /** Whether href must be resolved against import.meta.url. */
  relative: boolean;
}[] {
  return JSON.parse(code.match(/for \(const p of (\[.*\])\) \{/)![1]);
}
