import { pathToFileURL } from "node:url";

import { cleanModuleId, fileSourceKind } from "@stillwind/host";
import { prepareSource } from "@stillwind/host/projection";
import { transform } from "esbuild";

/** Finds literal projection imports in a source document without evaluating its code. */
export async function projectionImports(
  code: string,
  id: string,
  context: { parse(code: string): unknown },
) {
  if (!code.includes("stillwind.css")) {
    return [];
  }
  const file = cleanModuleId(id);
  const { format, syntax } = fileSourceKind(id);
  const prepared = await prepareSource({
    resolverId: id,
    checkerFileName: file,
    projectionFileName: file,
    moduleId: id,
    document: {
      code,
      uri: pathToFileURL(file).href,
      format,
    },
    syntax,
  });
  const script = prepared.kind === "projected" ? prepared.projection.code : code;
  const language =
    prepared.kind === "projected" ? prepared.projection.syntax.language : syntax.language;
  const javascript = await transform(script, {
    loader: language === "dts" ? "ts" : language,
    target: "esnext",
    tsconfigRaw: {
      compilerOptions: {
        verbatimModuleSyntax: true,
      },
    },
  });
  const imports = new Set<string>();
  const visit = (node: unknown): void => {
    if (!node || typeof node !== "object") {
      return;
    }
    const record = node as Record<string, unknown>;
    if (
      record.type === "ImportDeclaration" ||
      record.type === "ExportNamedDeclaration" ||
      record.type === "ExportAllDeclaration" ||
      record.type === "ImportExpression"
    ) {
      const source = record.source as { value?: unknown } | undefined;
      if (typeof source?.value === "string" && source.value.endsWith("?stillwind.css")) {
        imports.add(source.value);
      }
    }
    for (const value of Object.values(record)) {
      if (Array.isArray(value)) {
        value.forEach(visit);
      } else if (value && typeof value === "object") {
        visit(value);
      }
    }
  };
  visit(context.parse(javascript.code));
  return [...imports];
}
