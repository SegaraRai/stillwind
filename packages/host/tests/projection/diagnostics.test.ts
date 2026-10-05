import path from "node:path";

import { NativeDesignSystemLoader, NativeProjectSession } from "@stillwind/native";
import { expect, test, type TestContext } from "vite-plus/test";

import { CompilationFailure, compileModule, fileModuleDescriptor } from "../../src/compiler.ts";
import { TypeScriptFactsService } from "../../src/type-facts.ts";

const root = path.resolve(import.meta.dirname, "../../../integration/testss");
const css = "@utility red {color:red;} @utility blue {color:blue;} @stillwind-slot utilities;";

function component(format: string, utility = "red") {
  const code = `import {style} from 'stillwind/${format}';const props=style('${utility}');`;
  return format === "astro"
    ? `---\n${code}\n---\n<div {...props}/>`
    : format === "vue"
      ? `<script setup lang="ts">${code}</script><template><div v-bind="props"/></template>`
      : `<script lang="ts">${code}</script><div {...props}></div>`;
}

function harness(t: TestContext, files: Record<string, string>, entry: string) {
  const loader = new NativeDesignSystemLoader();
  loader.provide(0, "theme.css", css, "untitled:theme.css");
  const session = new NativeProjectSession(loader, true);
  const watched = new Set<string>();
  let facts: TypeScriptFactsService | undefined;
  t.onTestFinished(() => facts?.close());
  const compile = (diagnostics: "throw" | "collect" = "throw") =>
    compileModule(
      session,
      () => (facts ??= new TypeScriptFactsService(root)),
      files[entry],
      path.join(root, entry),
      root,
      {
        describe: (id) => fileModuleDescriptor(root, id),
        resolve: async (specifier, importer) =>
          specifier.startsWith(".")
            ? { id: path.resolve(path.dirname(importer), specifier) }
            : null,
        read: async (id) => files[path.relative(root, id)],
        watch: (id) => {
          watched.add(id);
        },
        warn: () => {},
      },
      { diagnostics },
    );
  return { compile, session, watched };
}

for (const { format, broken, code, converter } of [
  {
    format: "vue",
    broken: "<template><div></template>",
    code: "SW01013",
    converter: "@vue/language-core",
  },
  {
    format: "svelte",
    broken: "<!-- 日本語 🧭 -->\r\n<div>{</div>",
    code: "SW01013",
    converter: "svelte2tsx",
  },
  {
    format: "astro",
    broken: "<script>const identity=<T>(value:T)=>value;</script>",
    code: "SW10007",
    converter: "@astrojs/compiler",
  },
]) {
  test(`${format}: throw and collect preserve reports, watch output and recovery`, async (t) => {
    const entry = `Invalid.${format}`;
    const files = { [entry]: component(format) };
    const { compile, session } = harness(t, files, entry);
    expect((await compile())?.diagnostics).toEqual([]);
    const before = session.css();
    files[entry] = broken;
    const failure = await compile().catch((error: unknown) => error);
    expect(failure).toBeInstanceOf(CompilationFailure);
    if (!(failure instanceof CompilationFailure)) {
      expect.fail("expected compilation reports");
    }
    const output = await compile("collect");
    expect(output?.code).toBeNull();
    expect(output?.css).toBe("");
    expect(output?.composition).toEqual([]);
    expect(output?.reports).toEqual(failure.batches[0].reports);
    expect(output?.diagnostics[0]).toMatchObject({
      code,
      uri: fileModuleDescriptor(root, path.join(root, entry)).document.uri,
    });
    expect(output?.reports[0].notes).toContainEqual(
      expect.stringMatching(new RegExp(`^Converter: ${converter}@`)),
    );
    expect(session.css()).toBe(before);
    expect(session.diagnosticBatches().find((batch) => batch.module === entry)?.reports).toEqual(
      output?.reports,
    );
    const reload = session.newLoader();
    reload.provide(0, "theme.css", css, "untitled:theme.css");
    expect(() => session.reload(reload)).toThrow(/CSS reload compilation failed/);
    expect(session.css()).toBe(before);
    expect(session.diagnosticBatches().find((batch) => batch.module === entry)?.reports).toEqual(
      output?.reports,
    );
    files[entry] = component(format, "blue");
    expect((await compile("collect"))?.reports).toEqual([]);
    expect(session.css()).toMatch(/color:\s*blue/);
    expect(session.diagnosticBatches().find((batch) => batch.module === entry)?.reports).toEqual(
      [],
    );
  });
}

test("a failed dependency reports its own revision and remains watched for recovery", async (t) => {
  const entry = "Entry.vue";
  const files = {
    [entry]:
      "<script setup lang='ts'>import {style} from 'stillwind/vue';import {base} from './Broken.vue';const props=style(base,'red');</script>",
    "Broken.vue": "<template><div></template>",
  };
  const { compile, session, watched } = harness(t, files, entry);
  const output = await compile("collect");
  expect(output?.moduleId).toBe(entry);
  expect(output?.reports[0]).toMatchObject({
    code: "SW01013",
    primary: {
      location: {
        uri: fileModuleDescriptor(root, path.join(root, "Broken.vue")).document.uri,
      },
    },
  });
  expect(session.diagnosticBatches()).toMatchObject([
    {
      module: "Broken.vue",
      reports: output?.reports,
    },
  ]);
  expect(watched).toContain(path.join(root, "Broken.vue"));
  files["Broken.vue"] =
    "<script lang='ts'>import {style} from 'stillwind/vue';export const base=style('blue');</script>";
  expect((await compile("collect"))?.diagnostics).toEqual([]);
  expect(session.css()).toMatch(/color:\s*blue/);
  expect(
    session.diagnosticBatches().find((batch) => batch.module === "Broken.vue")?.reports,
  ).toEqual([]);
});

test("host I/O failures remain exceptions in collect mode", async () => {
  const loader = new NativeDesignSystemLoader();
  loader.provide(0, "theme.css", css);
  const error = new Error("input unavailable");
  await expect(
    compileModule(
      new NativeProjectSession(loader, true),
      () => {
        expect.fail("no checker query");
      },
      "import {style} from 'stillwind/react';import {base} from './missing.ts';const props=style(base);",
      path.join(root, "Input.tsx"),
      root,
      {
        describe: (id) => fileModuleDescriptor(root, id),
        resolve: async () => ({
          id: path.join(root, "missing.ts"),
        }),
        read: async () => {
          throw error;
        },
        watch: () => {},
        warn: () => {},
      },
      { diagnostics: "collect" },
    ),
  ).rejects.toBe(error);
});
