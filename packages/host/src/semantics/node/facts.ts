import type { NativeTypeFact as TypeFact } from "@stillwind/native";
import type { Node, SourceFile } from "typescript-7/unstable/ast";
import { isCallExpression } from "typescript-7/unstable/ast/is";
import type { Diagnostic, Type } from "typescript-7/unstable/sync";

import { TypeFactsError, type UnverifiedRange } from "../backend.ts";
import { CheckerSource } from "../checker-source.ts";
import type { AssetTypeDecoder } from "./decoder.ts";

/**
 * Traverses a checker AST in preorder.
 *
 * @param node - Root visited before its descendants.
 * @param callback - Operation called once for the root and each child exposed by `forEachChild`.
 * Thrown errors stop traversal.
 */
export function visit(node: Node, callback: (node: Node) => void) {
  callback(node);
  node.forEachChild((child) => visit(child, callback));
}

/**
 * Flattens a checker diagnostic and its nested message chain for matching and display.
 *
 * @param diagnostic - Checker diagnostic with optional nested messages.
 * @returns Diagnostic text followed recursively by child messages, separated by newlines.
 */
export function diagnosticText(diagnostic: Diagnostic): string {
  return [diagnostic.text, ...(diagnostic.messageChain ?? []).map(diagnosticText)].join("\n");
}

/**
 * Tests whether a TypeScript argument error belongs to a style producer call.
 * Lowering reports unsupported macro arguments as lints, so these errors do not invalidate the
 * facts of the call. The operands still require valid facts.
 *
 * @param diagnostic - Checker diagnostic for `sourceFile`.
 * @param sourceFile - Checker source file that contains the diagnostic.
 * @param mapping - Producer call ranges of the same source.
 * @returns Whether the diagnostic is an argument error (TS2345 or TS2769) whose innermost
 * enclosing call is a recorded producer call.
 */
export function producerArgumentDiagnostic(
  diagnostic: Diagnostic,
  sourceFile: SourceFile,
  mapping: CheckerSource,
): boolean {
  if (diagnostic.code !== 2345 && diagnostic.code !== 2769) {
    return false;
  }
  let owner: Node | undefined;
  visit(sourceFile, (node) => {
    if (
      isCallExpression(node) &&
      node.getStart(sourceFile) <= diagnostic.pos &&
      node.end >= diagnostic.end &&
      (!owner || node.end - node.getStart(sourceFile) < owner.end - owner.getStart(sourceFile))
    ) {
      owner = node;
    }
  });
  return !!owner && mapping.producers.has(`${owner.getStart(sourceFile)}:${owner.end}`);
}

/**
 * Decodes the types of checker expressions matched to original semantic probes.
 *
 * @param sourceFile - Final checker AST for the mapped buffer.
 * @param mapping - Original probes indexed by exact checker AST start and end positions.
 * @param decoder - Decoder with asset brands already registered from producer types.
 * @param nodes - Expression nodes from sourceFile, in AST traversal order.
 * @param typesAt - Returns one type per node, in the supplied order. Undefined means no type is available.
 * @param unverified - Optional array to append unavailable original ranges to. Supplying it keeps
 * other facts usable when a probe lacks an expression or a finite declaration type.
 * @returns Facts in AST traversal order with original byte ranges. Known types that do not
 * represent Stillwind assets produce null values. Unresolved recognition-only probes are skipped
 * in both strict and partial queries.
 * @throws If unverified is omitted and a probe has no matching expression or a required type cannot
 * be resolved. Errors from typesAt also propagate.
 */
export function factsForSource(
  sourceFile: SourceFile,
  mapping: CheckerSource,
  decoder: AssetTypeDecoder,
  nodes: readonly Node[],
  typesAt: (nodes: readonly Node[]) => readonly (Type | undefined)[],
  unverified?: UnverifiedRange[],
): TypeFact[] {
  const expressions: Node[] = [];
  const matched = new Set<string>();
  for (const node of nodes) {
    const key = `${node.getStart(sourceFile)}:${node.end}`;
    if (mapping.probes.has(key)) {
      expressions.push(node);
      matched.add(key);
    }
  }
  for (const [key, probe] of mapping.probes) {
    if (!matched.has(key)) {
      const error = new TypeFactsError(
        mapping.source.moduleId,
        probe.start,
        probe.end,
        "checker",
        "Projected expression has no matching checker AST node",
      );
      if (!unverified) {
        throw error;
      }
      unverified.push({
        moduleId: error.moduleId,
        start: error.start,
        end: error.end,
        reason: error.message,
      });
    }
  }

  const types = typesAt(expressions);
  const facts: TypeFact[] = [];
  for (const [index, expression] of expressions.entries()) {
    const probe = mapping.probes.get(`${expression.getStart(sourceFile)}:${expression.end}`)!;
    const type = types[index];
    const decoded = type
      ? decoder.decode(type, probe.recognitionOnly ?? false)
      : { kind: "unverified" as const };
    if (decoded.kind !== "asset") {
      if (
        decoded.kind === "unverified" ||
        (decoded.kind === "unbranded" && !probe.recognitionOnly)
      ) {
        if (probe.recognitionOnly) {
          continue;
        }
        const error = new TypeFactsError(
          mapping.source.moduleId,
          probe.start,
          probe.end,
          "identity",
          "Cannot determine a finite set of declaration identities. Preserve the value's inferred type, use an explicit union, or use a generic parameter.",
        );
        if (!unverified) {
          throw error;
        }
        unverified.push({
          moduleId: error.moduleId,
          start: error.start,
          end: error.end,
          reason: error.message,
        });
      } else {
        facts.push({
          ...probe,
          value: null,
        });
      }
      continue;
    }
    facts.push({
      ...probe,
      value: decoded.value,
    });
  }

  return facts;
}
