import path from "node:path";
import { fileURLToPath } from "node:url";
import { runInNewContext } from "node:vm";

import {
  NativeDesignSystemLoader,
  NativeProjectSession,
  type ProjectBindingOutput,
} from "@stillwind/native";
import { prepareSource, type ProjectInput } from "@stillwind/projection";
import * as vueRuntime from "stillwind/runtime/class-object";
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
  mode = "release",
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
  const loader = new NativeDesignSystemLoader();
  loader.provide(0, "framework-imports.css", utilities);
  const session = new NativeProjectSession(loader, false, mode);
  const output = session.compile([input]) as ProjectBindingOutput;
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
  for (const adapter of ["react", "preact", "solid", "vue", "svelte", "astro", "vanilla"]) {
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

test("explicit source framework constraints reject a different component format", async () => {
  const output = await compileFixture(
    '<script setup>import { style } from "stillwind/vue"; const props = style("flex");</script><template><div v-bind="props" /></template>',
    "Constraint.vue",
    "react",
  );
  expect(output.code).toBe(null);
  expect(output.diagnostics.some((diagnostic) => diagnostic.code === "SW01001")).toBeTruthy();
  expect(output.lintFindings).toStrictEqual([]);
});

test("compatible macro entrypoints share output conventions and retain runtime helper imports", async () => {
  const output = await compileFixture(
    `import { style as vanilla, applyStyle } from "stillwind/vanilla";
import { style as vue } from "stillwind/vue";
export const first = vanilla("flex", {"--progress": amount});
export const second = vue("flex");
applyStyle(element, first);`,
    "Compatible.ts",
    "solid",
  );
  expect(output.diagnostics).toEqual([]);
  expect(output.lintFindings).toEqual([]);
  expect(output.code).toMatch(/import \{\s*applyStyle\s*\} from "stillwind\/vanilla"/);
  expect(output.code).toContain("applyStyle(element, first)");
  const sandbox = {
    ...vueRuntime,
    amount: 0.5,
    element: {},
    applyStyle: (element: unknown, props: unknown) => ({ element, props }),
  };
  const code = output
    .code!.replace(/^import\s+[^;]+;\s*/gm, "")
    .replaceAll("export const", "const");
  const props = runInNewContext(`${code}; first;`, sandbox);
  expect(props.style).toEqual({ "--progress": "0.5" });
  expect(props.class).toBeTruthy();
});

test("mixed macro bindings keep three runtime formats and CVA helpers independent", async () => {
  const formats = [
    ["react", "className", "object"],
    ["vanilla", "class", "object"],
    ["svelte", "class", "string"],
  ] as const;
  const source = `
import { style as react } from "stillwind/react";
import { style as vanilla, applyStyle } from "stillwind/vanilla";
import { style as svelte } from "stillwind/svelte";
import { style as vue } from "stillwind/vue";
export { applyStyle };
const stillwindMerge = "reserved";
export function values(active, tone) {
${formats.map(([name]) => `const ${name}Factory = ${name}.cva("flex", { variants: { enabled: { false: null, true: { "--enabled": tone } } } });`).join("\n")}
return {
${formats
  .map(
    ([
      name,
    ]) => `${name}: ${name}(active && "flex", { "--tone": tone }, active && { "--enabled": tone }),
${name}Choice: ${name}Factory({ enabled: active }),`,
  )
  .join("\n")}
vue: vue(active && "flex", { "--tone": tone }, active && { "--enabled": tone }),
reserved: stillwindMerge,
};
}`;
  for (const mode of ["release", "development"]) {
    const output = await compileFixture(source, "Mixed.ts", "react", mode);
    expect(output.diagnostics).toEqual([]);
    expect(output.lintFindings).toEqual([]);
    expect(output.code).toBeTruthy();
    const code = output.code!.replaceAll(/"stillwind\/[^"\n]+"/g, (specifier) =>
      JSON.stringify(import.meta.resolve(JSON.parse(specifier))),
    );
    const compiled = (await import(
      `data:text/javascript;base64,${Buffer.from(code).toString("base64")}`
    )) as {
      values: (
        active: boolean,
        tone: string,
      ) => Record<string, { class?: string; className?: string; style?: unknown }>;
    };
    for (const active of [false, true]) {
      const result = compiled.values(active, "red");
      for (const [name, key, representation] of formats) {
        const props = result[name]!;
        expect(props[key]).toEqual(active ? expect.any(String) : undefined);
        expect(props[key === "class" ? "className" : "class"]).toBeUndefined();
        expect(props.style).toEqual(
          representation === "string"
            ? `--tone:red${active ? ";--enabled:red" : ""}`
            : {
                "--tone": "red",
                ...(active
                  ? {
                      "--enabled": "red",
                    }
                  : {}),
              },
        );
        const choice = result[`${name}Choice`]!;
        expect(choice[key]).toEqual(expect.any(String));
        expect(choice.style).toEqual(
          active
            ? representation === "string"
              ? "--enabled:red"
              : {
                  "--enabled": "red",
                }
            : undefined,
        );
      }
      expect(result.vue!.style).toEqual(result.vanilla!.style);
      expect(typeof result.vue!.class).toBe(active ? "string" : "undefined");
      expect(result.reserved).toBe("reserved");
    }
  }
});

test("mixed formats diagnose composition at the operand using its class and inline style", async () => {
  const source = `import { style as react } from "stillwind/react";
import { style as vanilla } from "stillwind/vanilla";
import { style as svelte } from "stillwind/svelte";
const base = vanilla("flex");
const inline = vanilla("flex", { "--tone": tone });
export const valid = svelte(base);
export const wrongKey = react(base);
export const sameKeyInline = svelte(inline);
export const wrongKeyInline = react(inline);`;
  const output = await compileFixture(source, "MixedComposition.ts");
  expect(output.diagnostics).toEqual([]);
  expect(output.code).toBeTruthy();
  expect(
    output.lintFindings.map((finding) => [
      finding.rule,
      finding.message,
      source.slice(finding.start, finding.end),
    ]),
  ).toEqual([
    [
      "invalid-style-composition",
      "a style base with a different class property contributes no class",
      "base",
    ],
    [
      "invalid-style-composition",
      "a style base contributes only its class; its inline style is ignored",
      "inline",
    ],
    [
      "invalid-style-composition",
      "a style base with a different class property contributes no class",
      "inline",
    ],
  ]);
});

test("Vue projection preserves independent macro formats in the same script realm", async () => {
  const source = `<script setup lang="ts">
import { style as react } from "stillwind/react";
import { style as vanilla } from "stillwind/vanilla";
import { style as vue } from "stillwind/vue";
import { style as svelte } from "stillwind/svelte";
const props = defineProps<{ active: boolean; tone: string }>();
const reactProps = react(props.active && "flex", { "--tone": props.tone }, props.active && { "--enabled": props.tone });
const domProps = vanilla(props.active && "flex", { "--tone": props.tone }, props.active && { "--enabled": props.tone });
const stringProps = svelte(props.active && "flex", { "--tone": props.tone }, props.active && { "--enabled": props.tone });
const vueProps = vue(props.active && "flex", { "--tone": props.tone }, props.active && { "--enabled": props.tone });
</script><template><div v-bind="vueProps" /></template>`;
  const output = await compileFixture(source, "Mixed.vue", "vue");
  expect(output.diagnostics).toEqual([]);
  expect(
    output.lintFindings.map((finding) => [finding.rule, source.slice(finding.start, finding.end)]),
  ).toEqual([
    [rule, '"stillwind/react"'],
    [rule, '"stillwind/svelte"'],
  ]);
  for (const format of ["class-name-object", "class-object", "class-string"]) {
    expect(output.code).toContain(`from "stillwind/runtime/${format}"`);
  }
  expect(output.code).toContain('<template><div v-bind="vueProps" /></template>');
  expect(output.code).not.toMatch(/__VLS|style as|props\.value/);
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
