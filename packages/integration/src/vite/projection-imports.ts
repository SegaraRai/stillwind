import { pathToFileURL } from "node:url";

import { cleanModuleId, fileSourceKind } from "@stillwind/host";
import { prepareSource } from "@stillwind/host/projection";
import { transform } from "esbuild";

/**
 * Finds literal projection imports in a source document without evaluating its code.
 * @param code - Original source, optionally in a supported component format.
 * @param id - Full source identity whose cleaned path determines the frontend and parser language.
 * @param context - Parser for the JavaScript produced after projection and TypeScript/JSX lowering.
 * @returns Distinct static and literal dynamic requests ending in ?stillwind.css, in traversal order.
 * @throws If preparation, JavaScript lowering, or parsing fails.
 */
export async function projectionImports(
  code: string,
  id: string,
  context: {
    /**
     * Parses generated JavaScript into an AST with enumerable child nodes.
     * @param code - JavaScript with TypeScript and JSX syntax lowered.
     * @returns AST whose import/export sources expose string literal values.
     * @throws If parsing fails.
     */
    parse(code: string): unknown;
  },
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
  /**
   * Collects literal projection requests while traversing enumerable AST fields.
   * @param node - Parser AST node or child value. Primitive values are skipped.
   */
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
      const source = record.source as
        | {
            /** Parser literal value, used only when it is a string with the projection suffix. */
            value?: unknown;
          }
        | undefined;
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
