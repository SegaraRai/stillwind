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
 * @returns The rendered document CSS and the module's diagnostic messages.
 */
async function compile(
  code: string,
  options = "",
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
          ? `@import "stillwind";\n@plugin "@stillwind/plugin-iconify" { ${options} }\n`
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

test("both spellings share one icon utility and emit its SVG token once", async () => {
  const { css, diagnostics } = await compile(`import { style } from "stillwind/react";
export const named = style("icon-demo--close");
export const bracket = style("hover:icon-[demo--close]");
`);

  expect(diagnostics).toStrictEqual([]);
  expect(css.match(/--icon-demo--close:/g)).toHaveLength(1);
  expect(css).toContain("mask-image: var(--icon-demo--close)");
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

  expect(bracket.css).toContain("mask-image: var(--icon-demo--close)");
  expect(named.css).not.toContain("--icon-demo--close");
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
