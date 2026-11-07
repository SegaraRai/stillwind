import { mkdtemp, rm, writeFile } from "node:fs/promises";
import path from "node:path";

import ts from "typescript";
import { expect, test } from "vite-plus/test";

const frameworks = [
  "alpine",
  "astro",
  "lit",
  "preact",
  "qwik",
  "react",
  "solid",
  "svelte",
  "vanilla",
  "vue",
] as const;

test("declaration emit names exported style() results through the framework entrypoint", async () => {
  // The directory stays inside the package so that `stillwind/*` resolves to the packed
  // entrypoints through package self-reference.
  const root = await mkdtemp(path.join(import.meta.dirname, ".test-declaration-emit-"));
  try {
    const files = await Promise.all(
      frameworks.map(async (framework) => {
        const file = path.join(root, `${framework}.ts`);
        await writeFile(
          file,
          [
            `import { style } from "stillwind/${framework}";`,
            `export const base = style("p-2");`,
            `export const composed = style(base, "text-sm");`,
            "",
          ].join("\n"),
        );
        return [framework, file] as const;
      }),
    );
    const declarations = new Map<string, string>();
    const program = ts.createProgram(
      files.map(([, file]) => file),
      {
        declaration: true,
        emitDeclarationOnly: true,
        module: ts.ModuleKind.NodeNext,
        moduleResolution: ts.ModuleResolutionKind.NodeNext,
        rootDir: root,
        skipLibCheck: true,
        strict: true,
        target: ts.ScriptTarget.ES2024,
        types: [],
      },
    );
    const result = program.emit(undefined, (name, text) => declarations.set(name, text));
    const diagnostics = [...ts.getPreEmitDiagnostics(program), ...result.diagnostics].map(
      (diagnostic) => ts.flattenDiagnosticMessageText(diagnostic.messageText, "\n"),
    );
    expect(diagnostics).toEqual([]);

    for (const [framework, file] of files) {
      const declaration = declarations.get(file.replace(/\.ts$/, ".d.ts").replaceAll("\\", "/"));
      expect(declaration).toBeDefined();
      for (const name of ["base", "composed"]) {
        expect(declaration).toMatch(
          new RegExp(`const ${name}: import\\("stillwind/${framework}"[^)]*\\)\\.StyleResultOf<`),
        );
      }
    }
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
