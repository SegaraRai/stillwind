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
