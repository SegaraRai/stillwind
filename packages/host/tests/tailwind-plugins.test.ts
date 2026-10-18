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
    {
      root,
      previous,
    },
  );
}

test("theme edits re-evaluate previously requested plugin candidates", async () => {
  const extension = plugin((api) =>
    api.matchUtilities(
      {
        ink: (value) => ({
          "--ink": value,
        }),
      },
      {
        values: api.theme("colors"),
      },
    ),
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
          return {
            width: value,
          };
        },
      },
      {
        values: {
          four: "4px",
          eight: "8px",
        },
      },
    );
    api.matchVariant("state", (value) => `&[data-state="${value}"]`, {
      values: {
        open: "open",
      },
    });
  });
  const source = 'import { style } from "stillwind/react"; export const props = style("probe-");';
  const stylesheet = '@plugin "test"; @stillwind-slot utilities; .target { @apply probe-; }';
  const project = new BrowserProject({
    root: "/workspace",
    files: new Map([
      ["/workspace/theme.css", stylesheet],
      ["/workspace/entry.tsx", source],
    ]),
    config: {
      css: "theme.css",
      files: ["*.tsx"],
      typeFacts: false,
    },
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

test("plugin selectors keep sibling rules and register every class", async () => {
  const loaded = await graph(
    '@plugin "test"; @stillwind-slot utilities;',
    plugin((api) =>
      api.addUtilities({
        ".probe": {
          color: "red",
          "&:hover": {
            color: "blue",
          },
          "&:focus": {
            color: "green",
          },
        },
        ".outer .inner": {
          color: "pink",
        },
      }),
    ),
  );
  const query = loaded.loader.editorQuery();
  const css = query.previewCandidate("probe");
  expect(css).toContain("color:red");
  expect(css).toContain("&:hover");
  expect(css).toContain("&:focus");
  expect(query.previewCandidate("outer")).toBe("& .inner{color:pink;}");
  expect(query.previewCandidate("inner")).toBe(".outer &{color:pink;}");
});

/**
 * Creates a project whose entry module uses `candidates` with the plugin `extension`.
 * @param extension - Plugin that the stylesheet loads.
 * @param candidates - Space-separated candidates for one `style()` call.
 * @param stylesheet - CSS that precedes the plugin in the stylesheet.
 * @param after - CSS that follows the plugin in the stylesheet.
 * @returns The project and its entry module source.
 */
function pluginProject(extension: LoadedPlugin, candidates: string, stylesheet = "", after = "") {
  const source = `import { style } from "stillwind/react"; export const props = style(${JSON.stringify(candidates)});`;
  return {
    source,
    project: new BrowserProject({
      root: "/workspace",
      files: new Map([
        [
          "/workspace/theme.css",
          `${stylesheet}@plugin "test"; ${after} @stillwind-slot utilities;`,
        ],
        ["/workspace/entry.tsx", source],
      ]),
      config: {
        css: "theme.css",
        files: ["*.tsx"],
        typeFacts: false,
      },
      plugins: {
        load: async () => extension,
        resolve: async () => undefined,
        read: async () => expect.fail("unexpected read"),
      },
    }),
  };
}

test("variants apply before plugin selectors, and plugin bodies expand @apply", async () => {
  const extension = plugin((api) => {
    api.addUtilities({
      ".probe": {
        "& > span": {
          color: "red",
          "@apply pad": {},
        },
      },
    });
    api.matchUtilities(
      {
        child: (value) => ({
          "& > span": {
            color: value,
            "@apply gap": {},
          },
        }),
      },
      {
        values: {
          blue: "blue",
        },
      },
    );
  });
  const { project } = pluginProject(
    extension,
    "focus:probe hover:child-blue",
    "@utility pad { padding: 4px; } @utility gap { margin: 2px; } @custom-variant focus (&:focus); @custom-variant hover (&:hover);",
  );
  try {
    const result = await project.analyze("/workspace/entry.tsx");
    expect(result.batches.flatMap((batch) => batch.reports)).toEqual([]);
    const css = project.css().css;
    expect(css).toMatch(/:focus > span\{[^}]*color:red/);
    expect(css).toMatch(/:hover > span\{[^}]*color:blue/);
    expect(css).toMatch(/:focus > span\{[^}]*padding: ?4px/);
    expect(css).toMatch(/:hover > span\{[^}]*margin: ?2px/);
  } finally {
    await project.close();
  }
});

test("utility keyframes appear only with their utility, and top-level keyframes stay global", async () => {
  const extension = plugin((api) => {
    api.addUtilities({
      ".spin": {
        animation: "turn 1s",
        "@keyframes turn": {
          to: {
            rotate: "1turn",
          },
        },
      },
      "@keyframes shared": {
        to: {
          opacity: "1",
        },
      },
    });
    api.matchUtilities(
      {
        fade: (value) =>
          Object.fromEntries([
            ["animation", `${value} 1s`],
            [`--${value}`, "0"],
            [
              `@keyframes ${value}`,
              {
                to: {
                  opacity: value === "fade-half" ? "0.5" : "1",
                },
              },
            ],
            [
              `@property --${value}`,
              {
                syntax: '"*"',
                inherits: "false",
              },
            ],
          ]),
      },
      {
        values: {
          half: "fade-half",
          full: "fade-full",
        },
      },
    );
  });
  const stylesheet = "@stillwind-slot keyframes;";
  const { project, source } = pluginProject(extension, "fade-", stylesheet);
  try {
    const completion = await project.query("/workspace/entry.tsx", {
      kind: "completion",
      offset: source.indexOf("fade-") + 5,
    });
    const item = completion!.completions!.find((item) => item.candidate === "fade-half")!;
    const resolved = await project.query("/workspace/entry.tsx", {
      kind: "resolve",
      candidate: item.candidate,
      input: item.input,
      snapshot: item.snapshot,
    });
    expect(resolved?.hover?.css).toContain("fade-half");
    const unused = project.css().css;
    expect(unused).toContain("@keyframes shared");
    expect(unused).not.toContain("@keyframes turn");
    expect(unused).not.toContain("@keyframes fade-half");
    expect(unused).not.toContain("@property --fade-half");
  } finally {
    await project.close();
  }
  const used = pluginProject(extension, "spin fade-full", stylesheet);
  try {
    await used.project.analyze("/workspace/entry.tsx");
    const css = used.project.css().css;
    expect(css).toContain("@keyframes shared");
    expect(css).toContain("@keyframes turn");
    expect(css).toContain("@keyframes fade-full");
    expect(css).toContain("@property --fade-full");
    expect(css).not.toContain("@keyframes fade-half");
    expect(css).not.toContain("@property --fade-half");
  } finally {
    await used.project.close();
  }
});

test("dynamic variant ordering and explicit type hints follow v4 callback contracts", async () => {
  const loaded = await graph(
    '@plugin "test";',
    plugin((api) => {
      api.matchVariant("rank", (value) => `&[data-rank="${value}"]`, {
        sort: (a, b) => Number(b.value) - Number(a.value),
      });
      api.addVariant("both", {
        "&:hover": "@slot",
        "&:focus": "@slot",
      });
      api.addVariant("2d", "&[data-dimension='two']");
      api.matchUtilities(
        {
          probe: (value) => ({
            width: value,
          }),
        },
        {
          type: "length",
          values: {
            DEFAULT: "1px",
          },
        },
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
    return (api) =>
      api.addUtilities({
        ".test": {
          color: "red",
        },
      });
  });
  const loaded = await graph(
    '@plugin "test" { flag: false; count: 2; names: "a", "b"; empty: null; }',
    extension,
  );
  expect(options).toEqual([
    {
      flag: false,
      count: 2,
      names: ["a", "b"],
      empty: null,
    },
  ]);
  expect(loaded.loader.editorQuery().previewCandidate("test")).toContain("color:red");
  expect(
    defaultPluginExport(
      {
        default: () => {},
      },
      "test",
    ),
  ).toBeTypeOf("function");
  expect(
    defaultPluginExport(
      {
        default: plugin(() => {}),
      },
      "test",
    ),
  ).toHaveProperty("handler");
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
        api.addBase({
          input: {
            accentColor: api.theme("colors.brand"),
          },
        });
        api.addComponents({
          ".card:hover": {
            color: api.theme("colors.brand"),
            padding: "1rem",
          },
        });
      },
      {
        theme: {
          extend: {
            colors: {
              brand: "green",
              extra: "pink",
            },
            fontSize: {
              sm: [
                ".8rem",
                {
                  lineHeight: "1.2",
                },
              ],
            },
          },
        },
      },
    ),
  );
  expect(observed).toEqual([
    "blue",
    [
      ".8rem",
      {
        lineHeight: "1.2",
      },
    ],
    "blue",
  ]);
  expect(loaded.loader.editorQuery().previewCandidate("card")).toContain("&:hover");
  expect(loaded.loader.editorQuery().themeValues()["--color-extra"]).toBe("pink");
  expect(new NativeProjectSession(loaded.loader, false).baseCss().css).toContain(
    "accent-color:blue",
  );
});

test("source and CSS directive discovery resolve modifiers, negative values, fractions, and variants", async () => {
  const extension = plugin((api) => {
    api.matchUtilities(
      {
        probe: (value, { modifier }) => ({
          width: value,
          "--selected": modifier ?? "none",
        }),
      },
      {
        values: {
          DEFAULT: "1px",
          4: "4px",
          "1/2": "50%",
        },
        modifiers: {
          mirror: "M",
        },
        supportsNegativeValues: true,
      },
    );
    api.matchUtilities(
      {
        ink: (value) => ({
          color: value,
        }),
      },
      {
        type: "color",
        values: {
          brand: "#123456",
        },
      },
    );
    api.addVariant("hocus", ["&:hover", "&:focus"]);
    api.matchVariant("state", (value) => `&[data-state="${value}"]`, {
      values: {
        open: "open",
      },
    });
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
    config: {
      css: "theme.css",
      files: ["*.tsx"],
      typeFacts: false,
    },
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

test("plugin registrations are defaults that built-ins and stylesheet utilities override", async () => {
  const extension = plugin((api) => {
    api.addUtilities({
      ".probe": {
        color: "red",
      },
      ".box": {
        display: "block",
        width: "1em",
      },
    });
    api.matchUtilities(
      {
        icon: (value) => ({
          width: value,
        }),
      },
      {
        values: {
          one: "1em",
        },
      },
    );
  });

  // `@stillwind-utility` defines `size-full` as a built-in.
  const builtin = "@stillwind-utility size-full { width: 100%; }";
  const stylesheet = "@utility probe { color: blue; }";
  for (const [before, after] of [
    [`${builtin} ${stylesheet}`, ""],
    [builtin, stylesheet],
  ]) {
    for (const candidates of ["probe box icon-one size-full", "size-full icon-one box probe"]) {
      const { project } = pluginProject(extension, candidates, before, after);
      try {
        // The overlap lint reports the intended conflicts, so reports are not checked here.
        await project.analyze("/workspace/entry.tsx");
        const css = project.css().css.replaceAll(" ", "");
        const label = `${before} | ${after} | ${candidates}`;
        expect(css.lastIndexOf("color:blue"), label).toBeGreaterThan(css.lastIndexOf("color:red"));
        expect(css.lastIndexOf("width:100%"), label).toBeGreaterThan(css.lastIndexOf("width:1em"));
      } finally {
        await project.close();
      }
    }
  }
});

test("plugin config important and container apply to build and hover, and the last config wins", async () => {
  const extension = plugin(() => {}, {
    presets: [
      {
        important: "#app",
      },
      {
        important: true,
        theme: {
          container: {
            center: true,
            padding: "2rem",
          },
        },
      },
    ],
  } as never);
  const { project, source } = pluginProject(
    extension,
    "container pad",
    "@utility pad { padding: 4px; } @utility container { width: 100%; }",
  );
  try {
    await project.analyze("/workspace/entry.tsx");
    const css = project.css().css;
    expect(css).toContain("padding: 4px!important");
    expect(css).not.toContain("#app{");
    expect(css).toMatch(
      /margin-inline:auto!important;padding-inline:2rem!important;[^}]*width: 100%!important/,
    );
    const resolved = await project.query("/workspace/entry.tsx", {
      kind: "hover",
      offset: source.indexOf("pad") + 1,
    });
    expect(resolved?.hover?.css).toContain("!important");
  } finally {
    await project.close();
  }
});
