import path from "node:path";
import { fileURLToPath } from "node:url";

import { definePlugin } from "@stillwind/config";
import { NativeProjectSession } from "@stillwind/native";
import { expect, test } from "vite-plus/test";

import { compileModule, fileModuleDescriptor } from "../src/compilation/index.ts";
import {
  attachPlugins,
  cssWithSourceMap,
  decodeCssSourceMap,
  loadCssGraph,
} from "../src/css/index.ts";
import { TypeScriptFactsService } from "../src/semantics/index.ts";

const root = path.dirname(fileURLToPath(import.meta.url));

/**
 * Creates a plugin whose `demo-*` family sets `color` to the candidate's suffix.
 * @returns The plugin and the candidate lists that `resolve` received.
 */
function demoPlugin() {
  const calls: string[][] = [];
  const plugin = definePlugin({
    name: "demo",
    css: ({ options }) =>
      `@stillwind-dynamic demo-*;\n@stillwind-dynamic demo-[*];\n@utility demo-static { color: ${options.color ?? "black"}; }\n`,
    resolve: (candidates) => {
      calls.push([...candidates]);
      return candidates
        .filter((candidate) => candidate !== "demo-missing")
        .map((candidate) => `@utility ${candidate} { color: ${candidate.slice(5)}; }\n`)
        .join("");
    },
  });
  return { plugin, calls };
}

/**
 * Loads a design system whose root stylesheet loads the plugin.
 * @param plugin - Configured plugin named `demo`.
 * @param statement - `@plugin` statement of the root stylesheet.
 * @returns The loaded graph.
 */
async function load(
  plugin: ReturnType<typeof demoPlugin>["plugin"],
  statement = '@plugin "demo";',
) {
  return loadCssGraph(
    path.join(root, "design.css"),
    {},
    {
      resolve: async () => expect.fail("unexpected import"),
      read: async () => `${statement}\n@stillwind-slot utilities;\n`,
      documentUri: (id) => `untitled:${encodeURIComponent(id)}`,
      sourceMapSource: (id) => id,
      watch: () => {},
    },
    undefined,
    undefined,
    {
      plugins: [plugin],
      root,
    },
  );
}

/**
 * Compiles one module and renders its document CSS.
 * @param session - Session with attached plugins.
 * @param code - Module text.
 * @param clean - Whether the module must compile without diagnostics.
 * @returns The rendered CSS.
 */
async function compile(session: NativeProjectSession, code: string, clean = true): Promise<string> {
  const facts = new TypeScriptFactsService({ root, project: { kind: "isolated" } });
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
    if (clean) {
      expect(output?.diagnostics ?? []).toStrictEqual([]);
    }
    const plan = session.planStylesheet({
      modules: [output!.moduleId],
      coverage: {
        kind: "observed",
      },
      placement: "document",
    });
    return decodeCssSourceMap(cssWithSourceMap(plan, new Map())).code;
  } finally {
    facts.close();
  }
}

const entry = `import { style } from "stillwind/react";
export const props = style("demo-static", "demo-red");
export const bracket = style("demo-[blue]");
`;

test("plugin CSS replaces the statement and resolves both spellings of dynamic utilities", async () => {
  const { plugin, calls } = demoPlugin();
  const loaded = await load(plugin, '@plugin "demo" { color: green; }');
  const session = new NativeProjectSession(loaded.loader, false);
  attachPlugins(session, loaded.plugins);

  const css = await compile(session, entry);

  expect(css).toContain("color: green");
  expect(css).toContain("color: red");
  expect(css).toMatch(/\{color: blue;\}/);
  expect(calls).toStrictEqual([["demo-blue", "demo-red"]]);
});

test("sessions keep added definitions, and reloads resolve the earlier candidates again", async () => {
  const { plugin, calls } = demoPlugin();
  const first = await load(plugin);
  const session = new NativeProjectSession(first.loader, false);
  attachPlugins(session, first.plugins);
  await compile(session, entry);

  // A new session of the same graph starts from the original loader.
  const fresh = new NativeProjectSession(first.loader, false);
  attachPlugins(fresh, first.plugins);
  expect(await compile(fresh, entry)).toContain("color: red");

  const reloaded = await loadCssGraph(
    path.join(root, "design.css"),
    {},
    {
      resolve: async () => expect.fail("unexpected import"),
      read: async () => '@plugin "demo";\n@stillwind-slot utilities;\n',
      documentUri: (id) => `untitled:${encodeURIComponent(id)}`,
      sourceMapSource: (id) => id,
      watch: () => {},
    },
    undefined,
    undefined,
    {
      plugins: [plugin],
      root,
      previous: first.plugins,
    },
  );
  const next = new NativeProjectSession(reloaded.loader, false);
  attachPlugins(next, reloaded.plugins);
  expect(await compile(next, entry)).toContain("color: red");
  expect(calls).toStrictEqual([
    ["demo-blue", "demo-red"],
    ["demo-blue", "demo-red"],
  ]);
});

test("a candidate that a plugin leaves undefined is requested once", async () => {
  const { plugin, calls } = demoPlugin();
  const loaded = await load(plugin);
  const session = new NativeProjectSession(loaded.loader, false);
  attachPlugins(session, loaded.plugins);
  const code = `import { style } from "stillwind/react";
export const props = style("demo-missing");
`;

  await compile(session, code, false);
  await compile(session, code, false);

  expect(calls).toStrictEqual([["demo-missing"]]);
});
