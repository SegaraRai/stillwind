import type { AnalysisProjectConfig } from "@stillwind/config";
import { expect, test } from "vite-plus/test";

import { BrowserProject } from "../browser/index.ts";
import { applyEdits } from "./sort.ts";

const theme = `
@theme { --spacing: .25rem; --color-brand: #123456; --color-bright: red; }
@utility block { display: block; }
@utility flex { display: flex; }
@utility p-* { padding: --value(--spacing-*, --spacing(), [length]); }
@utility border-* { border-width: calc(--value(integer) * 1px); border-width: --value([length]); }
@utility bg-* { background-color: --value(--color-*, [color]); }
@variant hover { &:hover { @slot; } }
@variant focus { &:focus { @slot; } }
@stillwind-slot utilities;
`;

function project(source: string, config: Partial<AnalysisProjectConfig> = {}, css = theme) {
  return new BrowserProject({
    root: "/workspace",
    files: new Map([
      ["/workspace/entry.tsx", source],
      ["/workspace/theme.css", css],
    ]),
    config: { css: "theme.css", files: ["*.tsx"], types: false, typeFacts: false, ...config },
  });
}

test("canonical lint uses native policy, suppression, exact escaped ranges, and separate sorting actions", async () => {
  const source = String.raw`import {style} from "stillwind/react";
// 日本語 🦀
const card = style("p-[16px] flex block border-[2px]");
// swlint-disable-next-line canonical-utilities
const ignored = style("p-[16px]");
const escaped = style("p-\u005b16px\u005d");`;
  const current = project(source);
  try {
    const graph = await current.analyze("/workspace/entry.tsx");
    const reports = graph.batches.flatMap((batch) => batch.reports);
    expect(reports.filter((report) => report.code === "canonical-utilities")).toHaveLength(3);
    expect(reports.some((report) => report.code === "sort-utilities")).toBe(false);
    const fixes = (await current.query("/workspace/entry.tsx", { kind: "fixes" }))!.edits!;
    const fixed = applyEdits(source, fixes);
    expect(fixed).toContain('style("p-4 flex block border-2")');
    expect(fixed).toContain('const ignored = style("p-[16px]")');
    expect(fixed).toContain('const escaped = style("p-4")');
    const sorted = applyEdits(
      source,
      (await current.query("/workspace/entry.tsx", { kind: "sort" }))!.edits!,
    );
    expect(sorted).not.toBe(source);
    expect(sorted).toContain("p-[16px]");
    const next = project(fixed);
    try {
      expect((await next.query("/workspace/entry.tsx", { kind: "fixes" }))!.edits).toEqual([]);
    } finally {
      await next.close();
    }
  } finally {
    await current.close();
  }
});

test("CSS severity survives partial project overrides and explicit sorting ignores disabled lint", async () => {
  const source =
    'import {style} from "stillwind/react"; const card = style("p-[16px] flex block");';
  const current = project(
    source,
    { lint: { "canonical-utilities": { pxToRem: false }, "sort-utilities": "off" } },
    `@stillwind { canonical-utilities: deny; } ${theme}`,
  );
  try {
    const graph = await current.analyze("/workspace/entry.tsx");
    expect(graph.batches.flatMap((batch) => batch.reports)).toEqual([]);
    expect(
      (await current.query("/workspace/entry.tsx", { kind: "sort" }))!.edits?.length,
    ).toBeGreaterThan(0);
    expect((await current.query("/workspace/entry.tsx", { kind: "fixes" }))!.edits).toEqual([]);
  } finally {
    await current.close();
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
    const colors = (await current.query("/workspace/entry.tsx", { kind: "colors" }))!.colors!;
    expect(colors).toHaveLength(1);
    expect(colors[0].red).toBeCloseTo(0x12 / 255);
    const presentation = (await current.query("/workspace/entry.tsx", {
      kind: "colorPresentation",
      start: colors[0].start,
      end: colors[0].end,
      color: { red: 1, green: 0, blue: 0, alpha: 1 },
    }))!.edits!;
    expect(applyEdits(source, presentation)).toContain("hover:bg-[#ff0000]");
  } finally {
    await current.close();
  }
});

test("arrays keep dynamic boundaries, base order, and comments, while fix all composes nonoverlapping edits", async () => {
  const source = `import {style} from "stillwind/react";
const first = style("block"); const second = style("flex"); const marker = style.ref();
const value = style([
  "p-[16px]", // trailing comment
  first, second, marker,
  unknown,
  "flex", "block",
]);`;
  const current = project(source, { lint: { "sort-utilities": "warn" } });
  try {
    const edits = (await current.query("/workspace/entry.tsx", { kind: "fixes" }))!.edits!;
    expect(edits.every((edit, index) => index === 0 || edits[index - 1].end <= edit.start)).toBe(
      true,
    );
    const fixed = applyEdits(source, edits);
    expect(fixed).toContain("// trailing comment");
    const array = fixed.slice(fixed.indexOf("const value"));
    expect(array.indexOf("first")).toBeLessThan(array.indexOf("second"));
    expect(fixed).toMatch(/"block",\s*"flex"/);
  } finally {
    await current.close();
  }
});

test("incomplete literals and outer variants retain original completion and hover ranges", async () => {
  const source = 'import {style} from "stillwind/react"; const card=style({"hover:":"bg-br';
  const current = project(source);
  try {
    const items = (await current.query("/workspace/entry.tsx", {
      kind: "completion",
      offset: source.length,
    }))!.completions!;
    const item = items.find((item) => item.label === "bg-brand")!;
    expect(item).toMatchObject({
      candidate: "hover:bg-brand",
      start: source.indexOf("bg-br"),
      end: source.length,
      newText: "bg-brand",
    });
  } finally {
    await current.close();
  }
  const complete =
    'import {style} from "stillwind/react"; const card=style({"hover:":"bg-brand"});';
  const next = project(complete);
  try {
    expect(
      (await next.query("/workspace/entry.tsx", {
        kind: "hover",
        offset: complete.indexOf("bg-brand"),
      }))!.hover?.css,
    ).toContain("&:hover");
  } finally {
    await next.close();
  }
});

test("sort targets are independent and noninterfering base styles participate in sorting", async () => {
  const source =
    'import {style} from "stillwind/react"; const z=style("p-4"); const a=style("block"); const marker=style.ref(); const value=style([z,a,marker,"flex block"]);';
  const current = project(source, { sort: { stringLiterals: false } });
  try {
    const sorted = applyEdits(
      source,
      (await current.query("/workspace/entry.tsx", { kind: "sort" }))!.edits!,
    );
    expect(sorted).toContain('[marker,a,z,"flex block"]');
    const next = project(sorted, { sort: { stringLiterals: false } });
    try {
      expect((await next.query("/workspace/entry.tsx", { kind: "sort" }))!.edits).toEqual([]);
    } finally {
      await next.close();
    }
  } finally {
    await current.close();
  }
  const strings = project(source, { sort: { arrays: false } });
  try {
    const sorted = applyEdits(
      source,
      (await strings.query("/workspace/entry.tsx", { kind: "sort" }))!.edits!,
    );
    expect(sorted).toContain('[z,a,marker,"block flex"]');
  } finally {
    await strings.close();
  }
});

test("array sorting retains comments attached after an expression", async () => {
  const source =
    'import {style} from "stillwind/react"; const value=style(["flex" /* attached */, "block", "p-4"]);';
  const current = project(source);
  try {
    const sorted = applyEdits(
      source,
      (await current.query("/workspace/entry.tsx", { kind: "sort" }))!.edits!,
    );
    expect(sorted).toContain('"flex" /* attached */');
    expect(sorted.match(/attached/g)).toHaveLength(1);
  } finally {
    await current.close();
  }
});

test("hover follows imported style aliases without treating ordinary variables as styles", async () => {
  const source =
    'import {style} from "stillwind/react"; import {base as imported} from "./base"; const derived=style([imported,"flex"]); console.log(imported);';
  const current = new BrowserProject({
    root: "/workspace",
    config: { css: "theme.css", files: ["*.tsx"], types: false, typeFacts: false },
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

test("namespace imports complete unfinished style calls", async () => {
  const source = 'import * as sw from "stillwind/react"; const value=sw.style("bg-br';
  const current = project(source);
  try {
    expect(
      (await current.query("/workspace/entry.tsx", {
        kind: "completion",
        offset: source.length,
      }))!.completions?.map((item) => item.label),
    ).toContain("bg-brand");
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
    const colors = (await current.query("/workspace/entry.tsx", { kind: "colors" }))!.colors!;
    expect(colors).toHaveLength(2);
    const replacements: string[] = [];
    for (const color of colors) {
      const edits = (await current.query("/workspace/entry.tsx", {
        kind: "colorPresentation",
        start: color.start,
        end: color.end,
        color: { red: 1, green: 0, blue: 0, alpha: 1 },
      }))!.edits!;
      replacements.push(edits[0].newText);
    }
    expect(replacements).toEqual(["hover:!border-t-[#ff0000]", "[color:#ff0000]"]);
  } finally {
    await current.close();
  }
});
