import type { AnalysisProjectConfig } from "@stillwind/config";
import { prepareSource, type PreparedSource } from "@stillwind/projection";
import { expect, test } from "vite-plus/test";

import { fileModuleDescriptor } from "../compilation/index.ts";
import { BrowserProject } from "./project.ts";

test("changing package style exports reloads retained CSS", async () => {
  const entry = "/workspace/entry.ts";
  const manifest = "/workspace/node_modules/palette/package.json";
  const files = new Map([
    [entry, 'import {style} from "stillwind/react"; export const value=style("tone");'],
    ["/workspace/theme.css", '@import "palette"; @stillwind-slot utilities;'],
    [manifest, JSON.stringify({ exports: { style: "./first.css" } })],
    ["/workspace/node_modules/palette/first.css", "@utility tone{color:red}"],
    ["/workspace/node_modules/palette/second.css", "@utility tone{color:blue}"],
  ]);
  const project = new BrowserProject({
    root: "/workspace",
    config: {
      css: "theme.css",
      files: ["*.ts"],
      typeFacts: false,
    },
    files,
  });
  try {
    await project.analyze(entry);
    expect(project.css().css).toContain("color:red");
    files.set(
      manifest,
      JSON.stringify({
        exports: {
          style: "./second.css",
        },
      }),
    );
    await project.replaceFiles(files);
    await project.analyze(entry);
    expect(project.css().css).toContain("color:blue");
  } finally {
    await project.close();
  }
});

test("browser projects capture custom source and projection operations and their results", async () => {
  const root = "/workspace";
  const entry = `${root}/entry.tsx`;
  let prepared: PreparedSource | undefined;
  const config: AnalysisProjectConfig = {
    css: "theme.css",
    files: ["*.tsx"],
    typeFacts: false,
    sources: {
      kind: "custom",
      supports: () => true,
      load: async (id, context) => {
        const descriptor = fileModuleDescriptor(root, id);
        return {
          resolverId: id,
          ...descriptor,
          document: {
            ...descriptor.document,
            code: context.documents.get(id)!,
          },
        };
      },
    },
    projection: {
      kind: "custom",
      prepare: async (input) => {
        prepared = await prepareSource(input);
        return prepared;
      },
    },
  };
  const project = new BrowserProject({
    root,
    config,
    files: new Map([
      [entry, 'import {style} from "stillwind/react"; export const card=style("flex");'],
      [`${root}/theme.css`, "@utility flex { display:flex; } @stillwind-slot utilities;"],
    ]),
  });
  if (config.sources?.kind === "custom") {
    config.sources.supports = () => false;
    config.sources.load = async () => expect.fail("Source operation was replaced");
  }
  if (config.projection?.kind === "custom") {
    config.projection.prepare = async () => expect.fail("Projection operation was replaced");
  }
  try {
    const graph = await project.analyze(entry);
    expect(graph.generation).toBe("complete");
    const original = graph.sources[0].document.code;
    expect(prepared).toBeDefined();
    Object.assign(prepared!.document, { code: "changed" });
    expect((await project.analyze(entry)).sources[0].document.code).toBe(original);
    expect(project.css().css).toContain("display:flex");
  } finally {
    await project.close();
  }
});

test("browser document and carrier outputs expose naming fallbacks", async () => {
  const project = new BrowserProject({
    root: "/workspace",
    config: {
      css: "theme.css",
      files: ["*.ts"],
      typeFacts: false,
      naming: {
        layerName: () => {
          throw new Error("layer rejected");
        },
      },
    },
    files: new Map([
      [
        "/workspace/entry.ts",
        'import {style} from "stillwind/react"; export const card=style("flex");',
      ],
      [
        "/workspace/theme.css",
        "@utility flex { display:flex; } @layer { .authored { color:red; } @stillwind-slot utilities; }",
      ],
    ]),
  });
  try {
    expect(project.css()).toEqual({ css: "", rejectedNames: [] });
    await project.analyze("/workspace/entry.ts");
    const output = project.css();
    expect(output.rejectedNames).toHaveLength(1);
    expect(output.rejectedNames[0]).toMatchObject({
      reason: expect.stringContaining("layer rejected"),
      substituted: true,
    });
    const value = project.cssValues().get("/workspace/entry.ts")!;
    expect(value.rejectedNames).toEqual(output.rejectedNames);
    expect(value.css).toContain("display:flex");
    expect(project.css().rejectedNames).toEqual(output.rejectedNames);
  } finally {
    await project.close();
  }
});

test("projects capture caller inputs and share cached analyses independently of editable copies", async () => {
  const source = 'import {style} from "stillwind/react"; const card=style("flex block");';
  const options = {
    root: "/workspace",
    config: {
      css: "theme.css",
      files: ["*.ts"],
      typeFacts: false as const,
      sort: {
        stringLiterals: true,
      },
    },
    files: new Map([
      ["/workspace/entry.ts", source],
      [
        "/workspace/theme.css",
        "@utility block { display:block; } @utility flex { display:flex; } @stillwind-slot utilities;",
      ],
    ]),
  };
  const project = new BrowserProject(options);
  options.config.css = "missing.css";
  options.config.sort.stringLiterals = false;
  options.files.clear();
  try {
    const graph = await project.analyze("/workspace/entry.ts");
    const editable = structuredClone(graph);
    const expected = structuredClone(graph);
    const sorted = await project.query("/workspace/entry.ts", {
      kind: "sort",
    });
    expect(sorted?.edits?.length).toBeGreaterThan(0);
    const css = project.css().css;
    Object.assign(editable.sources, { length: 0 });
    Object.assign(editable.modules, { length: 0 });
    Object.assign(editable.batches, { length: 0 });
    expect(await project.analyze("/workspace/entry.ts")).toBe(graph);
    expect(graph).toEqual(expected);
    expect(
      await project.query("/workspace/entry.ts", {
        kind: "sort",
      }),
    ).toEqual(sorted);
    expect(project.css().css).toEqual(css);
  } finally {
    await project.close();
  }
});

test("failed graphs invalidate cached dependencies and keep unrelated CSS", async () => {
  const project = new BrowserProject({
    root: "/workspace",
    config: { css: "theme.css", files: ["*.ts"], typeFacts: false },
    files: new Map([
      [
        "/workspace/styles.ts",
        'import {style} from "stillwind/react"; export const card = style("block");',
      ],
      [
        "/workspace/other.ts",
        'import {style} from "stillwind/react"; export const other = style("flex");',
      ],
      [
        "/workspace/entry.ts",
        'import {style} from "stillwind/react"; import {card} from "./styles"; export const value = style(card, style("block"));',
      ],
      [
        "/workspace/theme.css",
        "@utility block { display:block; } @utility flex { display:flex; } @stillwind-slot utilities;",
      ],
    ]),
  });
  try {
    await project.analyze("/workspace/styles.ts");
    await project.analyze("/workspace/other.ts");
    expect((await project.analyze("/workspace/entry.ts")).generation).toBe("blocked");
    expect(project.css().css).toContain("display:flex");
    expect(project.css().css).not.toContain("display:block");
    expect([...project.cssValues().keys()]).toEqual(["/workspace/other.ts"]);
    await project.analyze("/workspace/styles.ts");
    expect(project.css().css).toContain("display:block");
  } finally {
    await project.close();
  }
});

test("browser projects deliver explicit CSS values and resolve captured SVG resources", async () => {
  const files = new Map([
    [
      "/workspace/entry.ts",
      'import {style} from "stillwind/react"; import css from "stillwind/module-css"; export const card = style("red"); export {css};',
    ],
    [
      "/workspace/theme.css",
      '@utility red { color:red; background-image:url("./icon.svg"); } @stillwind-slot utilities;',
    ],
    ["/workspace/icon.svg", '<svg xmlns="http://www.w3.org/2000/svg"/>'],
  ]);
  const project = new BrowserProject({
    root: "/workspace",
    files,
    config: {
      css: "theme.css",
      files: ["**/*.ts"],
      automatic: false,
      typeFacts: false,
    },
  });
  try {
    const graph = await project.analyze("/workspace/entry.ts");
    expect(graph.generation).toBe("complete");
    expect(project.css().css).not.toContain("color:red");
    const value = project.cssValues().get("/workspace/entry.ts");
    expect(value?.css).toContain("color:red");
    expect(value?.css).toContain("data:image/svg+xml");
    expect(value?.parts.map((part) => part.css).join("")).toBe(value?.css);
  } finally {
    await project.close();
  }
});
