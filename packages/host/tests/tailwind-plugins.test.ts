import path from "node:path";

import { NativeProjectSession } from "@stillwind/native";
import plugin from "tailwindcss/plugin";
import { expect, test } from "vite-plus/test";

import { BrowserProject } from "../src/browser/project.ts";
import { loadCssGraph } from "../src/css/css.ts";
import { defaultPluginExport, type LoadedPlugin, type PluginRuntime } from "../src/css/plugins.ts";

const root = path.resolve(import.meta.dirname);

async function graph(css: string, extension: LoadedPlugin, previous?: PluginRuntime) {
  return loadCssGraph(
    path.join(root, "theme.css"),
    {},
    {
      resolve: async () => expect.fail("unexpected import"),
      read: async () => css,
      documentUri: (id) => `memory:${encodeURIComponent(id)}`,
      sourceMapSource: (id) => id,
      watch: () => {},
      plugins: {
        load: async () => extension,
        resolve: async () => undefined,
        read: async () => expect.fail("unexpected read"),
      },
    },
    undefined,
    undefined,
    { root, previous },
  );
}

test("theme edits re-evaluate previously requested plugin candidates", async () => {
  const extension = plugin((api) =>
    api.matchUtilities({ ink: (value) => ({ "--ink": value }) }, { values: api.theme("colors") }),
  );
  const first = await graph('@plugin "test"; @theme { --color-brand: #123456; }', extension);
  expect((await first.plugins!.query("ink-brand")).previewCandidate("ink-brand")).toContain(
    "#123456",
  );
  const second = await graph(
    '@plugin "test"; @theme { --color-brand: #654321; }',
    extension,
    first.plugins,
  );
  expect(second.loader.editorQuery().previewCandidate("ink-brand")).toContain("#654321");
});

test("decoded values and modifiers register definitions that the compiler resolves", async () => {
  const extension = plugin((api) =>
    api.matchUtilities(
      {
        probe: (value, { modifier }) => ({
          width: value,
          "--modifier": modifier ?? "none",
        }),
      },
      {
        type: "length",
        values: {
          one: "1px",
        },
        modifiers: "any",
      },
    ),
  );
  const loaded = await graph('@plugin "test"; @stillwind-slot utilities;', extension);
  const preview = async (candidate: string) =>
    (await loaded.plugins!.query(candidate)).previewCandidate(candidate);

  expect(await preview("probe-one/[foo_bar]")).toBe("&{width:1px;--modifier:foo bar;}");
  expect(await preview("probe-[calc(100%_-_1rem)]")).toBe(
    "&{width:calc(100% - 1rem);--modifier:none;}",
  );
  expect(await preview("probe-(length:--my-size)")).toBe(
    "&{width:var(--my-size);--modifier:none;}",
  );
});

test("editor catalogs resolve selected script and CSS candidates lazily without emitting unused CSS", async () => {
  const calls: string[] = [];
  const extension = plugin((api) => {
    api.matchUtilities(
      {
        probe: (value) => {
          calls.push(value);
          return { width: value };
        },
      },
      { values: { four: "4px", eight: "8px" } },
    );
    api.matchVariant("state", (value) => `&[data-state="${value}"]`, { values: { open: "open" } });
  });
  const source = 'import { style } from "stillwind/react"; export const props = style("probe-");';
  const stylesheet = '@plugin "test"; @stillwind-slot utilities; .target { @apply probe-; }';
  const project = new BrowserProject({
    root: "/workspace",
    files: new Map([
      ["/workspace/theme.css", stylesheet],
      ["/workspace/entry.tsx", source],
    ]),
    config: { css: "theme.css", files: ["*.tsx"], typeFacts: false },
    plugins: {
      load: async () => extension,
      resolve: async () => undefined,
      read: async () => expect.fail("unexpected read"),
    },
  });
  try {
    const result = await project.query("/workspace/entry.tsx", {
      kind: "completion",
      offset: source.indexOf("probe-") + 6,
    });
    const item = result!.completions!.find((item) => item.candidate === "probe-eight")!;
    expect(item).toBeDefined();
    expect(calls).toEqual([]);
    const resolved = await project.query("/workspace/entry.tsx", {
      kind: "resolve",
      candidate: item.candidate,
      input: item.input,
      snapshot: item.snapshot,
    });
    expect(resolved?.hover?.css).toContain("width: 8px");
    expect(calls).toEqual(["8px"]);
    expect(project.css().css).not.toContain("width:8px");
    const css = await project.query("/workspace/theme.css", {
      kind: "completion",
      offset: stylesheet.indexOf("probe-") + 6,
    });
    const selected = css!.completions!.find((item) => item.candidate === "probe-four")!;
    expect(selected).toBeDefined();
    const cssResolved = await project.query("/workspace/theme.css", {
      kind: "resolve",
      candidate: selected.candidate,
      input: selected.input,
      snapshot: selected.snapshot,
    });
    expect(cssResolved?.hover?.css).toContain("width: 4px");
  } finally {
    await project.close();
  }
});

test("plugin selectors keep sibling rules and lift nested keyframes", async () => {
  const loaded = await graph(
    '@plugin "test"; @stillwind-slot utilities;',
    plugin((api) =>
      api.addUtilities({
        ".probe": {
          color: "red",
          "&:hover": { color: "blue" },
          "&:focus": { color: "green" },
          "@keyframes pulse": { from: { opacity: "0" }, to: { opacity: "1" } },
        },
      }),
    ),
  );
  const css = loaded.loader.editorQuery().previewCandidate("probe");
  expect(css).toContain("color:red");
  expect(css).toContain("&:hover");
  expect(css).toContain("&:focus");
  expect(new NativeProjectSession(loaded.loader, false).baseCss().css).toContain(
    "@keyframes pulse",
  );
});

test("dynamic variant ordering and explicit type hints follow v4 callback contracts", async () => {
  const loaded = await graph(
    '@plugin "test";',
    plugin((api) => {
      api.matchVariant("rank", (value) => `&[data-rank="${value}"]`, {
        sort: (a, b) => Number(b.value) - Number(a.value),
      });
      api.addVariant("both", { "&:hover": "@slot", "&:focus": "@slot" });
      api.addVariant("2d", "&[data-dimension='two']");
      api.matchUtilities(
        { probe: (value) => ({ width: value }) },
        { type: "length", values: { DEFAULT: "1px" } },
      );
    }),
  );
  const query = await loaded.plugins!.query(
    "rank-[2]:probe rank-[10]:probe probe-[length:var(--width)] probe-[red]",
  );
  expect(query.compareCandidates("rank-[10]:probe", "rank-[2]:probe")).toBeLessThan(0);
  expect(query.previewCandidate("probe-[length:var(--width)]")).toContain("var(--width)");
  expect(query.previewCandidate("probe-[red]")).toBeNull();
  expect(query.previewVariant("both:")).toContain("&:focus");
  expect(query.previewVariant("2d:")).toContain("data-dimension");
});

test("Tailwind export forms and typed options preserve the native plugin contract", async () => {
  const options: unknown[] = [];
  const extension = plugin.withOptions((value) => {
    options.push(value);
    return (api) => api.addUtilities({ ".test": { color: "red" } });
  });
  const loaded = await graph(
    '@plugin "test" { flag: false; count: 2; names: "a", "b"; empty: null; }',
    extension,
  );
  expect(options).toEqual([{ flag: false, count: 2, names: ["a", "b"], empty: null }]);
  expect(loaded.loader.editorQuery().previewCandidate("test")).toContain("color:red");
  expect(defaultPluginExport({ default: () => {} }, "test")).toBeTypeOf("function");
  expect(defaultPluginExport({ default: plugin(() => {}) }, "test")).toHaveProperty("handler");
  await expect(graph('@plugin "test" { flag: true; }', () => {})).rejects.toThrow(
    "does not accept options",
  );
});

test("handlers see the complete CSS theme and add base, components, and configured defaults", async () => {
  const observed: unknown[] = [];
  const loaded = await graph(
    `@theme default { --color-brand: red; } @plugin "test"; @theme { --color-brand: blue; } @stillwind-slot utilities;`,
    plugin(
      (api) => {
        observed.push(
          api.theme("colors.brand"),
          api.theme("fontSize.sm"),
          api.config("theme.colors.brand"),
        );
        api.addBase({ input: { accentColor: api.theme("colors.brand") } });
        api.addComponents({ ".card:hover": { color: api.theme("colors.brand"), padding: "1rem" } });
      },
      {
        theme: {
          extend: {
            colors: { brand: "green", extra: "pink" },
            fontSize: { sm: [".8rem", { lineHeight: "1.2" }] },
          },
        },
      },
    ),
  );
  expect(observed).toEqual(["blue", [".8rem", { lineHeight: "1.2" }], "blue"]);
  expect(loaded.loader.editorQuery().previewCandidate("card")).toContain("&:hover");
  expect(loaded.loader.editorQuery().themeValues()["--color-extra"]).toBe("pink");
  expect(new NativeProjectSession(loaded.loader, false).baseCss().css).toContain(
    "accent-color:blue",
  );
});

test("source and CSS directive discovery resolve modifiers, negative values, fractions, and variants", async () => {
  const extension = plugin((api) => {
    api.matchUtilities(
      { probe: (value, { modifier }) => ({ width: value, "--selected": modifier ?? "none" }) },
      {
        values: { DEFAULT: "1px", 4: "4px", "1/2": "50%" },
        modifiers: { mirror: "M" },
        supportsNegativeValues: true,
      },
    );
    api.matchUtilities(
      { ink: (value) => ({ color: value }) },
      { type: "color", values: { brand: "#123456" } },
    );
    api.addVariant("hocus", ["&:hover", "&:focus"]);
    api.matchVariant("state", (value) => `&[data-state="${value}"]`, { values: { open: "open" } });
  });
  const files = new Map([
    [
      "/workspace/theme.css",
      '@plugin "test"; @stillwind-slot utilities; .applied { @apply probe-4/mirror; }',
    ],
    [
      "/workspace/entry.tsx",
      'import { style } from "stillwind/react"; export const a = style("-probe-[3px]/mirror"); export const b = style("probe-1/2"); export const c = style("ink-brand/50"); export const d = style("state-open:hocus:probe");',
    ],
  ]);
  const project = new BrowserProject({
    root: "/workspace",
    files,
    config: { css: "theme.css", files: ["*.tsx"], typeFacts: false },
    plugins: {
      load: async () => extension,
      resolve: async () => undefined,
      read: async () => expect.fail("unexpected read"),
    },
  });
  try {
    const result = await project.analyze("/workspace/entry.tsx");
    expect(
      result.batches.flatMap((batch) => batch.reports),
      JSON.stringify(result.batches),
    ).toEqual([]);
    const css = project.css().css;
    expect(css).toContain("calc(3px * -1)");
    expect(css).toContain("width:50%");
    expect(css).toContain("--selected:M");
    expect(css).toContain("color-mix(");
    expect(css).toContain('data-state="open"');
    expect(css).toContain(".applied");
  } finally {
    await project.close();
  }
});
