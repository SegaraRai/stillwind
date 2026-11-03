import { createContext, runInContext, runInNewContext } from "node:vm";

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
  const document = fakeDocument();
  document.head.children.push(
    fakeLink({
      href: "existing.css",
      dataset: {
        stillwindPart: "2:red",
        stillwindOrder: "2",
      },
    }),
  );
  const runnable = code.replace(
    "import.meta.url",
    JSON.stringify("https://example.test/app/assets/chunks/entry.js"),
  );

  runInNewContext(runnable, { document, URL });
  runInNewContext(runnable, { document, URL });

  expect(document.head.children.map((link) => link.dataset.stillwindPart)).toEqual([
    "0:red",
    "1:blue",
    "2:red",
  ]);
  expect(document.head.children.map((link) => link.href)).toEqual([
    "https://example.test/app/assets/red.css",
    "https://example.test/app/assets/blue.css",
    "existing.css",
  ]);
  expect(() => runInNewContext(runnable, { URL })).not.toThrow();
});

test("loaded split links stay in the head across Astro ClientRouter swaps", () => {
  const { project, context, chunk } = fixture("/app/");
  const code = renderChunkStyles(project, "client", context, "", chunk)!.code.replace(
    "import.meta.url",
    JSON.stringify("https://example.test/app/assets/chunks/entry.js"),
  );
  const document = fakeDocument();
  const sandbox = createContext({ document, URL });

  // Two chunks share one realm, so the listener must copy each link only once.
  runInContext(code, sandbox);
  runInContext(code, sandbox);
  const newDocument = fakeDocument();
  newDocument.head.children.push(fakeLink({ href: "/app/assets/ssr.css" }));
  document.dispatch("astro:before-swap", { newDocument });

  expect(newDocument.head.children.map((link) => link.href)).toEqual([
    "/app/assets/ssr.css",
    "/app/assets/red.css",
    "/app/assets/blue.css",
    "/app/assets/red.css",
  ]);
});

test.each(["/app/", "./", ""])("single emission loads client CSS once with base %j", (base) => {
  const { project, context, chunk } = fixture(base, {
    buildCss: "single-reference",
  });
  project.config.emission = "single";
  const result = renderChunkStyles(project, "client", context, "", chunk, true)!;
  const document = fakeDocument("https://example.test/app/");
  const runnable = result.code.replace(
    "import.meta.url",
    JSON.stringify("https://example.test/app/assets/chunks/entry.js"),
  );
  const sandbox = createContext({ document, URL });

  runInContext(runnable, sandbox);
  runInContext(runnable, sandbox);
  expect(document.head.children.map((link) => link.href)).toEqual([
    "https://example.test/app/assets/single.css",
  ]);

  const newDocument = fakeDocument();
  document.dispatch("astro:before-swap", { newDocument });
  expect(newDocument.head.children.map((link) => link.href)).toEqual([
    "https://example.test/app/assets/single.css",
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
    modules: {
      [registryId]: {},
    },
    facadeModuleId: "/facade.ts",
    fileName: "assets/chunks/entry.js",
    viteMetadata: {
      importedCss: new Set<string>(),
      importedAssets: new Set(["image.svg"]),
    },
  } as unknown as Rollup.RenderedChunk;
  return {
    project,
    context,
    chunk,
  };
}

/** Stylesheet link in {@link fakeDocument}. */
interface FakeLink {
  rel: string;
  href: string;
  dataset: Record<string, string>;
  getAttribute(name: string): string | null;
  cloneNode(): FakeLink;
}

/**
 * Creates a stylesheet link that supports the attribute reads of the generated loaders.
 * @param init - Initial `href` and `dataset`. The link is a stylesheet unless `rel` is given.
 * @returns A link whose `href` attribute equals its `href` property.
 */
function fakeLink(init: Partial<Pick<FakeLink, "rel" | "href" | "dataset">> = {}): FakeLink {
  const link: FakeLink = {
    rel: "stylesheet",
    href: "",
    dataset: {},
    ...init,
    getAttribute: (name) => (name === "href" ? link.href : null),
    cloneNode: () =>
      fakeLink({
        rel: link.rel,
        href: link.href,
        dataset: {
          ...link.dataset,
        },
      }),
  };
  return link;
}

/**
 * Creates a document with a head of links and the listener API that the generated loaders use.
 * The head supports only the `link[...]` selectors that the loaders query.
 * @param baseURI - Base URL for loaders that resolve public paths.
 * @returns The document, its head links in order, and `dispatch`, which calls listeners of a type.
 */
function fakeDocument(baseURI?: string) {
  const children: FakeLink[] = [];
  const matches = (link: FakeLink, selector: string) =>
    selector === 'link[rel="stylesheet"]'
      ? link.rel === "stylesheet"
      : selector === "link[data-stillwind-part]"
        ? "stillwindPart" in link.dataset
        : selector === "link[data-stillwind-loader]"
          ? "stillwindLoader" in link.dataset
          : expect.fail(`unexpected selector ${selector}`);
  const head = {
    children,
    querySelectorAll: (selector: string) => children.filter((link) => matches(link, selector)),
    append: (link: FakeLink) => {
      children.push(link);
    },
    insertBefore: (link: FakeLink, before: FakeLink | null) => {
      children.splice(before ? children.indexOf(before) : children.length, 0, link);
    },
  };
  const listeners = new Map<string, ((event: unknown) => void)[]>();
  return {
    baseURI,
    head,
    querySelectorAll: head.querySelectorAll,
    createElement: () => fakeLink({ rel: "" }),
    addEventListener: (type: string, listener: (event: unknown) => void) => {
      listeners.set(type, [...(listeners.get(type) ?? []), listener]);
    },
    dispatch: (type: string, event: unknown) => {
      for (const listener of listeners.get(type) ?? []) {
        listener(event);
      }
    },
  };
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
