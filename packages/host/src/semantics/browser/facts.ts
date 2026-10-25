import type { NativeExportedValue, NativeIdentity } from "@stillwind/native";
import ts from "typescript-6";

import { CheckerSource } from "../checker-source.ts";

/**
 * Visits an AST in parent-first order.
 *
 * @param node - Root of the subtree to traverse.
 * @param callback - Operation invoked for the root and every child node. Exceptions propagate.
 */
export function visit(node: ts.Node, callback: (node: ts.Node) => void): void {
  callback(node);
  node.forEachChild((child) => visit(child, callback));
}

/**
 * Result of decoding a checker type.
 *
 * `nonAsset` covers types whose values cannot be Stillwind assets: primitive types, macro use types,
 * and object types to which no producer type of the snapshot is assignable. `unbranded` covers object types without
 * asset branding to which a producer type is assignable. Such a value is not a macro receiver, but
 * it may hold an asset whose type was widened, so lowering cannot treat it as a confirmed
 * non-asset. Without producer types, every object type without branding is `unbranded`.
 */
type DecodedType =
  | {
      kind: "asset";
      value: NativeExportedValue;
    }
  | {
      kind: "nonAsset" | "unbranded" | "unverified";
    };

/**
 * Tests whether a type alias is declared in one of `files`.
 *
 * @param type - Checker type to inspect.
 * @param files - Declaration file names of the macro module.
 * @returns Whether the first declaration of the alias symbol belongs to `files`.
 */
function declaredBy(type: ts.Type, files: ReadonlySet<string>): boolean {
  const file = type.aliasSymbol?.declarations?.[0]?.getSourceFile().fileName;
  return file !== undefined && files.has(file);
}

/** Types whose values cannot be Stillwind assets. */
const primitiveTypeFlags =
  ts.TypeFlags.StringLike |
  ts.TypeFlags.NumberLike |
  ts.TypeFlags.BigIntLike |
  ts.TypeFlags.BooleanLike |
  ts.TypeFlags.EnumLike |
  ts.TypeFlags.ESSymbolLike |
  ts.TypeFlags.Null |
  ts.TypeFlags.Undefined |
  ts.TypeFlags.Void |
  ts.TypeFlags.Never;

/**
 * Decodes a checker type into finite declarations from one registered asset family.
 *
 * @param checker - Checker that owns type and location.
 * @param type - Probed type, including unions of declaration candidates.
 * @param location - AST node used to obtain property types.
 * @param identities - Serialized keys mapped to declarations inserted into this snapshot.
 * @param brands - Checker symbol names registered from asset producers and mapped to their families.
 * @param producers - Producer call types of this snapshot, used to detect object types that may hold
 * a widened asset.
 * @param recognitionOnly - True skips checks for widened asset types when only macro recognition is needed.
 * @returns Finite candidates, a confirmed non-asset, an object type that may hold a widened
 * asset, or an unverified value whose declarations cannot be determined.
 */
export function decode(
  checker: ts.TypeChecker,
  type: ts.Type,
  location: ts.Node,
  identities: ReadonlyMap<string, NativeIdentity>,
  brands: ReadonlyMap<string, NativeIdentity["kind"]>,
  producers: readonly ts.Type[],
  recognitionOnly: boolean,
): DecodedType {
  if (type.flags & (ts.TypeFlags.Any | ts.TypeFlags.Unknown)) {
    return {
      kind: "unverified",
    };
  }

  let family: NativeIdentity["kind"] | undefined;
  let recognized = false;
  let valid = true;
  let finite = true;

  const values: NativeIdentity[] = [];
  for (const member of type.isUnion() ? type.types : [type]) {
    let kind: NativeIdentity["kind"] | undefined;
    let identityType: ts.Type | undefined;
    for (const property of checker.getPropertiesOfType(member)) {
      if (property.name === "data-__stillwindStyleIdentity") {
        kind = "style";
      } else if (
        brands.get(property.name) === "ref" &&
        checker.getSignaturesOfType(member, ts.SignatureKind.Call).length
      ) {
        kind = "ref";
      } else if (
        brands.get(property.name) === "keyframes" &&
        checker.getSignaturesOfType(member, ts.SignatureKind.Call).length
      ) {
        kind = "keyframes";
      } else if (brands.get(property.name) === "viewTransition") {
        const method = checker.getPropertyOfType(member, "for");
        if (
          method &&
          checker.getSignaturesOfType(
            checker.getTypeOfSymbolAtLocation(method, location),
            ts.SignatureKind.Call,
          ).length
        ) {
          kind = "viewTransition";
        }
      }

      if (kind) {
        identityType = checker.getTypeOfSymbolAtLocation(property, location);
        if (kind === "ref") {
          const ref = checker.getPropertyOfType(identityType, "ref");
          identityType = ref && checker.getTypeOfSymbolAtLocation(ref, location);
        }
        break;
      }
    }

    const name = (member.aliasSymbol ?? member.getSymbol())?.name;
    recognized ||=
      !!kind || (!!name && /^Style(?:Result|Ref|Keyframes|ViewTransition)$/.test(name));
    if (!kind || (family && kind !== family)) {
      valid = false;
      continue;
    }
    family = kind;
    if (!identityType) {
      finite = false;
      continue;
    }

    for (const candidate of identityType.isUnion() ? identityType.types : [identityType]) {
      const identity = candidate.isStringLiteral() ? identities.get(candidate.value) : undefined;
      if (!identity) {
        finite = false;
        continue;
      }
      if (identity.kind !== kind) {
        valid = false;
        continue;
      }
      values.push(identity);
    }
  }

  if (valid && family === "style" && !finite) {
    // A style result with an unknown declaration still has a known runtime form. The
    // compiler merges it as a base without a CSS ordering dependency.
    return {
      kind: "asset",
      value: {
        kind: "styles",
        value: {
          kind: "style",
          candidates: {
            kind: "universe",
          },
        },
      },
    };
  }

  if (!valid || !family || !values.length) {
    if (recognized) {
      return {
        kind: "unverified",
      };
    }
    const members = type.isUnion() ? type.types : [type];
    const macroFiles = new Set(
      producers.flatMap((producer) => {
        const file = producer.aliasSymbol?.declarations?.[0]?.getSourceFile().fileName;
        return file === undefined ? [] : [file];
      }),
    );
    const widened =
      !recognitionOnly &&
      members.some(
        (member) =>
          !(member.flags & primitiveTypeFlags) &&
          !declaredBy(member, macroFiles) &&
          (producers.length === 0 ||
            producers.some((producer) => checker.isTypeAssignableTo(producer, member))),
      );
    return {
      kind: widened ? "unbranded" : "nonAsset",
    };
  }

  const kinds = {
    style: "styles",
    ref: "refs",
    keyframes: "keyframes",
    viewTransition: "viewTransitions",
  } as const;
  return {
    kind: "asset",
    value: {
      kind: kinds[family],
      value: {
        kind: family,
        candidates: {
          kind: "finite",
          values,
        },
      },
    },
  };
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
  diagnostic: ts.Diagnostic,
  sourceFile: ts.SourceFile,
  mapping: CheckerSource,
): boolean {
  if ((diagnostic.code !== 2345 && diagnostic.code !== 2769) || diagnostic.start === undefined) {
    return false;
  }
  const start = diagnostic.start;
  const end = start + (diagnostic.length ?? 0);
  let owner: ts.Node | undefined;
  visit(sourceFile, (node) => {
    if (
      ts.isCallExpression(node) &&
      node.getStart(sourceFile) <= start &&
      node.end >= end &&
      (!owner || node.end - node.getStart(sourceFile) < owner.end - owner.getStart(sourceFile))
    ) {
      owner = node;
    }
  });
  return !!owner && mapping.producers.has(`${owner.getStart(sourceFile)}:${owner.end}`);
}
