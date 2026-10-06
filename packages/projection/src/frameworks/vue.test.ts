import path from "node:path";

import ts from "typescript-6";
import { expect, test, vi } from "vite-plus/test";
import * as core302 from "vue-language-core-3-0-2";

test("Vue language-core 3.0.2 supplies globals that preserve template value types", async () => {
  vi.doMock("@vue/language-core", () => core302);

  const { convert } = await import("./vue.ts");

  try {
    const filename = path.resolve(import.meta.dirname, "../../../integration/tests/Core302.vue.ts");
    const globalName = path.join(path.dirname(filename), "core-3.0.2-globals.d.ts");
    const projection = convert(
      `<script setup lang="ts">const chosen={class:'identity' as const};</script><template><div v-bind="chosen"/></template>`,
      filename.replace(/\.ts$/, ""),
    );
    expect(projection.checkerGlobals).toBeTruthy();
    const virtual = new Map([
      [filename, projection.code],
      [globalName, projection.checkerGlobals!],
    ]);
    const options: ts.CompilerOptions = {
      target: ts.ScriptTarget.ESNext,
      module: ts.ModuleKind.ESNext,
      moduleResolution: ts.ModuleResolutionKind.Bundler,
      strict: true,
      skipLibCheck: true,
      types: [],
      noEmit: true,
    };
    const host = ts.createCompilerHost(options);
    const orgReadFile = host.readFile.bind(host);
    const orgFileExists = host.fileExists.bind(host);
    const orgGetSourceFile = host.getSourceFile.bind(host);
    host.readFile = (id) => virtual.get(path.normalize(id)) ?? orgReadFile(id);
    host.fileExists = (id) => virtual.has(path.normalize(id)) || orgFileExists(id);
    host.getSourceFile = (id, language, ...args) => {
      const text = virtual.get(path.normalize(id));
      return text === undefined
        ? orgGetSourceFile(id, language, ...args)
        : ts.createSourceFile(id, text, language, true);
    };
    const program = ts.createProgram([...virtual.keys()], options, host);
    const file = program.getSourceFile(filename)!;
    const checker = program.getTypeChecker();
    let expression: ts.PropertyAccessExpression | undefined;
    const visit = (node: ts.Node) => {
      if (
        ts.isPropertyAccessExpression(node) &&
        node.expression.getText(file) === "__VLS_ctx" &&
        node.name.text === "chosen"
      ) {
        expression = node;
      }
      ts.forEachChild(node, visit);
    };
    visit(file);
    expect(expression).toBeDefined();
    const type = checker.getTypeAtLocation(expression!);
    const property = checker.getPropertyOfType(type, "class");
    expect(property).toBeDefined();
    expect(checker.typeToString(checker.getTypeOfSymbolAtLocation(property!, expression!))).toBe(
      '"identity"',
    );
  } finally {
    vi.doUnmock("@vue/language-core");
  }
});
