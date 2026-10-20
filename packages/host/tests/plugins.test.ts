import path from "node:path";
import { fileURLToPath } from "node:url";

import { definePlugin } from "@stillwind/config";
import { NativeProjectSession } from "@stillwind/native";
import { expect, test } from "vite-plus/test";

import { BrowserProject } from "../src/browser/project.ts";
import { compileModule, fileModuleDescriptor } from "../src/compilation/index.ts";
import {
  attachPlugins,
  cssWithSourceMap,
  decodeCssSourceMap,
  loadCssGraph,
} from "../src/css/index.ts";
import { applyEdits } from "../src/editor/sort.ts";
import { TypeScriptFactsService } from "../src/semantics/index.ts";

const root = path.dirname(fileURLToPath(import.meta.url));

test("browser plugin paths complete and link without evaluating plugins", async () => {
  const files = new Map([
    [
      "/workspace/package.json",
      JSON.stringify({
        dependencies: {
          "demo-plugin": "1",
        },
      }),
    ],
    ["/workspace/local.mjs", "throw new Error('must not run');"],
    ["/workspace/quo'te.mjs", "throw new Error('must not run');"],
    [
      "/workspace/node_modules/demo-plugin/package.json",
      JSON.stringify({
        exports: {
          ".": "./index.mjs",
          "./feature": "./feature.mjs",
          "./forbidden": null,
        },
      }),
    ],
    ["/workspace/node_modules/demo-plugin/index.mjs", "throw new Error('must not run');"],
    ["/workspace/node_modules/demo-plugin/feature.mjs", "throw new Error('must not run');"],
  ]);
  const create = (text: string) =>
    new BrowserProject({
      root: "/workspace",
      files: new Map([...files, ["/workspace/theme.css", text]]),
      documentUri: (id) => `memory:${id}`,
      config: {
        css: "theme.css",
        files: ["*.tsx"],
        typeFacts: false,
        plugins: [
          definePlugin({
            name: "configured",
            css: () => expect.fail("plugin execution during navigation"),
          }),
        ],
      },
    });
  const text =
    '/* 🦀 */ @plugin "configured"; @plugin "demo-plugin"; @plugin "./local.mjs"; @plugin "missing";';
  const project = create(text);
  try {
    const links = (await project.query("/workspace/theme.css", {
      kind: "links",
    }))!.links!;
    expect(links.map((link) => [text.slice(link.start, link.end), link.target])).toEqual([
      ["configured", undefined],
      ["demo-plugin", "memory:/workspace/node_modules/demo-plugin/index.mjs"],
      ["./local.mjs", "memory:/workspace/local.mjs"],
      ["missing", undefined],
    ]);
  } finally {
    await project.close();
  }
  for (const [text, prefix, label, expected] of [
    ['/* 🦀 */ @plugin "./loZZ";', "./lo", "./local.mjs", '/* 🦀 */ @plugin "./local.mjs";'],
    ["@plugin './quo", "./quo", "./quo'te.mjs", "@plugin './quo\\'te.mjs'"],
    [
      '@plugin "demo-plugin/f',
      "demo-plugin/f",
      "demo-plugin/feature",
      '@plugin "demo-plugin/feature"',
    ],
    ['@plugin "conf', "conf", "configured", '@plugin "configured"'],
  ]) {
    const project = create(text);
    try {
      const result = await project.query("/workspace/theme.css", {
        kind: "completion",
        offset: text.indexOf(prefix) + prefix.length,
      });
      const item = result!.completions!.find((item) => item.label === label)!;
      expect(item, JSON.stringify(result)).toBeDefined();
      expect(applyEdits(text, [item])).toBe(expected);
      expect(result!.completions!.some((item) => item.label === "demo-plugin/forbidden")).toBe(
        false,
      );
    } finally {
      await project.close();
    }
  }
});

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

test("plugin options keep quoted delimiters and omit comments", async () => {
  const received: Record<string, string>[] = [];
  const plugin = definePlugin({
    name: "demo",
    css: ({ options }) => {
      received.push({
        ...options,
      });
      return "";
    },
  });

  await load(
    plugin,
    `@plugin "demo" {
  /* note: ignored; */
  label: "a;b: c";
  icon: url("data:image/svg+xml;utf8,<svg/>");
  size: 1 /* inline */ 2;
}`,
  );

  expect(received).toStrictEqual([
    {
      label: '"a;b: c"',
      icon: 'url("data:image/svg+xml;utf8,<svg/>")',
      size: "1 2",
    },
  ]);
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

test("files that a plugin host loads for a statement are watched with the graph", async () => {
  const { plugin } = demoPlugin();
  const watched: string[] = [];
  const design = path.join(root, "design.css");
  await loadCssGraph(
    design,
    {},
    {
      resolve: async () => expect.fail("unexpected import"),
      read: async () => '@plugin "./demo.js";\n@stillwind-slot utilities;\n',
      documentUri: (id) => `untitled:${encodeURIComponent(id)}`,
      sourceMapSource: (id) => id,
      watch: (id) => watched.push(id),
      plugins: {
        load: async (specifier, importer, watch) => {
          expect([specifier, importer]).toStrictEqual(["./demo.js", design]);
          watch(path.join(root, "demo.js"));
          return plugin;
        },
        resolve: async () => undefined,
        read: async () => expect.fail("unexpected read"),
      },
    },
    undefined,
    undefined,
    {
      root,
    },
  );
  expect(watched).toContain(path.join(root, "demo.js"));
});
