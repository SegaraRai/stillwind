import { pathToFileURL } from "node:url";

import { cleanModuleId, fileSourceKind } from "@stillwind/host";
import { prepareSource } from "@stillwind/host/projection";
import { parseSync } from "rolldown/utils";

/**
 * Finds literal projection imports in a source document without evaluating its code.
 * @param code - Original source, optionally in a supported component format.
 * @param id - Full source identity whose cleaned path determines the frontend and parser language.
 * @returns Distinct static and literal dynamic requests ending in ?stillwind.css, in traversal order.
 * @throws If preparation or parsing fails.
 */
export async function projectionImports(code: string, id: string) {
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
  const parser = prepared.kind === "projected" ? prepared.projection.syntax : syntax;
  const parsed = parseSync(file, script, {
    lang: parser.language,
    sourceType: parser.moduleKind,
  });
  if (parsed.errors.length) {
    throw new Error(`${id}: ${parsed.errors.map((error) => error.message).join("\n")}`);
  }
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
    if (record.declare === true) {
      return;
    }
    if (
      (record.type === "ImportDeclaration" && record.importKind !== "type") ||
      ((record.type === "ExportNamedDeclaration" || record.type === "ExportAllDeclaration") &&
        record.exportKind !== "type") ||
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
  visit(parsed.program);
  return [...imports];
}
