import { lintPresets, type AnalysisProjectConfig } from "@stillwind/config";
import { NativeDesignSystemLoader, NativeProjectSession } from "@stillwind/native";
import { expect, test } from "vite-plus/test";

import { BrowserProject } from "../browser/index.ts";
import { analyzeModule, fileModuleDescriptor } from "../compilation/index.ts";
import { compilerOptions } from "../project/options.ts";
import { EditorSnapshot } from "./snapshot.ts";
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

/**
 * Creates an isolated virtual project for queries that do not require semantic type facts.
 *
 * @param source - Original TSX text stored at `/workspace/entry.tsx`.
 * @param config - Overrides for the default entry, CSS path, and disabled type facts.
 * @param css - Complete stylesheet stored at `/workspace/theme.css`.
 * @returns A project whose lifecycle the caller must end with `close()`.
 */
function project(source: string, config: Partial<AnalysisProjectConfig> = {}, css = theme) {
  return new BrowserProject({
    root: "/workspace",
    files: new Map([
      ["/workspace/entry.tsx", source],
      ["/workspace/theme.css", css],
    ]),
    config: {
      css: "theme.css",
      files: ["*.tsx"],
      typeFacts: false,
      ...config,
    },
  });
}

test("lint false disables reports and fixes while explicit sorting remains available", async () => {
  const source = 'import {style} from "stillwind/react"; const value=style("p-[16px] flex block");';
  const current = project(source, {
    lint: false,
  });
  try {
    const graph = await current.analyze("/workspace/entry.tsx");
    expect(graph.batches.flatMap((batch) => batch.reports)).toEqual([]);
    expect(
      (await current.query("/workspace/entry.tsx", {
        kind: "fixes",
      }))!.edits,
    ).toEqual([]);
    const sorted = applyEdits(
      source,
      (await current.query("/workspace/entry.tsx", {
        kind: "sort",
      }))!.edits!,
    );
    expect(sorted).not.toBe(source);
    expect(sorted).toContain("p-[16px]");
  } finally {
    await current.close();
  }
});

test("sort false disables explicit and lint sorting while preserving other lint fixes", async () => {
  const source =
    'import {style} from "stillwind/react"; const base=style("p-4"); const ref=style.ref(); const value=style([base,ref,"p-[16px] flex block",{"data-z":1,"aria-label":"x"}]);';
  const current = project(source, {
    sort: false,
    lint: {
      extends: [lintPresets.recommended, lintPresets.stylistic],
    },
  });
  try {
    const graph = await current.analyze("/workspace/entry.tsx");
    expect(
      graph.batches
        .flatMap((batch) => batch.reports)
        .some((report) => report.code === "prefer-sorted-utilities"),
    ).toBe(false);
    expect(
      (await current.query("/workspace/entry.tsx", {
        kind: "sort",
      }))!.edits,
    ).toEqual([]);
    const fixed = applyEdits(
      source,
      (await current.query("/workspace/entry.tsx", {
        kind: "fixes",
      }))!.edits!,
    );
    expect(fixed).toContain('style([base,ref,"p-4 flex block",{"data-z":1,"aria-label":"x"}])');
  } finally {
    await current.close();
  }
});

test("canonical lint uses native policy, suppression, exact escaped ranges, and separate sorting actions", async () => {
  const source = String.raw`import {style} from "stillwind/react";
// 日本語 🦀
const card = style("p-[16px] flex block border-[2px]");
// swlint-disable-next-line prefer-canonical-utilities
const ignored = style("p-[16px]");
const escaped = style("p-\u005b16px\u005d");`;
  const current = project(source);
  try {
    const graph = await current.analyze("/workspace/entry.tsx");
    const reports = graph.batches.flatMap((batch) => batch.reports);
    expect(reports.filter((report) => report.code === "prefer-canonical-utilities")).toHaveLength(
      3,
    );
    expect(reports.some((report) => report.code === "prefer-sorted-utilities")).toBe(false);
    const fixes = (await current.query("/workspace/entry.tsx", {
      kind: "fixes",
    }))!.edits!;
    const fixed = applyEdits(source, fixes);
    expect(fixed).toContain('style("p-4 flex block border-2")');
    expect(fixed).toContain('const ignored = style("p-[16px]")');
    expect(fixed).toContain('const escaped = style("p-4")');
    const sorted = applyEdits(
      source,
      (await current.query("/workspace/entry.tsx", {
        kind: "sort",
      }))!.edits!,
    );
    expect(sorted).not.toBe(source);
    expect(sorted).toContain("p-[16px]");
    const next = project(fixed);
    try {
      expect(
        (await next.query("/workspace/entry.tsx", {
          kind: "fixes",
        }))!.edits,
      ).toEqual([]);
    } finally {
      await next.close();
    }
  } finally {
    await current.close();
  }
});

test("project severity supports explicit conversion options and explicit sorting ignores disabled lint", async () => {
  const source =
    'import {style} from "stillwind/react"; const card = style("p-[16px] flex block");';
  const current = project(
    source,
    {
      lint: {
        extends: [lintPresets.recommended],
        rules: {
          "prefer-canonical-utilities": {
            pxToRem: false,
            severity: "deny",
          },
          "prefer-sorted-utilities": "off",
        },
      },
    },
    theme,
  );
  try {
    const graph = await current.analyze("/workspace/entry.tsx");
    expect(graph.batches.flatMap((batch) => batch.reports).map((report) => report.code)).toEqual([
      "conflicting-utilities",
    ]);
    expect(
      (await current.query("/workspace/entry.tsx", {
        kind: "sort",
      }))!.edits?.length,
    ).toBeGreaterThan(0);
    expect(
      (await current.query("/workspace/entry.tsx", {
        kind: "fixes",
      }))!.edits,
    ).toEqual([]);
  } finally {
    await current.close();
  }
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

test("style.media arguments complete and preview only media variants", async () => {
  const source =
    'import {style} from "stillwind/react"; const wide = style.media("wide:"); const next = style.media("");';
  const current = project(
    source,
    {},
    `${theme}
@custom-variant wide (@media (width >= 40rem));`,
  );
  try {
    const hover = (await current.query("/workspace/entry.tsx", {
      kind: "hover",
      offset: source.indexOf("wide:") + 1,
    }))!.hover;
    expect(hover?.css).toContain("@media (width >= 40rem /* 640px */)");
    const completions = (await current.query("/workspace/entry.tsx", {
      kind: "completion",
      offset: source.lastIndexOf('("') + 2,
    }))!.completions!;
    const labels = completions.map((item) => item.label);
    expect(labels).toContain("wide:");
    expect(labels).not.toContain("hover:");
    expect(labels).not.toContain("block");
  } finally {
    await current.close();
  }
});

test("style.css inputs preview their selected element and complete by selector option", async () => {
  const source =
    'import {style} from "stillwind/react"; const sheet = style.css("[:host]:flex", { "[button]:": "block" }); const anchored = style.css("p-4", { selector: ":host" }); const plain = style.css("");';
  const current = project(
    source,
    {},
    `${theme}
@custom-variant shell scope(document) (:host);`,
  );
  try {
    const hover = async (offset: number) =>
      (await current.query("/workspace/entry.tsx", { kind: "hover", offset }))!.hover?.css;
    expect(await hover(source.indexOf("flex"))).toMatch(/:host \{\n\s+display: flex;/);
    expect(await hover(source.indexOf("block"))).toMatch(/button \{\n\s+display: block;/);
    expect(await hover(source.indexOf("p-4"))).toMatch(/& \{\n\s+padding:/);
    const labels = async (offset: number) =>
      (await current.query("/workspace/entry.tsx", {
        kind: "completion",
        offset,
      }))!.completions!.map((item) => item.label);
    const plain = await labels(source.lastIndexOf('("') + 2);
    expect(plain).toContain("shell:");
    expect(plain).not.toContain("hover:");
    expect(plain).not.toContain("block");
    expect(await labels(source.indexOf("p-4"))).toContain("block");
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
      color: { red: 1, green: 0, blue: 0, alpha: 1 },
    }))!.edits!;
    expect(applyEdits(source, presentation)).toContain("hover:bg-[#ff0000]");
  } finally {
    await current.close();
  }
});

test("arrays keep dynamic boundaries, category order, and comments, while fix all composes nonoverlapping edits", async () => {
  const source = `import {style} from "stillwind/react";
const first = style("block"); const second = style("flex"); const marker = style.ref();
const value = style([
  "p-[16px]", // trailing comment
  first, second, marker,
  unknown,
  "flex", "block",
]);`;
  const current = project(source, {
    lint: {
      extends: [lintPresets.recommended],
      rules: {
        "prefer-sorted-utilities": "warn",
      },
    },
  });
  try {
    const edits = (await current.query("/workspace/entry.tsx", {
      kind: "fixes",
    }))!.edits!;
    expect(edits.every((edit, index) => index === 0 || edits[index - 1].end <= edit.start)).toBe(
      true,
    );
    const fixed = applyEdits(source, edits);
    expect(fixed).toContain("// trailing comment");
    const array = fixed.slice(fixed.indexOf("const value"));
    expect(array.indexOf("first")).toBeLessThan(array.indexOf("second"));
    expect(fixed).toMatch(/"flex",\s*"block"/);
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

test("sort targets are independent and base inputs retain their original order", async () => {
  const source =
    'import {style} from "stillwind/react"; const z=style("p-4"); const a=style("block"); const marker=style.ref(); const value=style([z,a,marker,"flex block"]);';
  const current = project(source, {
    sort: {
      stringLiterals: false,
    },
  });
  try {
    const sorted = applyEdits(
      source,
      (await current.query("/workspace/entry.tsx", {
        kind: "sort",
      }))!.edits!,
    );
    expect(sorted).toContain('[marker,z,a,"flex block"]');
    const next = project(sorted, {
      sort: {
        stringLiterals: false,
      },
    });
    try {
      expect(
        (await next.query("/workspace/entry.tsx", {
          kind: "sort",
        }))!.edits,
      ).toEqual([]);
    } finally {
      await next.close();
    }
  } finally {
    await current.close();
  }
  const strings = project(source, {
    sort: {
      arrays: false,
    },
  });
  try {
    const sorted = applyEdits(
      source,
      (await strings.query("/workspace/entry.tsx", {
        kind: "sort",
      }))!.edits!,
    );
    expect(sorted).toContain('[z,a,marker,"block flex"]');
  } finally {
    await strings.close();
  }
});

test("empty rule keys retain completion prefixes, hover selectors, and utility grouping", async () => {
  for (const key of ["", ":"]) {
    const name = JSON.stringify(key);
    const incomplete = `import {style} from "stillwind/react"; const card=style({"hover:":{${name}:"bg-br`;
    const current = project(incomplete);
    try {
      const items = (await current.query("/workspace/entry.tsx", {
        kind: "completion",
        offset: incomplete.length,
      }))!.completions!;
      expect(items.find((item) => item.label === "bg-brand")).toMatchObject({
        candidate: "hover:bg-brand",
        start: incomplete.indexOf("bg-br"),
        end: incomplete.length,
        newText: "bg-brand",
      });
    } finally {
      await current.close();
    }
    const source = `import {style} from "stillwind/react"; const card=style({"data-z":1,${name}:"flex block","hover:":{${name}:"bg-brand"}});`;
    const complete = project(source, { sort: { stringLiterals: true, arrays: true } });
    try {
      expect(
        (await complete.query("/workspace/entry.tsx", {
          kind: "hover",
          offset: source.indexOf("bg-brand"),
        }))!.hover?.css,
      ).toContain("&:hover");
      const sorted = applyEdits(
        source,
        (await complete.query("/workspace/entry.tsx", {
          kind: "sort",
        }))!.edits!,
      );
      expect(sorted).toContain(
        `style({${name}:"block flex","hover:":{${name}:"bg-brand"},"data-z":1})`,
      );
    } finally {
      await complete.close();
    }
  }
});

test("array sorting retains comments attached after an expression", async () => {
  const source =
    'import {style} from "stillwind/react"; const value=style(["flex" /* attached */, "block", "p-4"]);';
  const current = project(source);
  try {
    const sorted = applyEdits(
      source,
      (await current.query("/workspace/entry.tsx", {
        kind: "sort",
      }))!.edits!,
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

test("input groups retain category order and place each ref marker before its projections", async () => {
  const source = `import {style} from "stillwind/react";
// 日本語 🦀
const z=style({"hover:":"p-4"}); const a=style(["p-8", {"--x":1}]);
const first=style.ref(); const alias=first; const second=style.ref();
const value=style([{ "data-z":1 }, {"--z":2}, "flex block", z, second.asContainer(),
 first.positionAnchor(), a, alias, second, first.asAnchor(), "p-4"]);`;
  const current = project(source);
  try {
    const result = applyEdits(
      source,
      (await current.query("/workspace/entry.tsx", {
        kind: "sort",
      }))!.edits!,
    );
    expect(result.replace(/\s+/g, " ")).toContain(
      'style([ second, second.asContainer(), alias, first.positionAnchor(), first.asAnchor(), z, a, "block flex", "p-4", {"--z":2},{ "data-z":1 }])',
    );
    const next = project(result);
    try {
      expect(
        (await next.query("/workspace/entry.tsx", {
          kind: "sort",
        }))!.edits,
      ).toEqual([]);
    } finally {
      await next.close();
    }
  } finally {
    await current.close();
  }
});

test("attribute names sort only when enabled, independently from input grouping and utilities", async () => {
  const source = `import {style} from "stillwind/react"; const value=style({"data-z":0,"aria-label":"label","data-a":1,"hover:":"flex block","--z":2,"--a":3});`;
  for (const [sort, expected] of [
    [{}, '{"hover:":"block flex","--z":2,"--a":3,"data-z":0,"aria-label":"label","data-a":1}'],
    [
      {
        attributes: true,
      },
      '{"hover:":"block flex","--z":2,"--a":3,"aria-label":"label","data-a":1,"data-z":0}',
    ],
    [
      {
        arrays: false,
        stringLiterals: false,
        attributes: true,
      },
      '{"aria-label":"label","data-a":1,"data-z":0,"hover:":"flex block","--z":2,"--a":3}',
    ],
  ] as const) {
    const current = project(source, {
      sort,
    });
    try {
      expect(
        applyEdits(
          source,
          (await current.query("/workspace/entry.tsx", {
            kind: "sort",
          }))!.edits!,
        ),
      ).toContain(expected);
    } finally {
      await current.close();
    }
  }
});

test("argument grouping preserves base and utility order regardless of their generated CSS", async () => {
  const source = `import {style} from "stillwind/react"; const z=style("p-4"); const a=style("p-8"); const ref=style.ref(); const value=style("flex", z, "block", a, ref);`;
  const current = project(source);
  try {
    expect(
      applyEdits(
        source,
        (await current.query("/workspace/entry.tsx", {
          kind: "sort",
        }))!.edits!,
      ),
    ).toContain('style( ref, z, a,"flex", "block")');
  } finally {
    await current.close();
  }
});

test("suppression keeps grouping fixes inside approved ranges while explicit sorting remains available", async () => {
  const source = `import {style} from "stillwind/react"; const base=style("block"); const ref=style.ref();
// swlint-disable-next-line prefer-sorted-utilities
const ignored=style(["flex",base,ref]);
const active=style(["flex",base,ref]);`;
  const current = project(source, {
    lint: {
      extends: [lintPresets.recommended],
      rules: { "prefer-sorted-utilities": "warn" },
    },
  });
  try {
    const fixes = applyEdits(
      source,
      (await current.query("/workspace/entry.tsx", {
        kind: "fixes",
      }))!.edits!,
    );
    expect(fixes).toContain('const ignored=style(["flex",base,ref])');
    expect(fixes).toContain('const active=style([ref,base,"flex"])');
    const sorted = applyEdits(
      source,
      (await current.query("/workspace/entry.tsx", {
        kind: "sort",
      }))!.edits!,
    );
    expect(sorted).toContain('const ignored=style([ref,base,"flex"])');
  } finally {
    await current.close();
  }
});

test("mutated bindings and cyclic aliases delimit input grouping", async () => {
  const source = `import {style} from "stillwind/react";
const base=style("block"); const ref=style.ref(); let changed=base; changed=other;
const left=right; const right=left;
const value=style(["flex",changed,ref,"block",base,left,"p-4",ref]);`;
  const current = project(source);
  try {
    const sorted = applyEdits(
      source,
      (await current.query("/workspace/entry.tsx", {
        kind: "sort",
      }))!.edits!,
    );
    expect(sorted).toContain('style(["flex",changed,ref,base,"block",left,ref,"p-4"])');
  } finally {
    await current.close();
  }
});

test("aria and data variant wrappers remain utility inputs when attribute name sorting is enabled", async () => {
  const source =
    'import {style} from "stillwind/react"; const value=style({"data-z":1,"aria-checked:":"flex block","--x":2,"data-active:":"block","aria-label":"x"});';
  const current = project(
    source,
    {
      sort: {
        attributes: true,
      },
    },
    `${theme} @variant aria-checked { &[aria-checked=true] { @slot; } } @variant data-active { &[data-active] { @slot; } }`,
  );
  try {
    expect(
      applyEdits(
        source,
        (await current.query("/workspace/entry.tsx", {
          kind: "sort",
        }))!.edits!,
      ),
    ).toContain(
      'style({"aria-checked:":"block flex","data-active:":"block","--x":2,"aria-label":"x","data-z":1})',
    );
  } finally {
    await current.close();
  }
});
