import path from "node:path";
import { fileURLToPath } from "node:url";
import { runInNewContext } from "node:vm";

import {
  compileProject,
  NativeDesignSystemLoader,
  NativeProjectSession,
  type ProjectBindingOutput,
} from "@stillwind/native";
import { prepareSource, type ProjectInput } from "@stillwind/projection";
import * as vueRuntime from "stillwind/runtime/vue";
import { expect, test } from "vite-plus/test";

import { compileModule, fileModuleDescriptor } from "../src/compilation/index.ts";

const root = path.dirname(fileURLToPath(import.meta.url));
const rule = "mismatched-framework-import";
const utilities = "@utility flex {display:flex;} @stillwind-slot utilities;";

/** Returns native output after projecting source using the filename's input format. */
async function compileFixture(
  source: string,
  filename: string,
  framework?: ProjectInput["framework"],
) {
  const id = path.join(root, filename);
  const input = await prepareSource({
    resolverId: id,
    ...fileModuleDescriptor(root, id),
    framework,
    document: {
      ...fileModuleDescriptor(root, id).document,
      code: source,
    },
  });
  const output = compileProject([input], {
    content: utilities,
    name: "framework-imports.css",
    uri: "untitled:framework-imports.css",
    sourceMapSource: "untitled:framework-imports.css",
  }) as ProjectBindingOutput;
  expect(output.modules.length).toBe(1);
  return output.modules[0]!;
}

for (const adapter of ["svelte", "astro"]) {
  test(`TSX without JSX warns at the stillwind/${adapter} source literal`, async () => {
    const literal = `'stillwind/${adapter}'`;
    const source = `// 🧭\nimport { style } from ${literal};\nexport const props = style("flex");`;
    const output = await compileFixture(source, "NoJsx.tsx");
    expect(output.diagnostics).toStrictEqual([]);
    expect(output.code).toBeTruthy();
    expect(output.lintSettings[rule]).toBe("warn");
    expect(output.lintFindings.length).toBe(1);
    const finding = output.lintFindings[0]!;
    expect(finding.rule).toBe(rule);
    expect(finding.uri).toBe(fileModuleDescriptor(root, path.join(root, "NoJsx.tsx")).document.uri);
    expect(finding.start).toBe(Buffer.byteLength(source.slice(0, source.indexOf(literal))));
    expect(finding.end).toBe(finding.start + Buffer.byteLength(literal));
    expect(Buffer.from(source).subarray(finding.start, finding.end).toString()).toBe(literal);
    expect(finding.startLine).toBe(1);
    expect(finding.startColumn).toBe(source.split("\n")[1]!.indexOf(literal));
  });
}

test("plain TypeScript allows each macro adapter without a format warning", async () => {
  for (const adapter of ["react", "preact", "solid", "vue", "svelte", "astro"]) {
    const output = await compileFixture(
      `import { style } from "stillwind/${adapter}"; export const props = style("flex");`,
      "Adapter.ts",
    );
    expect(output.diagnostics, adapter).toStrictEqual([]);
    expect(output.lintFindings, adapter).toStrictEqual([]);
    expect(output.code, adapter).toBeTruthy();
    expect(output.css, adapter).toMatch(/display:\s*flex/);
  }
});

test("Vue JSX compiles to class and style object props with an explicit Vue adapter", async () => {
  const source = [
    'import { style } from "stillwind/vue";',
    "const amount = 2;",
    'const props = style("flex", { "--progress": amount });',
    "const view = <div {...props} />;",
  ].join("\n");
  const output = await compileFixture(source, "VueJsx.tsx", "vue");
  expect(output.diagnostics).toStrictEqual([]);
  expect(output.lintFindings).toStrictEqual([]);
  expect(output.code).toBeTruthy();
  expect(output.code).toMatch(/const view = <div \{\.\.\.props\} \/>;/);
  expect(output.code).not.toMatch(/className|__VLS_ctx|style\('flex'/);
  const sandbox: Record<string, unknown> = { ...vueRuntime };
  const declarations = output.code!.slice(0, output.code!.indexOf("const view"));
  runInNewContext(
    `${declarations.replace(/^import\s+[^;]+;\s*/gm, "")}\nglobalThis.result = props;`,
    sandbox,
  );
  const props = sandbox.result as Record<string, unknown>;
  expect(typeof props.class).toBe("string");
  expect(props.class).toBeTruthy();
  expect(typeof props.style).toBe("object");
  expect((props.style as Record<string, unknown>)["--progress"]).toBe("2");
  expect(Object.keys(props)).toStrictEqual(["class", "style"]);
});

test("Vue SFC with a React macro warns in original coordinates without a framework conflict", async () => {
  const literal = "'stillwind/react'";
  const source = `<script setup>\n// 🧭\nimport {style} from ${literal};\nconst props = style("flex");\n</script><template><div v-bind="props" /></template>`;
  const output = await compileFixture(source, "WrongMacro.vue");
  expect(!output.diagnostics.some((diagnostic) => diagnostic.code === "SW01001")).toBeTruthy();
  expect(output.diagnostics).toStrictEqual([]);
  expect(output.code).toBeTruthy();
  expect(output.lintFindings.length).toBe(1);
  const finding = output.lintFindings[0]!;
  expect(finding.rule).toBe(rule);
  expect(finding.start).toBe(Buffer.byteLength(source.slice(0, source.indexOf(literal))));
  expect(Buffer.from(source).subarray(finding.start, finding.end).toString()).toBe(literal);
  expect(finding.startLine).toBe(2);
});

test("explicit framework constraints remain fatal even for an otherwise allowed input", async () => {
  const output = await compileFixture(
    'import { style } from "stillwind/vue"; export const props = style("flex");',
    "Constraint.ts",
    "react",
  );
  expect(output.code).toBe(null);
  expect(output.diagnostics.some((diagnostic) => diagnostic.code === "SW01001")).toBeTruthy();
  expect(output.lintFindings).toStrictEqual([]);
});

test("host options configure framework import warn, off, and deny with next-line suppression", async () => {
  for (const level of [undefined, "warn", "off", "deny"] as const) {
    for (const suppressed of [false, true]) {
      const loader = new NativeDesignSystemLoader({
        lint: {
          mismatchedFrameworkImport: level,
        },
      });
      loader.provide(0, "framework-imports.css", utilities);
      const session = new NativeProjectSession(loader, false);
      const warnings: string[] = [];
      const source = [
        ...(suppressed ? [`// swlint-disable-next-line ${rule}`] : []),
        'import { style } from "stillwind/svelte";',
        'export const props = style("flex");',
      ].join("\n");
      const id = path.join(root, "Policy.tsx");
      const compile = () =>
        compileModule(
          session,
          () => {
            expect.fail("Literal macros do not require type facts");
          },
          source,
          id,
          {
            describe: (id) => fileModuleDescriptor(root, id),
            resolve: async () => null,
            read: async () => {
              expect.fail("Unexpected dependency read");
            },
            watch: () => {},
            warn: (message) => warnings.push(message),
          },
        );
      const output = await compile();
      expect(output?.code).toBeTruthy();
      expect(output!.diagnostics).toStrictEqual([]);
      expect(output!.lintSettings[rule]).toBe(level ?? "warn");
      const active = !suppressed && level !== "off";
      expect(output!.lintFindings.length).toBe(active ? 1 : 0);
      expect(warnings.length).toBe(active ? 1 : 0);
      if (active) {
        expect(output!.lintFindings[0]!.rule).toBe(rule);
        expect(warnings[0]!).toMatch(/mismatched-framework-import/);
        expect(output!.reports).toContainEqual(
          expect.objectContaining({
            code: rule,
            severity: level === "deny" ? "error" : "warning",
          }),
        );
      }
    }
  }
});
