import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import {
  compileModule,
  fileModuleDescriptor,
  loadCssGraph,
  TypeScriptFactsService,
} from "@stillwind/host";
import { NativeDesignSystemLoader, NativeProjectSession } from "@stillwind/native";
import { expect, test, type BenchRunOptions } from "vite-plus/test";

const root = fileURLToPath(new URL("../", import.meta.url));

const loader = new NativeDesignSystemLoader({});
loader.provide(
  0,
  "root.css",
  "@utility flex {display:flex;} @utility red {color:red;} @utility blue {color:blue;} @stillwind-slot utilities;",
  "untitled:root.css",
  "untitled:root.css",
);

const host = {
  describe: (id: string) => fileModuleDescriptor(root, id),
  resolve: async () => null,
  read: async () => {
    expect.fail("Unexpected dependency read");
  },
  watch: () => {},
  warn: () => {},
};

const repeated: BenchRunOptions = {
  iterations: 9,
  time: 0,
  warmupIterations: 2,
  warmupTime: 0,
};

const literal = "import {style} from 'stillwind/react'; export const props=style('flex red');";
const semantic =
  "import {style} from 'stillwind/react'; const token=style.keyframes({from:'red',to:'blue'}); const selected=token; export const props=style(selected(''));";
const fixtures: [string, string, string][] = [
  ["literal", "perf-literal.tsx", literal],
  ["semantic", "perf-semantic.tsx", semantic],
  [
    "vue",
    "perf.vue",
    `<script setup lang="ts">${literal.replace("export const", "const")}</script><template><div v-bind="props" /></template>`,
  ],
  [
    "svelte",
    "perf.svelte",
    `<script lang="ts">${literal.replace("export const", "const")}</script><div {...props}></div>`,
  ],
  [
    "astro",
    "perf.astro",
    `---\n${literal.replace("export const", "const")}\n---\n<div {...props}></div>`,
  ],
];

for (const [name, filename, originalSource] of fixtures) {
  const source = ["vue", "svelte", "astro"].includes(name)
    ? originalSource.replace("stillwind/react", `stillwind/${name}`)
    : originalSource;
  const id = path.join(root, filename);

  for (const operation of ["unchanged", "edit"] as const) {
    test(`host/${name}/${operation}`, async ({ bench, task }) => {
      let facts: TypeScriptFactsService | undefined;
      const session = new NativeProjectSession(loader, true);
      const analyze = (text = source) =>
        compileModule(
          session,
          () =>
            (facts ??= new TypeScriptFactsService({
              root,
              project: {
                kind: "isolated",
              },
            })),
          text,
          id,
          host,
        );
      try {
        let output = await analyze();
        expect(output?.code).toBeTruthy();
        expect(output!.diagnostics).toEqual([]);
        let edit = 0;
        task.meta.measurement = repeated;
        await bench(`host/${name}/${operation}`, { async: true }, async () => {
          output = await analyze(
            operation === "edit" && ++edit % 2 ? source.replaceAll("red", "blue") : source,
          );
        }).run(repeated);
        expect(output?.code).toBeTruthy();
        expect(output!.diagnostics).toEqual([]);
      } finally {
        facts?.close();
      }
    });
  }
}

test("host/semantic/new-service-and-session", async ({ bench, task }) => {
  const options = { ...repeated, iterations: 5, warmupIterations: 1 };
  task.meta.measurement = options;
  task.meta.notes =
    "Includes checker subprocess creation and closure; dynamic imports and converters remain loaded. Uses synthetic local modules without a disk dependency graph.";
  let output: Awaited<ReturnType<typeof compileModule>>;
  await bench("host/semantic/new-service-and-session", { async: true }, async () => {
    const service = new TypeScriptFactsService({
      root,
      project: {
        kind: "isolated",
      },
    });
    try {
      output = await compileModule(
        new NativeProjectSession(loader, true),
        () => service,
        semantic,
        path.join(root, "perf-semantic.tsx"),
        host,
      );
    } finally {
      service.close();
    }
  }).run(options);
  expect(output?.code).toBeTruthy();
  expect(output!.diagnostics).toEqual([]);
});

/** A Tailwind plugin handler with a static utility and a utility family. */
const tailwindPlugin = (api: {
  addUtilities: (utilities: Record<string, Record<string, string>>) => void;
  matchUtilities: (
    utilities: Record<string, (value: string) => Record<string, string>>,
    options: {
      values: Record<string, string>;
    },
  ) => void;
}) => {
  api.addUtilities({
    ".probe": {
      color: "red",
    },
  });
  api.matchUtilities(
    {
      tone: (value) => ({
        color: value,
      }),
    },
    {
      values: {
        blue: "blue",
        green: "green",
      },
    },
  );
};

for (const [name, css] of [
  ["none", '@import "stillwind";'],
  ["tailwind", '@import "stillwind"; @plugin "probe";'],
  ["tailwind-apply", '@import "stillwind"; @plugin "probe"; .a { @apply probe tone-blue; }'],
] as const) {
  test(`host/plugin-load/${name}`, async ({ bench, task }) => {
    const file = path.join(root, "plugin-load.css");
    const load = () =>
      loadCssGraph(
        file,
        {},
        {
          resolve: async (request, importer) => createRequire(importer).resolve(request),
          read: async (id) => (id === file ? css : readFile(id, "utf8")),
          documentUri: (id) => pathToFileURL(id).href,
          sourceMapSource: (id) => id,
          watch: () => {},
          plugins: {
            load: async () => tailwindPlugin,
            resolve: async () => undefined,
            read: (id) => readFile(id, "utf8"),
          },
        },
        undefined,
        undefined,
        {
          root,
        },
      );
    const options = {
      ...repeated,
      iterations: 5,
      warmupIterations: 1,
    };
    task.meta.measurement = options;
    task.meta.notes =
      "Loads the Stillwind framework CSS graph from disk with the plugin statement and the CSS uses of the case.";
    let loaded = await load();
    await bench(`host/plugin-load/${name}`, { async: true }, async () => {
      loaded = await load();
    }).run(options);
    expect(loaded.loader.editorQuery().previewCandidate("flex")).toBeTruthy();
  });
}
