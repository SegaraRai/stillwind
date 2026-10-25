import type { NativeExportedValue, NativeIdentity, NativeSymbolicValue } from "@stillwind/native";
import {
  SignatureKind,
  TypeFlags,
  type Checker,
  type Type,
  type Symbol as TypeSymbol,
} from "typescript-7/unstable/sync";

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

/** Types whose values cannot be Stillwind assets. */
const primitiveTypeFlags =
  TypeFlags.StringLike |
  TypeFlags.NumberLike |
  TypeFlags.BigIntLike |
  TypeFlags.BooleanLike |
  TypeFlags.EnumLike |
  TypeFlags.ESSymbolLike |
  TypeFlags.Null |
  TypeFlags.Undefined |
  TypeFlags.Void |
  TypeFlags.Never;

/**
 * Native declaration identity containing its asset kind, declaring module, original byte range, and
 * binding name.
 */
type IdentityId = NativeIdentity;

/**
 * Decodes a string-literal identity type or union using the current declaration table.
 *
 * @param type - Checker type of an identity property, or undefined when the property is
 * unavailable.
 * @param identities - Serialized keys mapped to known native declaration identities.
 * @param kind - Asset family that every candidate must belong to.
 * @returns Candidates in type-member order. Returns `"unknown"` if the type is absent,
 * nonliteral, or unknown to the table, and `"mismatch"` if a candidate belongs to a
 * different asset family.
 */
function knownIdentities(
  type: Type | undefined,
  identities: ReadonlyMap<string, IdentityId>,
  kind: IdentityId["kind"],
): "unknown" | "mismatch" | IdentityId[] {
  if (!type) {
    return "unknown";
  }
  const types = type.isUnionType() ? type.getTypes() : [type];
  const result: IdentityId[] = [];
  let unknown = false;
  for (const candidate of types) {
    const identity = candidate.isStringLiteralType() ? identities.get(candidate.value) : undefined;
    if (!identity) {
      unknown = true;
      continue;
    }
    if (identity.kind !== kind) {
      return "mismatch";
    }
    result.push(identity);
  }
  return unknown ? "unknown" : result;
}

/**
 * Packages known declarations as a finite native symbolic value.
 *
 * @param kind - Asset family shared by the supplied identities.
 * @param identities - Candidate identities retained in their supplied order.
 * @returns A finite symbolic value containing a copied candidate array.
 */
function symbolic(
  kind: IdentityId["kind"],
  identities: readonly IdentityId[],
): NativeSymbolicValue {
  return {
    kind,
    candidates: {
      kind: "finite",
      values: [...identities],
    },
  };
}

/** Property lookups owned by one checker snapshot. */
export type TypePropertyCache = {
  /** Symbols keyed by type ID and property name. Missing properties are cached as undefined. */
  named: Map<string, TypeSymbol | undefined>;
  /** Complete property lists keyed by type ID from the same snapshot. */
  all: Map<number, readonly TypeSymbol[]>;
};

/**
 * Decodes checker types into native declaration candidates using brand properties discovered from
 * producer modules.
 */
export class AssetTypeDecoder {
  /**
   * Checker for the current project snapshot, used to inspect exported types and their properties.
   */
  readonly #checker: Checker;
  /** Shared property lookups. The owning service clears them before replacing the snapshot. */
  readonly #properties: TypePropertyCache;
  /**
   * All declaration keys registered in the current virtual project, mapped to original native
   * identities.
   */
  readonly #identities: ReadonlyMap<string, IdentityId>;
  /** Checker symbol IDs of modules whose exported asset brands have already been inspected. */
  readonly #modules = new Set<number>();
  /** Internal names of discovered unique-symbol brand properties mapped to their asset families. */
  readonly #brands = new Map<string, IdentityId["kind"]>();
  /** Registered producer call types, used to detect object types that may hold a widened asset. */
  readonly #producers: Type[] = [];

  /**
   * Creates a decoder for one checker snapshot and declaration table.
   *
   * @param checker - Checker owning all types supplied to this decoder.
   * @param identities - Serialized identity keys inserted into the project's producer calls.
   * @param properties - Property cache for the same checker snapshot. May be shared with other decoders.
   */
  constructor(
    checker: Checker,
    identities: ReadonlyMap<string, IdentityId>,
    properties: TypePropertyCache,
  ) {
    this.#checker = checker;
    this.#identities = identities;
    this.#properties = properties;
  }

  /**
   * Looks up a named property once per snapshot, including missing properties.
   *
   * @param type - Type from this decoder's checker snapshot.
   * @param name - Property name to look up.
   * @returns The property symbol, or undefined when absent. Stores the answer in the shared cache.
   */
  #property(type: Type, name: string): TypeSymbol | undefined {
    const key = `${type.id}:${name}`;
    if (!this.#properties.named.has(key)) {
      this.#properties.named.set(key, this.#checker.getPropertyOfType(type, name));
    }
    return this.#properties.named.get(key);
  }

  /**
   * Looks up all properties of a type once per snapshot.
   *
   * @param type - Type from this decoder's checker snapshot.
   * @returns The cached property list. Do not mutate it.
   */
  #allProperties(type: Type): readonly TypeSymbol[] {
    let properties = this.#properties.all.get(type.id);
    if (!properties) {
      properties = this.#checker.getPropertiesOfType(type);
      this.#properties.all.set(type.id, properties);
    }
    return properties;
  }

  /**
   * Discovers ref, keyframe, and view-transition brands in a producer type's declaring module.
   *
   * @param type - Producer call type from this decoder's checker snapshot. The type is retained for
   * later decoding. Missing alias modules and already inspected modules are skipped. The discovered
   * property names are retained for later decoding.
   */
  register(type: Type): void {
    this.#producers.push(type);
    const module = type.getAliasSymbol()?.getParent();
    if (!module || this.#modules.has(module.id)) {
      return;
    }
    this.#modules.add(module.id);
    for (const [name, kind, brand] of [
      ["StyleRef", "ref", "refIdentity"],
      ["StyleKeyframes", "keyframes", "keyframesIdentity"],
      ["StyleViewTransition", "viewTransition", "viewTransitionIdentity"],
    ] as const) {
      const symbol = this.#checker.getMemberInModuleExports(module, name);
      if (!symbol) {
        continue;
      }
      const declared = this.#checker.getDeclaredTypeOfSymbol(symbol);
      const property = this.#checker
        .getPropertiesOfType(declared)
        .find((property) => property.name.startsWith(`__@${brand}@`));
      if (property) {
        this.#brands.set(property.name, kind);
      }
    }
  }

  /**
   * Tests whether a type alias is declared by a registered macro module. Such a type describes a
   * macro use or declaration, not an author's widened asset.
   *
   * @param type - Type from this decoder's checker snapshot.
   * @returns Whether the alias symbol's module was inspected by {@link register}.
   */
  #declaredByMacro(type: Type): boolean {
    const module = type.getAliasSymbol()?.getParent();
    return !!module && this.#modules.has(module.id);
  }

  /**
   * Resolves a checker type to finite candidates from a single asset family.
   *
   * @param type - Type of a probed expression from the current checker snapshot.
   * @param recognitionOnly - True skips checks for widened asset types when only macro recognition is needed.
   * @returns Finite candidates, a confirmed non-asset, an object type that may hold a widened
   * asset, or an unverified value whose declarations cannot be determined.
   */
  decode(type: Type, recognitionOnly: boolean): DecodedType {
    if (type.flags & (TypeFlags.Any | TypeFlags.Unknown)) {
      return { kind: "unverified" };
    }
    const constituents = type.isUnionType() ? type.getTypes() : [type];
    const values: IdentityId[] = [];
    let family: IdentityId["kind"] | undefined;
    let recognized = false;
    let valid = true;
    let finite = true;
    for (const constituent of constituents) {
      if (constituent.flags & primitiveTypeFlags) {
        valid = false;
        continue;
      }
      let kind: IdentityId["kind"] | undefined;
      let identityType: Type | undefined;
      const style = this.#property(constituent, "data-__stillwindStyleIdentity");
      if (style) {
        kind = "style";
        identityType = this.#checker.getTypeOfSymbol(style);
      } else {
        for (const brand of this.#allProperties(constituent)) {
          const candidateKind = this.#brands.get(brand.name);
          if (!candidateKind) {
            continue;
          }
          const callable =
            candidateKind === "viewTransition" ? this.#property(constituent, "for") : undefined;
          const method = callable && this.#checker.getTypeOfSymbol(callable);
          const declaration =
            candidateKind === "viewTransition"
              ? method && this.#checker.getSignaturesOfType(method, SignatureKind.Call).length > 0
              : this.#checker.getSignaturesOfType(constituent, SignatureKind.Call).length > 0;
          if (!declaration) {
            continue;
          }
          kind = candidateKind;
          identityType = this.#checker.getTypeOfSymbol(brand);
          if (kind === "ref" && identityType) {
            const ref = this.#property(identityType, "ref");
            identityType = ref && this.#checker.getTypeOfSymbol(ref);
          }
          break;
        }
      }
      const name = (constituent.getAliasSymbol() ?? constituent.getSymbol())?.name;
      recognized ||=
        !!kind || (!!name && /^Style(?:Result|Ref|Keyframes|ViewTransition)$/.test(name));
      if (!kind) {
        valid = false;
        continue;
      }
      if (family && family !== kind) {
        valid = false;
        continue;
      }
      family = kind;
      const candidates = knownIdentities(identityType, this.#identities, kind);
      if (candidates === "mismatch") {
        valid = false;
      } else if (candidates === "unknown") {
        finite = false;
      } else {
        values.push(...candidates);
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
    if (!valid || !family || values.length === 0) {
      if (recognized) {
        return {
          kind: "unverified",
        };
      }
      const widened =
        !recognitionOnly &&
        constituents.some(
          (constituent) =>
            !(constituent.flags & primitiveTypeFlags) &&
            !this.#declaredByMacro(constituent) &&
            (this.#producers.length === 0 ||
              this.#producers.some((producer) =>
                this.#checker.isTypeAssignableTo(producer, constituent),
              )),
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
        value: symbolic(family, values),
      },
    };
  }
}
