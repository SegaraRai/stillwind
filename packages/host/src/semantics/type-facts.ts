import { createHash } from "node:crypto";
import { existsSync } from "node:fs";
import path from "node:path";

import type {
  NativeAssetRecord,
  NativeExportedValue,
  NativeIdentity,
  NativeSymbolicValue,
  NativeTypeFact,
} from "@stillwind/native";
import type { Node, SourceFile } from "typescript-7/unstable/ast";
import {
  isCallExpression,
  isElementAccessExpression,
  isExportDeclaration,
  isExpression,
  isIdentifier,
  isImportDeclaration,
  isPropertyAccessExpression,
  isSourceFile,
  isStringLiteral,
} from "typescript-7/unstable/ast/is";
import {
  API,
  SignatureKind,
  Snapshot,
  TypeFlags,
  type Checker,
  type Diagnostic,
  type Type,
} from "typescript-7/unstable/sync";

import {
  TypeFactsError,
  type AvailableFacts,
  type TypeFactSource,
  type TypeFactsBackend,
  type UnverifiedRange,
} from "./backend.ts";
import { CheckerSource, checkerId } from "./checker-source.ts";

/**
 * Native declaration identity containing its asset kind, declaring module, original byte range, and
 * binding name.
 */
type IdentityId = NativeIdentity;

/**
 * Native declaration record carrying an identity, metadata, and the original-byte range of its
 * producer call.
 */
export type AssetRecord = NativeAssetRecord;

/** Semantic value attached to an original UTF-8 expression range for native lowering. */
export type TypeFact = NativeTypeFact;

/**
 * Normalizes filesystem names used as keys in the virtual checker overlay.
 *
 * @param fileName - Absolute or relative filesystem path.
 * @returns An absolute path with forward slashes, lowercased on Windows to match case-insensitive
 * filesystem lookups.
 */
const normalize = (fileName: string) => {
  const name = path.resolve(fileName).replaceAll("\\", "/");
  return process.platform === "win32" ? name.toLowerCase() : name;
};

/**
 * Traverses a checker AST in preorder.
 *
 * @param node - Root visited before its descendants.
 * @param callback - Operation called once for the root and each child exposed by `forEachChild`.
 * Thrown errors stop traversal.
 */
function visit(node: Node, callback: (node: Node) => void) {
  callback(node);
  node.forEachChild((child) => visit(child, callback));
}

/**
 * Decodes a string-literal identity type or union using the current declaration table.
 *
 * @param type - Checker type of an identity property, or undefined when the property is
 * unavailable.
 * @param identities - Serialized keys mapped to known native declaration identities.
 * @param kind - Asset family that every candidate must belong to.
 * @returns Candidates in type-member order, or null if the type is absent, nonliteral, unknown to
 * the table, or contains a different asset family.
 */
function knownIdentities(
  type: Type | undefined,
  identities: ReadonlyMap<string, IdentityId>,
  kind: IdentityId["kind"],
): IdentityId[] | null {
  if (!type) {
    return null;
  }
  const types = type.isUnionType() ? type.getTypes() : [type];
  const result: IdentityId[] = [];
  for (const candidate of types) {
    if (!candidate.isStringLiteralType()) {
      return null;
    }
    const identity = identities.get(candidate.value);
    if (identity?.kind !== kind) {
      return null;
    }
    result.push(identity);
  }
  return result;
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

/**
 * Decodes checker types into native declaration candidates using brand properties discovered from
 * producer modules.
 */
class AssetTypeDecoder {
  /**
   * Checker for the current project snapshot, used to inspect exported types and their properties.
   */
  readonly #checker: Checker;
  /**
   * All declaration keys registered in the current virtual project, mapped to original native
   * identities.
   */
  readonly #identities: ReadonlyMap<string, IdentityId>;
  /** Checker symbol IDs of modules whose exported asset brands have already been inspected. */
  readonly #modules = new Set<number>();
  /** Internal names of discovered unique-symbol brand properties mapped to their asset families. */
  readonly #brands = new Map<string, IdentityId["kind"]>();

  /**
   * Creates a decoder for one checker snapshot and declaration table.
   *
   * @param checker - Checker owning all types supplied to this decoder.
   * @param identities - Serialized identity keys inserted into the project's producer calls.
   */
  constructor(checker: Checker, identities: ReadonlyMap<string, IdentityId>) {
    this.#checker = checker;
    this.#identities = identities;
  }

  /**
   * Discovers ref, keyframe, and view-transition brands in a producer type's declaring module.
   *
   * @param type - Producer call type from this decoder's checker snapshot. Missing alias modules
   * and already inspected modules are skipped. The discovered property names are retained for later
   * decoding.
   */
  register(type: Type): void {
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
   * Resolves a checker type to finite candidates from a single asset family.
   *
   * @param type - Type of a probed expression from the current checker snapshot.
   * @returns A native value when every union member carries known declarations from the same
   * family.
   */
  decode(type: Type): {
    /** Finite declarations, or null when a member lacks a valid identity or the families differ. */
    value: NativeExportedValue | null;
    /** Whether branding or a known asset type name was detected, even when decoding failed. */
    recognized: boolean;
  } {
    const constituents = type.isUnionType() ? type.getTypes() : [type];
    const values: IdentityId[] = [];
    let family: IdentityId["kind"] | undefined;
    let recognized = false;
    let valid = true;
    for (const constituent of constituents) {
      let kind: IdentityId["kind"] | undefined;
      let identityType: Type | undefined;
      const style = this.#checker.getPropertyOfType(constituent, "data-__stillwindStyleIdentity");
      if (style) {
        kind = "style";
        identityType = this.#checker.getTypeOfSymbol(style);
      } else {
        for (const brand of this.#checker.getPropertiesOfType(constituent)) {
          const candidateKind = this.#brands.get(brand.name);
          if (!candidateKind) {
            continue;
          }
          const callable =
            candidateKind === "viewTransition"
              ? this.#checker.getPropertyOfType(constituent, "for")
              : undefined;
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
            const ref = this.#checker.getPropertyOfType(identityType, "ref");
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
      const candidates = knownIdentities(identityType, this.#identities, kind);
      if (!candidates || (family && family !== kind)) {
        valid = false;
      } else {
        family = kind;
        values.push(...candidates);
      }
    }
    if (!valid || !family || values.length === 0) {
      return { value: null, recognized };
    }
    const kinds = {
      style: "styles",
      ref: "refs",
      keyframes: "keyframes",
      viewTransition: "viewTransitions",
    } as const;
    return {
      value: {
        kind: kinds[family],
        value: symbolic(family, values),
      },
      recognized,
    };
  }
}

/**
 * Virtual project files, checker range mappings, and declaration identities prepared for one
 * collection request.
 */
interface PreparedProject {
  /**
   * Normalized absolute filenames mapped to virtual text, including checker sources, helpers,
   * globals, and project configuration.
   */
  virtualFiles: Map<string, string>;
  /**
   * Checker buffers in source and buffer order, retaining the correspondence to original probes.
   */
  mappings: CheckerSource[];
  /**
   * Serialized identity keys collected from all checker buffers, mapped to their native
   * declarations.
   */
  identities: Map<string, IdentityId>;
}

/**
 * Cumulative checker lifecycle and filesystem counters for one service. They include recovery
 * attempts and are not reset between collections.
 */
export interface TypeScriptFactsStats {
  /** Number of API construction attempts, including starts after invalidation or query failure. */
  starts: number;
  /**
   * Number of snapshot updates that returned a snapshot, including updates followed by a query
   * failure.
   */
  snapshots: number;
  /** Number of snapshot updates made while a previous snapshot was retained. */
  incrementalSnapshots: number;
  /** Number of recovery restarts attempted after a failure other than `TypeFactsError`. */
  restarts: number;
  /** Number of checker read callbacks answered with text from the virtual overlay. */
  overlayReads: number;
  /**
   * Number of checker read callbacks delegated to the filesystem because the overlay had no text. A
   * request does not imply a successful disk read.
   */
  diskReadRequests: number;
}

/** TypeScript settings inherited by the generated project used for semantic queries. */
export type TypeScriptProject =
  | {
      /** Selects settings inherited from a consumer tsconfig. */
      readonly kind: "configured";
      /** Absolute path to the consumer's tsconfig. */
      readonly configPath: string;
    }
  | {
      /** Selects Stillwind's defaults without a consumer tsconfig. */
      readonly kind: "isolated";
    };

/** Fixed filesystem and TypeScript settings shared by collections in one checker service. */
export interface TypeFactsEnvironment {
  /** Absolute directory used for checker resolution and generated configuration paths. */
  readonly root: string;
  /** Settings inherited by the checker. Isolated projects use Stillwind's compiler defaults. */
  readonly project: TypeScriptProject;
}

/**
 * Selects the TypeScript environment before starting a checker.
 *
 * @param root - Filesystem project directory, resolved to an absolute path.
 * @param tsconfig - Absolute or root-relative config path. Omission selects an existing root
 * `tsconfig.json`. False selects isolated defaults even when that file exists.
 * @returns Fixed settings without evaluating configuration or starting the checker.
 * @throws If an explicitly selected path is empty or its configuration file does not exist.
 */
export function resolveTypeFactsEnvironment(
  root: string,
  tsconfig?: string | false,
): TypeFactsEnvironment {
  root = path.resolve(root);
  if (typeof tsconfig === "string" && !tsconfig.trim()) {
    throw new Error("TypeScript configuration path must not be empty");
  }
  const configPath = path.resolve(root, typeof tsconfig === "string" ? tsconfig : "tsconfig.json");
  if (typeof tsconfig === "string" && !existsSync(configPath)) {
    throw new Error(`TypeScript configuration does not exist: ${configPath}`);
  }
  return {
    root,
    project:
      tsconfig !== false && existsSync(configPath)
        ? {
            kind: "configured",
            configPath,
          }
        : {
            kind: "isolated",
          },
  };
}

/**
 * File contents captured for one collection. Ambient declarations override overlays at matching
 * paths. Prepared checker sources and the generated configuration override both maps.
 */
export interface TypeFactsInputs {
  /** Absolute paths to replacement text. Files participate only when the checker references them. */
  overlayFiles?: ReadonlyMap<string, string>;
  /** Absolute declaration paths to text. Every supplied declaration becomes a project root file. */
  ambientDeclarations?: ReadonlyMap<string, string>;
}

/**
 * Builds checker sources and an in-memory project configuration for semantic queries.
 *
 * @param sources - Prepared sources with validated mappings, original probes, and dependency
 * resolutions.
 * @param assetsByModuleId - Native declaration records keyed by compiler module identity.
 * @param shadowTsconfigPath - Absolute filename of the generated in-memory TypeScript project.
 * @param project - Consumer configuration to inherit, or isolated compiler defaults.
 * @param inputs - Captured file replacements and declarations required by this collection.
 * @param configuredDeclarations - Declaration files selected by the consumer's tsconfig.
 * @returns Virtual text and range mappings without writing any files to disk.
 * @throws If checker edits or projection maps are invalid, or an original probe has no
 * checker-buffer correspondence.
 */
function prepareProject(
  sources: readonly TypeFactSource[],
  assetsByModuleId: ReadonlyMap<string, readonly AssetRecord[]>,
  shadowTsconfigPath: string,
  project: TypeScriptProject,
  inputs: TypeFactsInputs,
  configuredDeclarations: readonly string[],
): PreparedProject {
  const virtualFiles = new Map(
    [...(inputs.overlayFiles ?? []), ...(inputs.ambientDeclarations ?? [])].map(([name, text]) => [
      normalize(name),
      text,
    ]),
  );
  const mappings: CheckerSource[] = [];
  const allIdentities = new Map<string, IdentityId>();
  const globals = new Set<string>();
  const ids = new Map(sources.map((source) => [source.resolverId, checkerId(source)]));
  for (const source of sources) {
    const declarations = source.kind === "projected" && source.projection.checkerGlobals;
    if (declarations) {
      const fingerprint = createHash("sha256").update(declarations).digest("hex").slice(0, 16);
      const id = normalize(
        path.join(path.dirname(shadowTsconfigPath), `.stillwind-globals-${fingerprint}.d.ts`),
      );
      virtualFiles.set(id, declarations);
      globals.add(id);
    }
    const syntax = source.kind === "direct" ? source.syntax : source.projection.syntax;
    const buffers = source.checkerBuffers ?? [
      {
        source: source.kind === "direct" ? source.document.code : source.projection.code,
        language:
          syntax.language === "tsx" || syntax.language === "jsx"
            ? ("tsx" as const)
            : ("ts" as const),
      },
    ];
    for (const [index, buffer] of buffers.entries()) {
      const shadow = new CheckerSource(
        source,
        buffer,
        index,
        assetsByModuleId.get(source.moduleId) ?? [],
        ids,
      );
      virtualFiles.set(normalize(shadow.id), shadow.text);
      if (shadow.helperModule) {
        virtualFiles.set(normalize(shadow.helperModule.id), shadow.helperModule.text);
      }
      mappings.push(shadow);
      for (const [key, identity] of shadow.identities) {
        allIdentities.set(key, identity);
      }
    }
    for (const probe of source.probes) {
      if (
        !mappings.some(
          (mapping) =>
            mapping.source === source &&
            [...mapping.probes.values()].some(
              (candidate) => candidate.start === probe.start && candidate.end === probe.end,
            ),
        )
      ) {
        throw new TypeFactsError(
          source.moduleId,
          probe.start,
          probe.end,
          "checker",
          "Cannot map this expression to a checker source",
        );
      }
    }
  }
  virtualFiles.set(
    normalize(shadowTsconfigPath),
    JSON.stringify({
      ...(project.kind === "configured"
        ? {
            extends: project.configPath,
          }
        : {}),
      compilerOptions: {
        allowJs: true,
        checkJs: true,
        ...(project.kind === "isolated"
          ? {
              jsx: "preserve",
              module: "esnext",
              moduleResolution: "bundler",
              strict: true,
              target: "esnext",
              types: [],
            }
          : {}),
        noEmit: true,
        skipLibCheck: true,
        allowImportingTsExtensions: true,
      },
      files: [
        ...configuredDeclarations,
        ...[...(inputs.ambientDeclarations?.keys() ?? [])].map(normalize),
        ...globals,
        ...mappings.map((mapping) => mapping.id),
        ...new Set(sources.flatMap((source) => source.checkerTypes ?? [])),
      ],
      include: [],
      exclude: [],
    }),
  );
  return {
    virtualFiles,
    mappings,
    identities: allIdentities,
  };
}

/**
 * Owns a synchronous TypeScript checker API and incrementally updated virtual project. Reuse it
 * across collections for one root and close it when its caller releases the project.
 */
export class TypeScriptFactsService implements TypeFactsBackend {
  /** Paths observed by disk reads and existence checks, including missing paths and overlay checks. */
  readonly dependencies = new Set<string>();
  /**
   * Live cumulative counters updated by this service. Consumers needing a stable report must copy
   * their values.
   */
  readonly stats: TypeScriptFactsStats = {
    starts: 0,
    snapshots: 0,
    incrementalSnapshots: 0,
    restarts: 0,
    overlayReads: 0,
    diskReadRequests: 0,
  };

  /** Copy of the selected TypeScript settings, fixed for this service's lifetime. */
  readonly #project: TypeScriptProject;
  /** Filesystem root supplied as the checker API working directory. */
  readonly #projectRoot: string;
  /**
   * Absolute filename of the in-memory `.stillwind-tsconfig.json`. The service does not write it to
   * disk.
   */
  readonly #shadowTsconfigPath: string;
  /** Active checker API instance, replaced after a recoverable collection failure. */
  #api: API;
  /**
   * Whether close has been requested. Subsequent collections reject and repeated close calls do
   * nothing.
   */
  #closed = false;
  /**
   * Latest retained checker snapshot, also retained after query failures until the next update,
   * restart, or close.
   */
  #snapshot: Snapshot | undefined;
  /**
   * Current normalized filename overlay read dynamically by the checker API filesystem callbacks.
   */
  #virtualFiles = new Map<string, string>();

  /**
   * Starts a checker API for a project root without creating its first snapshot.
   *
   * @param environment - Absolute filesystem root and selected TypeScript settings. The service
   * copies these settings and uses them for its lifetime. Create a new service to change them.
   * @throws If the checker API cannot start.
   */
  constructor(environment: TypeFactsEnvironment) {
    this.#project = { ...environment.project };
    this.#projectRoot = environment.root;
    this.#shadowTsconfigPath = path.join(environment.root, ".stillwind-tsconfig.json");
    this.#api = this.#startApi();
  }

  /**
   * Collects finite declaration candidates for original-coordinate probes.
   *
   * Each call replaces the virtual project contents and updates its snapshot. Known mapping,
   * assignment, and identity contract errors propagate as `TypeFactsError`. Other query failures
   * trigger one clean API restart and retry. The service filters diagnostics to those associated
   * with Stillwind declaration contracts.
   *
   * @param sources - Prepared sources and original-byte probes, with validated projection
   * correspondences where needed.
   * @param assetsByModuleId - Declaration records keyed by compiler module identity.
   * @param inputs - Complete replacement overlay and ambient declarations for this collection.
   * Omission removes the previous collection's supplied files. The service copies their contents
   * before updating its snapshot and does not modify the supplied maps.
   * @returns Facts keyed by compiler module identity. Facts for the same original range are
   * deduplicated across buffers. Incompatible values reject. Known types that do not represent
   * Stillwind assets and unresolved recognition-only probes produce no fact.
   * @throws If the service is closed, preparation or checker contracts fail, or the API fails. A
   * failed query retry throws an AggregateError containing both query failures.
   */
  collect(
    sources: readonly TypeFactSource[],
    assetsByModuleId: ReadonlyMap<string, readonly AssetRecord[]>,
    inputs: TypeFactsInputs = {},
  ): Map<string, TypeFact[]> {
    return this.#collect(sources, assetsByModuleId, inputs, false).facts;
  }

  /**
   * Updates the checker snapshot and retains facts for queries that can be verified.
   *
   * @param sources - Prepared sources and original UTF-8 probes with validated checker correspondences.
   * @param assetsByModuleId - Declaration records grouped by compiler module identity.
   * @param inputs - Complete overlays and ambient declarations for this call. Omission removes
   * files supplied by the previous call.
   * @returns Verified facts and unavailable original ranges. Unavailable queries do not discard
   * facts collected for other expressions.
   * @throws If the service is closed, project preparation fails, or checker restart and retry fail.
   */
  collectAvailable(
    sources: readonly TypeFactSource[],
    assetsByModuleId: ReadonlyMap<string, readonly AssetRecord[]>,
    inputs: TypeFactsInputs = {},
  ): AvailableFacts {
    return this.#collect(sources, assetsByModuleId, inputs, true);
  }

  /**
   * Resolves authored imports and exports using module symbols in the selected TypeScript settings.
   *
   * @param sources - Prepared sources whose checker buffers contain the authored module requests.
   * Probes are ignored during resolution.
   * @param assetsByModuleId - Declaration records used to expose generated helper types.
   * @param inputs - Complete overlays and ambient declarations for this snapshot.
   * @returns Request-to-identity maps grouped by source resolver identity. Prepared checker paths
   * map back to resolver identities. Other resolutions retain checker filenames, including declarations.
   * Requests without a matching module symbol are omitted.
   * @throws If the service is closed, project preparation fails, or checker resolution fails.
   */
  resolveModules(
    sources: readonly TypeFactSource[],
    assetsByModuleId: ReadonlyMap<string, readonly AssetRecord[]>,
    inputs: TypeFactsInputs = {},
  ): Map<string, Map<string, string>> {
    if (this.#closed) {
      throw new Error("TypeScript facts service is closed");
    }
    const prepared = prepareProject(
      sources.map((source) => ({ ...source, probes: [] })),
      assetsByModuleId,
      this.#shadowTsconfigPath,
      this.#project,
      inputs,
      this.#configuredDeclarations(inputs),
    );
    const project = this.#updateProject(prepared);
    const result = new Map<string, Map<string, string>>();
    for (const mapping of prepared.mappings) {
      const file = project.program.getSourceFile(mapping.id);
      if (!file) {
        continue;
      }
      const resolutions = result.get(mapping.source.resolverId) ?? new Map<string, string>();
      result.set(mapping.source.resolverId, resolutions);
      visit(file, (node) => {
        if (
          !(isImportDeclaration(node) || isExportDeclaration(node)) ||
          !node.moduleSpecifier ||
          !isStringLiteral(node.moduleSpecifier)
        ) {
          return;
        }
        const symbol = project.checker.getSymbolAtLocation(node.moduleSpecifier);
        const declaration = symbol?.declarations.find((declaration) => {
          const node = declaration.resolve(project);
          return (
            node !== undefined && (isSourceFile(node) || node.getSourceFile().isDeclarationFile)
          );
        });
        if (declaration) {
          const fileName = declaration.resolve(project)!.getSourceFile().fileName;
          const original = sources.find(
            (source) => normalize(checkerId(source)) === normalize(fileName),
          );
          resolutions.set(node.moduleSpecifier.text, original?.resolverId ?? fileName);
        }
      });
    }
    return result;
  }

  /**
   * Prepares one request and retries an unexpected checker failure once with a clean API.
   * @param sources - Prepared sources and original UTF-8 probes.
   * @param assetsByModuleId - Declaration records grouped by compiler identity.
   * @param inputs - Complete overlays and ambient declarations for the request.
   * @param available - Whether unavailable facts are recorded instead of rejecting the request.
   * @returns Collected facts and unavailable ranges from the successful attempt.
   * @throws If preparation fails, a TypeFactsError occurs, restart fails, or both query attempts fail.
   */
  #collect(
    sources: readonly TypeFactSource[],
    assetsByModuleId: ReadonlyMap<string, readonly AssetRecord[]>,
    inputs: TypeFactsInputs,
    available: boolean,
  ): AvailableFacts {
    if (this.#closed) {
      throw new Error("TypeScript facts service is closed");
    }
    const prepared = prepareProject(
      sources,
      assetsByModuleId,
      this.#shadowTsconfigPath,
      this.#project,
      inputs,
      this.#configuredDeclarations(inputs),
    );
    try {
      return this.#collectPrepared(prepared, available);
    } catch (firstError) {
      if (firstError instanceof TypeFactsError) {
        throw firstError;
      }
      this.stats.restarts += 1;
      this.#restartApi();
      try {
        return this.#collectPrepared(prepared, available);
      } catch (retryError) {
        throw new AggregateError(
          [firstError, retryError],
          `TypeScript 7 checker failed before and after a clean restart: ${String(retryError)}`,
        );
      }
    }
  }

  /**
   * Marks the service closed, disposes its retained snapshot, and closes the checker API. Repeated
   * calls return without further work.
   *
   * @throws If snapshot disposal or API cleanup fails.
   */
  close(): void {
    if (this.#closed) {
      return;
    }
    this.#closed = true;
    this.#snapshot?.dispose();
    this.#snapshot = undefined;
    this.#api.close();
  }

  /**
   * Discards cached disk contents and the retained snapshot while keeping the selected environment.
   * The next collection opens a new project. Calls after close have no effect.
   *
   * @throws If checker cleanup or restart fails.
   */
  invalidate(): void {
    if (!this.#closed) {
      this.#restartApi();
    }
  }

  /**
   * Reads the consumer's declaration roots using the captured configuration overlays. Ordinary
   * source files selected by the consumer remain outside the generated project's root files.
   *
   * @param inputs - File replacements used while parsing the selected configuration.
   * @returns Declaration filenames selected by TypeScript, or an empty list for isolated settings.
   * Restores the overlay used by the retained snapshot before returning or throwing.
   * @throws If TypeScript cannot parse the selected configuration or its inherited files.
   */
  #configuredDeclarations(inputs: TypeFactsInputs): readonly string[] {
    if (this.#project.kind === "isolated") {
      return [];
    }
    const previous = this.#virtualFiles;
    this.#virtualFiles = new Map(
      [...(inputs.overlayFiles ?? []), ...(inputs.ambientDeclarations ?? [])].map(
        ([name, text]) => [normalize(name), text],
      ),
    );
    try {
      return this.#api
        .parseConfigFile(this.#project.configPath)
        .fileNames.filter((file) => /\.d\.[cm]?ts$/i.test(file));
    } finally {
      this.#virtualFiles = previous;
    }
  }

  /**
   * Starts a checker API whose filesystem callbacks consult the current virtual overlay.
   *
   * @returns A new API using the project root, with timing collection enabled. Increments starts
   * before construction. Read callbacks update the filesystem counters.
   * @throws If API construction fails.
   */
  #startApi(): API {
    this.stats.starts += 1;
    return new API({
      cwd: this.#projectRoot,
      collectTiming: true,
      fs: {
        directoryExists: (directoryName) => {
          const prefix = normalize(directoryName).replace(/\/$/, "") + "/";
          return [...this.#virtualFiles.keys()].some((name) => name.startsWith(prefix))
            ? true
            : undefined;
        },
        fileExists: (fileName) => {
          this.dependencies.add(fileName);
          return this.#virtualFiles.has(normalize(fileName)) ? true : undefined;
        },
        readFile: (fileName) => {
          const overlay = this.#virtualFiles.get(normalize(fileName));
          if (overlay === undefined) {
            this.dependencies.add(fileName);
            this.stats.diskReadRequests += 1;
          } else {
            this.stats.overlayReads += 1;
          }
          return overlay;
        },
      },
    });
  }

  /**
   * Disposes the previous snapshot and API, then replaces the API without clearing virtual files or
   * counters.
   *
   * @throws If cleanup or creation of the replacement API fails.
   */
  #restartApi(): void {
    this.#snapshot?.dispose();
    this.#snapshot = undefined;
    this.#api.close();
    this.#api = this.#startApi();
  }

  /**
   * Replaces virtual files and retains the new snapshot after releasing the previous one.
   * @param prepared - Complete normalized virtual project, retained as the new filesystem overlay.
   * @returns The checker project from the new snapshot. Snapshot counters are incremented after update.
   * @throws If snapshot update or disposal fails, or the generated project cannot be opened.
   */
  #updateProject(prepared: PreparedProject) {
    const changed: string[] = [];
    const created: string[] = [];
    const deleted: string[] = [];
    for (const [fileName, contents] of prepared.virtualFiles) {
      if (!this.#virtualFiles.has(fileName)) {
        (existsSync(fileName) ? changed : created).push(fileName);
      } else if (this.#virtualFiles.get(fileName) !== contents) {
        changed.push(fileName);
      }
    }
    for (const fileName of this.#virtualFiles.keys()) {
      if (!prepared.virtualFiles.has(fileName)) {
        (existsSync(fileName) ? changed : deleted).push(fileName);
      }
    }
    this.#virtualFiles = prepared.virtualFiles;

    const next = this.#snapshot
      ? this.#api.updateSnapshot({
          fileChanges: {
            changed,
            created,
            deleted,
          },
        })
      : this.#api.updateSnapshot({
          openProjects: [this.#shadowTsconfigPath],
        });
    this.stats.snapshots += 1;
    if (this.#snapshot) {
      this.stats.incrementalSnapshots += 1;
    }
    this.#snapshot?.dispose();
    this.#snapshot = next;
    const project = next.getProject(this.#shadowTsconfigPath);
    if (!project) {
      throw new Error("TypeScript 7 could not open the Stillwind shadow project");
    }
    return project;
  }

  /**
   * Updates the snapshot and collects facts from the prepared checker buffers.
   * @param prepared - Virtual files, original-coordinate mappings, and serialized declaration identities.
   * @param available - Whether contract failures become unverified ranges where recovery is possible.
   * @returns Deduplicated facts by compiler identity and unverified original UTF-8 ranges.
   * @throws If snapshot update fails, a checker file is missing, facts conflict, or strict collection fails.
   */
  #collectPrepared(prepared: PreparedProject, available: boolean): AvailableFacts {
    const project = this.#updateProject(prepared);
    const unverified: UnverifiedRange[] = [];
    const result = new Map<string, TypeFact[]>();
    const decoder = new AssetTypeDecoder(project.checker, prepared.identities);
    const producers: Node[] = [];
    for (const mapping of prepared.mappings) {
      const file = project.program.getSourceFile(mapping.id);
      if (!file) {
        continue;
      }
      visit(file, (node) => {
        if (isCallExpression(node) && mapping.producers.has(`${node.getStart(file)}:${node.end}`)) {
          producers.push(node);
        }
      });
    }
    for (const type of project.checker.getTypeAtLocation(producers)) {
      if (type) {
        decoder.register(type);
      }
    }
    for (const mapping of prepared.mappings) {
      const sourceFile = project.program.getSourceFile(mapping.id);
      if (!sourceFile) {
        throw new Error(`TypeScript 7 omitted ${mapping.id}`);
      }
      const diagnostics = [
        ...project.program.getSyntacticDiagnostics(mapping.id),
        ...project.program.getSemanticDiagnostics(mapping.id),
      ];
      const diagnostic = diagnostics.find((d) =>
        /stillwind\/v1:|StyleResult|StyleRef|StyleKeyframes|StyleViewTransition|__stillwindDeclarationIdentity|data-__stillwindStyleIdentity/.test(
          diagnosticText(d),
        ),
      );
      if (diagnostic) {
        const start = mapping.originalOffset(diagnostic.pos) ?? undefined;
        const end = mapping.originalOffset(diagnostic.end, true) ?? undefined;
        const message = diagnosticText(diagnostic).replace(
          /stillwind\/v1:[A-Za-z0-9_-]+/g,
          "declaration identity",
        );
        const error = new TypeFactsError(
          mapping.source.moduleId,
          start,
          end,
          "assignment",
          `TS${diagnostic.code}: ${message} Preserve declaration candidates with an explicit union or a generic parameter.`,
        );
        if (!available) {
          throw error;
        }
        unverified.push({ moduleId: error.moduleId, start, end, reason: error.message });
      }
      const facts = factsForSource(
        project.checker,
        sourceFile,
        mapping,
        decoder,
        available ? unverified : undefined,
      );
      const previous = result.get(mapping.source.moduleId) ?? [];
      for (const fact of facts) {
        const existing = previous.find(
          (value) => value.start === fact.start && value.end === fact.end,
        );
        if (existing && JSON.stringify(existing.value) !== JSON.stringify(fact.value)) {
          throw new TypeFactsError(
            mapping.source.moduleId,
            fact.start,
            fact.end,
            "checker",
            "Ambiguous checker results for this original expression",
          );
        }
        if (!existing) {
          previous.push(fact);
        }
      }
      result.set(mapping.source.moduleId, previous);
    }
    return {
      facts: result,
      unverified,
    };
  }
}

/**
 * Collects facts with a temporary checker service and closes it afterward.
 *
 * @param sources - Prepared sources and their original-byte semantic probes.
 * @param assetsByModuleId - Native declarations keyed by compiler module identity.
 * @param environment - Fixed filesystem root and TypeScript settings for the temporary checker.
 * @param inputs - File replacements and declarations required by this collection.
 * @returns Facts keyed by compiler module identity. The temporary checker is closed before returning.
 * @throws If service startup, fact collection, or cleanup fails.
 */
export function collectTypeScriptFacts(
  sources: readonly TypeFactSource[],
  assetsByModuleId: ReadonlyMap<string, readonly AssetRecord[]>,
  environment: TypeFactsEnvironment,
  inputs: TypeFactsInputs = {},
): Map<string, TypeFact[]> {
  const service = new TypeScriptFactsService(environment);
  try {
    return service.collect(sources, assetsByModuleId, inputs);
  } finally {
    service.close();
  }
}

/**
 * Flattens a checker diagnostic and its nested message chain for matching and display.
 *
 * @param diagnostic - Checker diagnostic with optional nested messages.
 * @returns Diagnostic text followed recursively by child messages, separated by newlines.
 */
function diagnosticText(diagnostic: Diagnostic): string {
  return [diagnostic.text, ...(diagnostic.messageChain ?? []).map(diagnosticText)].join("\n");
}

/**
 * Decodes the types of checker expressions matched to original semantic probes.
 *
 * @param checker - Checker owning the supplied source file and decoder types.
 * @param sourceFile - Final checker AST for the mapped buffer.
 * @param mapping - Original probes indexed by exact checker AST start and end positions.
 * @param decoder - Decoder with asset brands already registered from producer types.
 * @param unverified - Optional array to append unavailable original ranges to. Supplying it keeps
 * other facts usable when a probe lacks an expression or a finite declaration type.
 * @returns Facts in AST traversal order with original byte ranges. Known types that do not
 * represent Stillwind assets produce no fact. Strict queries skip unresolved recognition-only
 * probes. Partial queries record their unavailable ranges instead.
 * @throws Without unverified, if a probe has no expression node or a required query has a missing,
 * unresolved, or recognized-but-nonfinite declaration type. Checker query failures can also propagate.
 */
function factsForSource(
  checker: Checker,
  sourceFile: SourceFile,
  mapping: CheckerSource,
  decoder: AssetTypeDecoder,
  unverified?: UnverifiedRange[],
): TypeFact[] {
  const expressions: Node[] = [];
  const matched = new Set<string>();
  visit(sourceFile, (node) => {
    if (!isExpression(node)) {
      return;
    }
    const key = `${node.getStart(sourceFile)}:${node.end}`;
    if (mapping.probes.has(key)) {
      expressions.push(node);
      matched.add(key);
    }
  });
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

  const types = checker.getTypeAtLocation(expressions);
  const facts: TypeFact[] = [];
  for (const [index, expression] of expressions.entries()) {
    const probe = mapping.probes.get(`${expression.getStart(sourceFile)}:${expression.end}`)!;
    const type = types[index];
    const decoded = type && decoder.decode(type);
    const value = decoded?.value;
    if (!value) {
      if (probe.recognitionOnly && !unverified) {
        continue;
      }
      if (
        !type ||
        (type.flags & (TypeFlags.Any | TypeFlags.Unknown) &&
          (isIdentifier(expression) ||
            isCallExpression(expression) ||
            isPropertyAccessExpression(expression) ||
            isElementAccessExpression(expression))) ||
        decoded?.recognized
      ) {
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
      }
      continue;
    }
    facts.push({
      ...probe,
      value,
    });
  }

  return facts;
}
