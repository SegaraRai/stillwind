import { lintPresets, type AnalysisProjectConfig } from "@stillwind/config";
import { NativeDesignSystemLoader, NativeProjectSession } from "@stillwind/native";
import { expect, test } from "vite-plus/test";

import { BrowserProject } from "../../src/browser/index.ts";
import { analyzeModule, fileModuleDescriptor } from "../../src/compilation/index.ts";
import { EditorSnapshot } from "../../src/editor/index.ts";
import { applyEdits } from "../../src/editor/sort.ts";
import { compilerOptions } from "../../src/project/index.ts";
import { project, theme } from "./support.ts";

test("captured editor inputs and reports survive mutation and later native compilation", async () => {
  const id = "/workspace/entry.tsx";
  const source = 'import {style} from "stillwind/react"; const card=style("p-[16px] flex block");';
  const config: AnalysisProjectConfig = {
    css: "theme.css",
    files: ["*.tsx"],
    typeFacts: false,
    sort: {
      stringLiterals: true,
    },
    lint: {
      extends: [lintPresets.recommended, lintPresets.stylistic],
    },
  };
  const loader = new NativeDesignSystemLoader(compilerOptions(config));
  expect(loader.provide(0, "theme.css", theme).kind).toBe("complete");
  const session = new NativeProjectSession(loader, false, "development");
  let text = source;
  const host = {
    describe: (file: string) => fileModuleDescriptor("/workspace", file),
    resolve: async () => null,
    read: async () => text,
    watch: () => {},
    warn: () => {},
  };
  const graph = await analyzeModule(session, () => undefined, id, host);
  const snapshot = new EditorSnapshot(graph, session, loader, config);
  const requests = [
    { kind: "hover", offset: source.indexOf("flex") },
    { kind: "fixes" },
    { kind: "sort" },
  ] as const;
  const expected = requests.map((request) => snapshot.query(id, request));
  expect(expected[0]?.hover?.css).toContain("display: flex");
  expect(expected[1]?.edits?.length).toBeGreaterThan(0);
  const reports = snapshot.reports();
  expect(reports.length).toBeGreaterThan(0);
  const expectedReports = structuredClone(reports);
  reports[0].actions[0].edits[0].replacement = "changed";
  reports[0].primary.location.uri = "untitled:changed";
  expect(graph.fixes.length).toBeGreaterThan(0);
  graph.fixes[0][0].replacement = "changed";
  graph.fixes.length = 0;
  const prepared = graph.sources[0];
  expect(prepared.kind).toBe("direct");
  if (prepared.kind === "direct") {
    prepared.syntax = { language: "js", moduleKind: "script" };
  }
  Object.assign(graph.sources[0].document, {
    code: "",
    uri: "untitled:changed",
  });
  graph.sources.length = 0;
  graph.dependencies.push({
    importer: id,
    specifier: "changed",
    status: "unresolved",
  });
  graph.batches.length = 0;
  config.sort = false;
  text = 'import {style} from "stillwind/react"; const card=style("block");';
  await analyzeModule(session, () => undefined, id, host);
  expect(requests.map((request) => snapshot.query(id, request))).toEqual(expected);
  expect(snapshot.reports()).toEqual(expectedReports);
});

test("hover previews add the configured comments using the project root font size", async () => {
  const source = 'import {style} from "stillwind/react"; const card = style("p-4 bg-bright");';
  const annotated = project(source, {
    rootFontSize: 20,
  });
  const plain = project(source, {
    preview: {
      themeValues: false,
      pixelEquivalents: false,
      colorEquivalents: false,
    },
  });
  const hover = async (current: BrowserProject, candidate: string) =>
    (await current.query("/workspace/entry.tsx", {
      kind: "hover",
      offset: source.indexOf(candidate),
    }))!.hover?.css;
  try {
    expect(await hover(annotated, "p-4")).toContain(
      "padding: calc(var(--spacing) * 4) /* 1rem = 20px */;",
    );
    expect(await hover(annotated, "bg-bright")).toContain(
      "background-color: var(--color-bright) /* red = #ff0000 */;",
    );
    expect(await hover(plain, "p-4")).not.toContain("/*");
  } finally {
    await annotated.close();
    await plain.close();
  }
});

test("hover uses emitted selectors, bound identities, and candidate variants; completions include both catalogs", async () => {
  const source =
    'import {style} from "stillwind/react"; const card = style("hover:bg-brand block"); const alias = card; console.log(alias); const other = "bg-brand";';
  const current = project(source);
  try {
    const hover = (await current.query("/workspace/entry.tsx", {
      kind: "hover",
      offset: source.indexOf("hover:bg-brand") + 8,
    }))!.hover;
    expect(hover?.css).toContain("&:hover");
    expect(hover?.css).toContain("background-color");
    expect(hover?.css).toMatch(/&:hover \{\n\s+background-color: [^\n]+;\n\s*\}/);
    const bound = (await current.query("/workspace/entry.tsx", {
      kind: "hover",
      offset: source.lastIndexOf("alias") + 1,
    }))!.hover;
    expect(bound?.css).toContain("&");
    expect(bound?.css).toMatch(/display:\s*block/);
    expect(bound?.css).toMatch(/\{\n\s+display: block;\n/);
    expect(
      (await current.query("/workspace/entry.tsx", {
        kind: "hover",
        offset: source.lastIndexOf("bg-brand") + 1,
      }))!.hover,
    ).toBeUndefined();
    const completions = (await current.query("/workspace/entry.tsx", {
      kind: "completion",
      offset: source.indexOf("block"),
    }))!.completions!;
    expect(completions.map((item) => item.label)).toEqual(
      expect.arrayContaining(["block", "hover:", "bg-brand"]),
    );
    const resolve = (await current.query("/workspace/entry.tsx", {
      kind: "resolve",
      candidate: "hover:bg-brand",
      input: completions[0].input,
      snapshot: completions[0].snapshot,
    }))!.hover;
    expect(resolve?.css).toContain("&:hover");
    expect(resolve?.css).toBe(hover?.css);
    const call = (await current.query("/workspace/entry.tsx", {
      kind: "hover",
      offset: source.indexOf('style("'),
    }))!.hover;
    expect(call?.css).toBe(bound?.css);
  } finally {
    await current.close();
  }
});

test("color queries display concrete colors and picker edits preserve variants", async () => {
  const source =
    'import {style} from "stillwind/react"; const card = style("hover:bg-brand bg-[currentColor]");';
  const current = project(source);
  try {
    const colors = (await current.query("/workspace/entry.tsx", {
      kind: "colors",
    }))!.colors!;
    expect(colors).toHaveLength(1);
    expect(colors[0].red).toBeCloseTo(0x12 / 255);
    const presentation = (await current.query("/workspace/entry.tsx", {
      kind: "colorPresentation",
      start: colors[0].start,
      end: colors[0].end,
      color: {
        red: 1,
        green: 0,
        blue: 0,
        alpha: 1,
      },
    }))!.edits!;
    expect(applyEdits(source, presentation)).toContain("hover:bg-[#ff0000]");
  } finally {
    await current.close();
  }
});

test("hover follows imported style aliases without treating ordinary variables as styles", async () => {
  const source =
    'import {style} from "stillwind/react"; import {base as imported} from "./base"; const derived=style([imported,"flex"]); console.log(imported);';
  const current = new BrowserProject({
    root: "/workspace",
    config: {
      css: "theme.css",
      files: ["*.tsx"],
      typeFacts: false,
    },
    files: new Map([
      ["/workspace/entry.tsx", source],
      [
        "/workspace/base.tsx",
        'import {style} from "stillwind/react"; export const base=style("block");',
      ],
      ["/workspace/theme.css", theme],
    ]),
  });
  try {
    const hover = (await current.query("/workspace/entry.tsx", {
      kind: "hover",
      offset: source.lastIndexOf("imported"),
    }))!.hover;
    expect(hover?.css).toMatch(/display:\s*block/);
  } finally {
    await current.close();
  }
});

test("color picker edits keep compound utility roots, importance, and arbitrary properties", async () => {
  const source =
    'import {style} from "stillwind/react"; const value=style("hover:!border-t-brand [color:#123456]");';
  const current = project(
    source,
    {},
    `${theme} @utility border-t-* { border-top-color:--value(--color-*, [color]); }`,
  );
  try {
    const colors = (await current.query("/workspace/entry.tsx", {
      kind: "colors",
    }))!.colors!;
    expect(colors).toHaveLength(2);
    const replacements: string[] = [];
    for (const color of colors) {
      const edits = (await current.query("/workspace/entry.tsx", {
        kind: "colorPresentation",
        start: color.start,
        end: color.end,
        color: {
          red: 1,
          green: 0,
          blue: 0,
          alpha: 1,
        },
      }))!.edits!;
      replacements.push(edits[0].newText);
    }
    expect(replacements).toEqual(["hover:!border-t-[#ff0000]", "[color:#ff0000]"]);
  } finally {
    await current.close();
  }
});
