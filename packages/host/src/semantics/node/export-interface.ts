import type { SourceFile } from "tsrs/ast";
import {
  SignatureKind,
  SymbolFlags,
  TypeFlags,
  type Checker,
  type Type,
  type Symbol as TypeSymbol,
} from "tsrs/sync";

import type { AssetTypeDecoder } from "./decoder.ts";

/** Types that the walk describes by their printed form. */
const printedTypeFlags =
  TypeFlags.Any |
  TypeFlags.Unknown |
  TypeFlags.StringLike |
  TypeFlags.NumberLike |
  TypeFlags.BigIntLike |
  TypeFlags.BooleanLike |
  TypeFlags.EnumLike |
  TypeFlags.ESSymbolLike |
  TypeFlags.Null |
  TypeFlags.Undefined |
  TypeFlags.Void |
  TypeFlags.Never |
  TypeFlags.NonPrimitive |
  TypeFlags.TypeParameter;

/** Nesting depth after which the walk gives up and reports no interface. */
const maxDepth = 12;

/** Number of visited types after which the walk gives up and reports no interface. */
const maxTypes = 2000;

/** Signals that a type is too large or too deep to describe. */
class InterfaceLimit extends Error {}

/**
 * Describes the exports of a checker file as consumers observe them through type facts.
 *
 * The description names each export with the decoded candidates of asset types and the structure
 * of other types. It omits the parts of an asset type that facts do not read, such as the utility
 * strings in the output type of a style, so an edit of those strings keeps the description. A type
 * declared in a package or library file is described by its name and type arguments, because its
 * declaration does not change while the project is edited.
 *
 * @param checker - Checker of the snapshot that contains `file`.
 * @param file - Checker file whose exports are described.
 * @param decoder - Decoder with the producer types of `file` and the files it imports registered.
 * @returns A string that is equal for two snapshots when consumers read the same facts from the
 * exports, or undefined when the exports are too large or too deep to describe. Undefined must be
 * treated as a changed interface.
 */
export function exportInterface(
  checker: Checker,
  file: SourceFile,
  decoder: AssetTypeDecoder,
): string | undefined {
  const module = checker.getSymbolAtLocation(file);
  if (!module) {
    return "";
  }
  const visited = new Map<number, number>();
  const walk = (type: Type | undefined, depth: number): string => {
    if (!type) {
      return "?";
    }
    if (type.flags & printedTypeFlags) {
      return checker.typeToString(type);
    }
    const seen = visited.get(type.id);
    if (seen !== undefined) {
      return `@${seen}`;
    }
    if (depth > maxDepth || visited.size >= maxTypes) {
      throw new InterfaceLimit();
    }
    visited.set(type.id, visited.size);
    const decoded = decoder.decode(type, true);
    if (decoded.kind === "asset") {
      return `asset${JSON.stringify(decoded.value)}`;
    }
    if (type.isUnionType() || type.isIntersectionType()) {
      const members = type
        .getTypes()
        .map((member) => walk(member, depth + 1))
        .sort();
      return `(${members.join(type.isUnionType() ? "|" : "&")})`;
    }
    const symbol = type.getAliasSymbol() ?? type.getSymbol();
    if (symbol && declaredOutsideProject(symbol)) {
      const args = type.getAliasSymbol()
        ? type.getAliasTypeArguments()
        : type.isTypeReference()
          ? checker.getTypeArguments(type)
          : [];
      return `${symbol.name}<${args.map((arg) => walk(arg, depth + 1)).join(",")}>`;
    }
    if (!type.isObjectType()) {
      return checker.typeToString(type);
    }
    const parts = checker
      .getPropertiesOfType(type)
      .map(
        (property) =>
          `${property.name}${property.flags & SymbolFlags.Optional ? "?" : ""}:${walk(checker.getTypeOfSymbol(property), depth + 1)}`,
      );
    for (const [kind, label] of [
      [SignatureKind.Call, "call"],
      [SignatureKind.Construct, "new"],
    ] as const) {
      for (const signature of checker.getSignaturesOfType(type, kind)) {
        const parameters = signature.parameters.map((_, index) =>
          walk(checker.getParameterType(signature, index), depth + 1),
        );
        parts.push(
          `${label}(${parameters.join(",")})=>${walk(checker.getReturnTypeOfSignature(signature), depth + 1)}`,
        );
      }
    }
    for (const info of checker.getIndexInfosOfType(type)) {
      parts.push(`[${walk(info.keyType, depth + 1)}]:${walk(info.valueType, depth + 1)}`);
    }
    return `{${parts.join(";")}}`;
  };
  try {
    return checker
      .getExportsOfModule(module)
      .map((exported) => {
        const target =
          exported.flags & SymbolFlags.Alias ? checker.getAliasedSymbol(exported) : exported;
        const value =
          target.flags & SymbolFlags.Value ? walk(checker.getTypeOfSymbol(target), 0) : "";
        const declared =
          target.flags & SymbolFlags.Type ? walk(checker.getDeclaredTypeOfSymbol(target), 0) : "";
        return `${exported.name}=${value}/${declared}`;
      })
      .join("\n");
  } catch (error) {
    if (error instanceof InterfaceLimit) {
      return undefined;
    }
    throw error;
  }
}

/**
 * Tests whether every declaration of a symbol is in a package or a TypeScript library file.
 *
 * @param symbol - Symbol of a type or type alias.
 * @returns False when the symbol has no declarations or one of them is in a project file.
 */
function declaredOutsideProject(symbol: TypeSymbol): boolean {
  return (
    symbol.declarations.length > 0 &&
    symbol.declarations.every(
      (declaration) =>
        declaration.path.includes("/node_modules/") ||
        /\/lib\.[^/]*\.d\.ts$/.test(declaration.path),
    )
  );
}
