import assert from "node:assert/strict";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { runInNewContext } from "node:vm";

import { compileProject, NativeDesignSystemLoader, NativeProjectSession } from "@stillwind/native";
import { prepareSource } from "@stillwind/projection";

import * as vueRuntime from "../../core/src/runtime/vue.ts";
import { compileModule, fileModuleDescriptor } from "../src/compiler.ts";
import type { ProjectCompilerOutput, ProjectInput } from "../src/protocol.ts";

const root = path.dirname(fileURLToPath(import.meta.url));
const rule = "framework-import-mismatch";
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
  }) as ProjectCompilerOutput;
  assert.equal(output.modules.length, 1);
  return output.modules[0]!;
}

for (const adapter of ["svelte", "astro"]) {
  void test(`TSX without JSX warns at the stillwind/${adapter} source literal`, async () => {
    const literal = `'stillwind/${adapter}'`;
    const source = `// 🧭\nimport {style} from ${literal};\nexport const props = style('flex');`;
    const output = await compileFixture(source, "NoJsx.tsx");
    assert.deepEqual(output.diagnostics, []);
    assert.ok(output.code);
    assert.equal(output.lintSettings[rule], "warn");
    assert.equal(output.lintFindings.length, 1);
    const finding = output.lintFindings[0]!;
    assert.equal(finding.rule, rule);
    assert.equal(
      finding.uri,
      fileModuleDescriptor(root, path.join(root, "NoJsx.tsx")).document.uri,
    );
    assert.equal(finding.start, Buffer.byteLength(source.slice(0, source.indexOf(literal))));
    assert.equal(finding.end, finding.start + Buffer.byteLength(literal));
    assert.equal(Buffer.from(source).subarray(finding.start, finding.end).toString(), literal);
    assert.equal(finding.startLine, 1);
    assert.equal(finding.startColumn, source.split("\n")[1]!.indexOf(literal));
  });
}

void test("plain TypeScript allows each macro adapter without a format warning", async () => {
  for (const adapter of ["react", "preact", "solid", "vue", "svelte", "astro"]) {
    const output = await compileFixture(
      `import {style} from 'stillwind/${adapter}'; export const props = style('flex');`,
      "Adapter.ts",
    );
    assert.deepEqual(output.diagnostics, [], adapter);
    assert.deepEqual(output.lintFindings, [], adapter);
    assert.ok(output.code, adapter);
    assert.match(output.css, /display:\s*flex/, adapter);
  }
});

void test("Vue JSX compiles to class and style object props with an explicit Vue adapter", async () => {
  const source = [
    "import {style} from 'stillwind/vue';",
    "const amount = 2;",
    "const props = style('flex', {'--progress': amount});",
    "const view = <div {...props} />;",
  ].join("\n");
  const output = await compileFixture(source, "VueJsx.tsx", "vue");
  assert.deepEqual(output.diagnostics, []);
  assert.deepEqual(output.lintFindings, []);
  assert.ok(output.code);
  assert.match(output.code, /const view = <div \{\.\.\.props\} \/>;/);
  assert.doesNotMatch(output.code, /className|__VLS_ctx|style\('flex'/);
  const sandbox: Record<string, unknown> = { ...vueRuntime };
  const declarations = output.code.slice(0, output.code.indexOf("const view"));
  runInNewContext(
    `${declarations.replace(/^import\s+[^;]+;\s*/gm, "")}\nglobalThis.result = props;`,
    sandbox,
  );
  const props = sandbox.result as Record<string, unknown>;
  assert.equal(typeof props.class, "string");
  assert.ok(props.class);
  assert.equal(typeof props.style, "object");
  assert.equal((props.style as Record<string, unknown>)["--progress"], "2");
  assert.deepEqual(Object.keys(props), ["class", "style"]);
});

void test("Vue SFC with a React macro warns in original coordinates without a framework conflict", async () => {
  const literal = "'stillwind/react'";
  const source = `<script setup>\n// 🧭\nimport {style} from ${literal};\nconst props = style('flex');\n</script><template><div v-bind="props" /></template>`;
  const output = await compileFixture(source, "WrongMacro.vue");
  assert.ok(!output.diagnostics.some((diagnostic) => diagnostic.code === "SW01001"));
  assert.deepEqual(output.diagnostics, []);
  assert.ok(output.code);
  assert.equal(output.lintFindings.length, 1);
  const finding = output.lintFindings[0]!;
  assert.equal(finding.rule, rule);
  assert.equal(finding.start, Buffer.byteLength(source.slice(0, source.indexOf(literal))));
  assert.equal(Buffer.from(source).subarray(finding.start, finding.end).toString(), literal);
  assert.equal(finding.startLine, 2);
});

void test("explicit framework constraints remain fatal even for an otherwise allowed input", async () => {
  const output = await compileFixture(
    "import {style} from 'stillwind/vue'; export const props = style('flex');",
    "Constraint.ts",
    "react",
  );
  assert.equal(output.code, null);
  assert.ok(output.diagnostics.some((diagnostic) => diagnostic.code === "SW01001"));
  assert.deepEqual(output.lintFindings, []);
});

void test("CSS configures framework import warn, off, and deny with next-line suppression", async () => {
  for (const level of [undefined, "warn", "off", "deny"] as const) {
    for (const suppressed of [false, true]) {
      const loader = new NativeDesignSystemLoader();
      loader.provide(
        0,
        "framework-imports.css",
        `${level ? `@stillwind {${rule}: ${level};}` : ""} ${utilities}`,
      );
      const session = new NativeProjectSession(loader, false);
      const warnings: string[] = [];
      const source = [
        ...(suppressed ? [`// swlint-disable-next-line ${rule}`] : []),
        "import {style} from 'stillwind/svelte';",
        "export const props = style('flex');",
      ].join("\n");
      const id = path.join(root, "Policy.tsx");
      const compile = () =>
        compileModule(
          session,
          () => {
            throw new Error("Literal macros do not require type facts");
          },
          source,
          id,
          root,
          {
            describe: (id) => fileModuleDescriptor(root, id),
            resolve: async () => null,
            read: async () => {
              throw new Error("Unexpected dependency read");
            },
            watch: () => {},
            warn: (message) => warnings.push(message),
          },
        );
      if (level === "deny" && !suppressed) {
        await assert.rejects(compile(), /framework-import-mismatch/);
        assert.deepEqual(warnings, []);
      } else {
        const output = await compile();
        assert.ok(output?.code);
        assert.deepEqual(output.diagnostics, []);
        assert.equal(output.lintSettings[rule], level ?? "warn");
        const active = !suppressed && level !== "off";
        assert.equal(output.lintFindings.length, active ? 1 : 0);
        assert.equal(warnings.length, active ? 1 : 0);
        if (active) {
          assert.equal(output.lintFindings[0]!.rule, rule);
          assert.match(warnings[0]!, /framework-import-mismatch/);
        }
      }
    }
  }
});
