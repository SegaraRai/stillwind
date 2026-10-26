import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";

import type { IconifyJSON } from "@iconify/types";
import {
  attachPlugins,
  compileModule,
  cssWithSourceMap,
  decodeCssSourceMap,
  fileModuleDescriptor,
  loadCssGraph,
  TypeScriptFactsService,
} from "@stillwind/host";
import { BrowserProject } from "@stillwind/host/browser";
import { NativeProjectSession } from "@stillwind/native";
import { expect, test } from "vite-plus/test";

import { iconify } from "../src/plugin.ts";

const root = import.meta.dirname;

const demo: IconifyJSON = {
  prefix: "demo",
  width: 24,
  height: 24,
  icons: {
    close: {
      body: '<path fill="currentColor" d="M0 0h24v24z"/>',
    },
    flag: {
      body: '<path fill="#f00" d="M0 0h24v24z"/>',
    },
  },
};

/**
 * Compiles a module against the Stillwind stylesheet with the Iconify plugin.
 * @param code - Module text.
 * @param options - Options block of the `@plugin` statement.
 * @param pluginFirst - Whether the `@plugin` statement precedes `@import "stillwind"`. An
 * options block would end the statements that `@import` may follow, so the plugin then receives
 * no options.
 * @param theme - Declarations of a `@theme` block after both statements.
 * @returns The rendered document CSS and the module's diagnostic messages.
 */
async function compile(
  code: string,
  options = "",
  pluginFirst = false,
  theme = "",
): Promise<{
  css: string;
  diagnostics: string[];
}> {
  const loaded = await loadCssGraph(
    path.join(root, "design.css"),
    {},
    {
      resolve: async (specifier, importer) => createRequire(importer).resolve(specifier),
      read: async (id) =>
        id === path.join(root, "design.css")
          ? (pluginFirst
              ? `@plugin "@stillwind/plugin-iconify";\n@import "stillwind";\n`
              : `@import "stillwind";\n@plugin "@stillwind/plugin-iconify" { ${options} }\n`) +
            `@theme { ${theme} }\n`
          : readFile(id, "utf8"),
      documentUri: (id) => `untitled:${encodeURIComponent(id)}`,
      sourceMapSource: (id) => id,
      watch: () => {},
    },
    undefined,
    undefined,
    {
      plugins: [
        iconify({
          collections: {
            demo,
          },
        }),
      ],
      root,
    },
  );
  const session = new NativeProjectSession(loaded.loader, false);
  attachPlugins(session, loaded.plugins);
  const facts = new TypeScriptFactsService({
    root,
    project: {
      kind: "isolated",
    },
  });
  try {
    const output = await compileModule(session, () => facts, code, path.join(root, "entry.tsx"), {
      resolve: async () => null,
      read: async () => expect.fail("unexpected read"),
      describe: (id) => ({
        ...fileModuleDescriptor(root, id),
        document: {
          ...fileModuleDescriptor(root, id).document,
          uri: `untitled:${encodeURIComponent(id)}`,
        },
      }),
      watch: () => {},
      warn: () => {},
    });
    const plan = session.planStylesheet({
      modules: [output!.moduleId],
      coverage: {
        kind: "observed",
      },
      placement: "document",
    });
    return {
      css: decodeCssSourceMap(cssWithSourceMap(plan, new Map())).code,
      diagnostics: (output?.diagnostics ?? []).map((diagnostic) => diagnostic.message),
    };
  } finally {
    facts.close();
  }
}

test("both spellings share one icon utility and emit its SVG asset once", async () => {
  const { css, diagnostics } = await compile(`import { style } from "stillwind/react";
export const named = style("icon-demo--close");
export const bracket = style("hover:icon-[demo--close]");
`);

  expect(diagnostics).toStrictEqual([]);
  expect(css.match(/--sw-icon-svg-[0-9a-z]+:/g)).toHaveLength(1);
  expect(css).toMatch(/mask-image: var\(--sw-icon-svg-[0-9a-z]+\)/);
  expect(css).toMatch(/:hover/);
});

test("the bracket syntax leaves the named spelling undefined", async () => {
  const bracket = await compile(
    `import { style } from "stillwind/react";
export const bracket = style("icon-[demo--close]");
`,
    "syntax: bracket;",
  );
  const named = await compile(
    `import { style } from "stillwind/react";
export const named = style("icon-demo--close");
`,
    "syntax: bracket;",
  );

  expect(bracket.css).toMatch(/mask-image: var\(--sw-icon-svg-[0-9a-z]+\)/);
  expect(named.css).not.toContain("--sw-icon-svg-");
}, 30_000);

test("stylesheet hash length controls the emitted SVG names", async () => {
  const { css, diagnostics } = await compile(
    `import { style } from "stillwind/react"; export const icon = style("icon-demo--close");`,
    "hash-length: 2;",
  );
  expect(diagnostics).toEqual([]);
  expect(css).toMatch(/--sw-icon-svg-[0-9a-z]{2}:/);
  expect(css).toMatch(/mask-image: var\(--sw-icon-svg-[0-9a-z]{2}\)/);
});

test("canonical fixes keep unresolved icons and size values unchanged", async () => {
  const code = `import { style } from "stillwind/react";
export const icons = style("icon-[red] icon-[demo--missing] icon-[2rem] icon-[demo--close] hover:!icon-[demo--close]");`;
  const project = new BrowserProject({
    root: "/workspace",
    files: new Map([
      ["/workspace/entry.tsx", code],
      [
        "/workspace/design.css",
        `@plugin "@stillwind/plugin-iconify";
@variant hover { &:hover { @slot; } }
:root { @stillwind-slot theme; }
@stillwind-slot properties;
*, ::before, ::after { @stillwind-slot property-initials; }
@stillwind-slot utilities;`,
      ],
    ]),
    config: {
      css: "design.css",
      files: ["*.tsx"],
      typeFacts: false,
      lint: {
        rules: {
          "prefer-canonical-utilities": "warn",
        },
      },
      plugins: [
        iconify({
          collections: {
            demo,
          },
        }),
      ],
    },
  });
  try {
    const graph = await project.analyze("/workspace/entry.tsx");
    const reports = graph.batches.flatMap((batch) => batch.reports);
    expect(
      reports
        .filter((report) => report.code === "prefer-canonical-utilities")
        .map((report) =>
          code.slice(report.primary.location.startUtf16, report.primary.location.endUtf16),
        ),
    ).toEqual(["icon-[demo--close]", "hover:!icon-[demo--close]"]);
    const fixes = await project.query("/workspace/entry.tsx", {
      kind: "fixes",
    });
    expect(fixes?.edits?.map((edit) => edit.newText)).toEqual([
      "icon-demo--close",
      "hover:icon-demo--close!",
    ]);
  } finally {
    await project.close();
  }
});

test("lint checks icon implementation only when requested and applies height policies to size inputs", async () => {
  for (const checkImplementation of [false, true]) {
    const project = new BrowserProject({
      root: "/workspace",
      files: new Map([
        [
          "/workspace/entry.tsx",
          `import {style} from "stillwind/react"; export const icon=style("icon-demo--close icon-[20px]");`,
        ],
        [
          "/workspace/design.css",
          `@plugin "@stillwind/plugin-iconify";
:root { @stillwind-slot theme; }
@stillwind-slot properties;
*, ::before, ::after { @stillwind-slot property-initials; }
@stillwind-slot utilities;`,
        ],
      ]),
      config: {
        css: "design.css",
        files: ["*.tsx"],
        typeFacts: false,
        plugins: [
          iconify({
            collections: {
              demo,
            },
          }),
        ],
        lint: {
          rules: {
            "prefer-logical-properties": {
              severity: "warn",
              checkImplementation,
            },
            "no-restricted-length-units": {
              severity: "warn",
              checkImplementation,
              rules: [
                {
                  properties: ["height"],
                  disallow: ["px", "em"],
                },
              ],
            },
            "no-arbitrary-spacing": {
              severity: "warn",
              properties: ["height"],
            },
            "no-restricted-theme-tokens": {
              severity: "warn",
              checkImplementation: true,
              tokens: ["--sw-icon-svg-*"],
            },
          },
        },
      },
    });
    try {
      const graph = await project.analyze("/workspace/entry.tsx");
      expect(
        graph.batches
          .flatMap((batch) => batch.reports)
          .map((report) => report.code)
          .sort(),
      ).toEqual(
        (checkImplementation
          ? [
              "prefer-logical-properties",
              "prefer-logical-properties",
              "no-restricted-length-units",
              "no-restricted-length-units",
              "no-arbitrary-spacing",
            ]
          : ["no-restricted-length-units", "no-arbitrary-spacing"]
        ).sort(),
      );
    } finally {
      await project.close();
    }
  }
});

test("editor colors omit icon utilities while keeping their hover previews", async () => {
  const code = `import { style } from "stillwind/react";
export const icons = style("icon-demo--close hover:icon-[demo--flag] bg-brand");`;
  const project = new BrowserProject({
    root: "/workspace",
    files: new Map([
      ["/workspace/entry.tsx", code],
      [
        "/workspace/design.css",
        `@theme { --color-brand: red; }
@utility bg-* { background-color: --value(--color-*); }
@variant hover { &:hover { @slot; } }
@plugin "@stillwind/plugin-iconify";
:root { @stillwind-slot theme; }
@stillwind-slot utilities;`,
      ],
    ]),
    config: {
      css: "design.css",
      files: ["*.tsx"],
      typeFacts: false,
      plugins: [
        iconify({
          collections: {
            demo,
          },
        }),
      ],
    },
  });
  try {
    const result = await project.query("/workspace/entry.tsx", {
      kind: "colors",
    });
    expect(result!.colors!.map((color) => code.slice(color.start, color.end))).toEqual([
      "bg-brand",
    ]);
    for (const [candidate, declaration] of [
      ["icon-demo--close", "mask-image:"],
      ["hover:icon-[demo--flag]", "background-image:"],
    ]) {
      const result = await project.query("/workspace/entry.tsx", {
        kind: "hover",
        offset: code.indexOf(candidate),
      });
      expect(result?.hover?.css).toContain(declaration);
    }
  } finally {
    await project.close();
  }
});

test.for([
  ["icon-demo--close size-full", false],
  ["size-full icon-demo--close", false],
  ["icon-demo--close size-full", true],
  ["size-full icon-demo--close", true],
] as const)(
  "built-in utilities override icon sizes: %s, plugin first: %s",
  { timeout: 30_000 },
  async ([classes, pluginFirst]) => {
    const { css, diagnostics } = await compile(
      `import { style } from "stillwind/react";
export const icon = style(${JSON.stringify(classes)});
`,
      "",
      pluginFirst,
    );
    expect(diagnostics).toStrictEqual([]);
    const rule = css.slice(css.indexOf("@layer utilities"));
    const height = rule.indexOf("height: var(--sw-icon-size, 1em)");
    expect(height).toBeGreaterThan(-1);
    expect(rule.indexOf("width:100%")).toBeGreaterThan(rule.indexOf("width: max-content"));
    expect(rule.indexOf("height:100%")).toBeGreaterThan(height);
  },
);

test("editors complete @iconify/json set names, then the icons of the typed set", async () => {
  const code = `import { style } from "stillwind/react";
export const icons = style("icon-");
export const named = style("hover:icon-demo--");`;
  const css = `@plugin "@stillwind/plugin-iconify";
.probe { @apply icon-demo--close; }
@stillwind-slot utilities;`;
  const packages = new Map([
    [
      "/workspace/node_modules/@iconify/json/collections.json",
      JSON.stringify({
        demo: {},
      }),
    ],
    ["/workspace/node_modules/@iconify/json/json/demo.json", JSON.stringify(demo)],
  ]);
  const project = new BrowserProject({
    root: "/workspace",
    files: new Map([
      ["/workspace/entry.tsx", code],
      ["/workspace/design.css", css],
    ]),
    config: {
      css: "design.css",
      files: ["*.tsx"],
      typeFacts: false,
      plugins: [iconify()],
    },
    plugins: {
      load: async () => expect.fail("unexpected plugin load"),
      resolve: async (specifier) =>
        packages.has(`/workspace/node_modules/${specifier}`)
          ? `/workspace/node_modules/${specifier}`
          : undefined,
      read: async (file) => packages.get(file) ?? expect.fail(`unexpected read of ${file}`),
    },
  });
  try {
    const complete = async (file: string, text: string, after: string) =>
      (
        await project.query(file, {
          kind: "completion",
          offset: text.indexOf(after) + after.length,
        })
      )?.completions?.map((item) => [item.label, item.retrigger === true]) ?? [];
    expect(await complete("/workspace/entry.tsx", code, '"icon-')).toContainEqual([
      "icon-demo--",
      true,
    ]);
    expect(await complete("/workspace/entry.tsx", code, '"hover:icon-demo--')).toEqual(
      expect.arrayContaining([
        ["hover:icon-demo--close", false],
        ["hover:icon-demo--flag", false],
      ]),
    );
    expect(await complete("/workspace/design.css", css, "@apply icon-de")).toEqual(
      expect.arrayContaining([["icon-demo--", true]]),
    );
    expect(await complete("/workspace/design.css", css, "@apply icon-demo--")).toContainEqual([
      "icon-demo--flag",
      false,
    ]);
  } finally {
    await project.close();
  }
});

test("size utilities set the height of icon boxes without inheritance", async () => {
  const { css, diagnostics } = await compile(
    `import { style } from "stillwind/react";
export const named = style("icon-demo--close icon-lg");
export const full = style("icon-[demo--flag] icon-full");
export const before = style("before:icon-demo--close before:icon-[2rem] icon-(--size)");
`,
    "",
    false,
    "--icon-size-lg: 1.5rem;",
  );

  expect(diagnostics).toStrictEqual([]);
  for (const value of ["var(--icon-size-lg)", "100%", "2rem", "var(--size)"]) {
    expect(css).toContain(`--sw-icon-size: ${value};`);
  }
  expect(css).toContain("--icon-size-lg:1.5rem");
  expect(css).toMatch(/background-image: var\(--sw-icon-svg-[0-9a-z]+\);/);
  expect(css).toMatch(/@property --sw-icon-size \{\s*syntax: "\*";\s*inherits: false;\s*\}/);
}, 30_000);
