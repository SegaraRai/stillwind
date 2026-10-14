import type {
  NativeAssetRecord,
  NativeExportedValue,
  NativeIdentity,
  NativeTypeFact,
} from "@stillwind/native";
import { minimatch } from "minimatch";
import * as path from "pathe";
import ts from "typescript-6";

import {
  TypeFactsError,
  type AvailableFacts,
  type SemanticInputs,
  type TypeFactsBackend,
  type TypeFactSource,
  type UnverifiedRange,
} from "./backend.ts";
import { checkerId, CheckerSource } from "./checker-source.ts";

/** Supplies complete virtual files and TypeScript settings for a browser checker. */
export interface BrowserCheckerOptions {
  /** Absolute virtual project path. Paths use forward slashes. */
  root: string;
  /** Files include package metadata, declarations, standard libraries, and source documents. */
  files: ReadonlyMap<string, string>;
  /** Selected virtual tsconfig path. False uses isolated defaults. */
  tsconfig?: string | false;
}

/** Answers Stillwind probes using the JavaScript TypeScript checker without filesystem access. */
export class BrowserTypeFactsService implements TypeFactsBackend {
  /** Root, settings, and inventory read again when preparing each request. */
  readonly #options: BrowserCheckerOptions;
  /** Language service reused while snapshots and project versions change. */
  #service?: ts.LanguageService;
  /** Current captured inventory with overlays, checker buffers, and helper declarations applied. */
  #files = new Map<string, string>();
  /** Per-file change counters exposed as language-service script versions. */
  #versions = new Map<string, number>();
  /** Effective TypeScript options from the virtual tsconfig and enforced checker settings. */
  #compilerOptions: ts.CompilerOptions = {};
  /** Checker buffers and ambient declaration filenames selected as current project roots. */
  #roots: string[] = [];
  /** Project version incremented every time a request installs a captured inventory. */
  #revision = 0;
  /** Prevents preparing further requests after cleanup. */
  #closed = false;

  /**
   * @param options - Virtual root, complete dependency files, and optional tsconfig selection.
   * Files are captured again for each request; caller mutations must occur between requests.
   */
  constructor(options: BrowserCheckerOptions) {
    this.#options = options;
  }

  /**
   * Collects finite declaration candidates and rejects unavailable required queries.
   * @param sources - Prepared sources with probes in original UTF-8 coordinates.
   * @param assets - Declaration records grouped by compiler module identity.
   * @param inputs - Captured overlays and ambient declarations applied over the configured inventory.
   * @returns Facts keyed by compiler identity, deduplicated by original range across checker buffers.
   * @throws If the service is closed, preparation fails, or checker contracts cannot be verified.
   */
  collect(
    sources: readonly TypeFactSource[],
    assets: ReadonlyMap<string, readonly NativeAssetRecord[]>,
    inputs?: SemanticInputs,
  ): Map<string, NativeTypeFact[]> {
    return this.#collect(sources, assets, inputs, false).facts;
  }

  /**
   * Collects usable facts while retaining unavailable original ranges for analysis.
   * @param sources - Prepared sources with original UTF-8 probes and checker correspondences.
   * @param assets - Declaration records grouped by compiler module identity.
   * @param inputs - Captured overlays and ambient declarations for this request.
   * @returns Verified facts and unverified ranges without discarding other expressions' answers.
   * @throws If preparation fails or checker results conflict for one original range.
   */
  collectAvailable(
    sources: readonly TypeFactSource[],
    assets: ReadonlyMap<string, readonly NativeAssetRecord[]>,
    inputs?: SemanticInputs,
  ): AvailableFacts {
    return this.#collect(sources, assets, inputs, true);
  }

  /**
   * Resolves authored imports and exports from module symbols in the virtual checker.
   * @param sources - Prepared sources whose checker buffers contain module requests.
   * @param assets - Declaration records used to prepare checker identities and helpers.
   * @param inputs - Captured overlays and ambient declarations for this request.
   * @returns Maps from importing resolver identity to specifier and resolved identity. Unmatched requests are omitted.
   * @throws If the service is closed or checker preparation fails.
   */
  resolveModules(
    sources: readonly TypeFactSource[],
    assets: ReadonlyMap<string, readonly NativeAssetRecord[]>,
    inputs?: SemanticInputs,
  ): Map<string, Map<string, string>> {
    const { program, mappings } = this.#prepare(sources, assets, inputs);

    const result = new Map<string, Map<string, string>>();
    const checker = program.getTypeChecker();
    for (const mapping of mappings) {
      const file = program.getSourceFile(mapping.id);
      if (!file) {
        continue;
      }

      const resolutions = new Map<string, string>();
      result.set(mapping.source.resolverId, resolutions);
      visit(file, (node) => {
        if (
          !(ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) ||
          !node.moduleSpecifier ||
          !ts.isStringLiteral(node.moduleSpecifier)
        ) {
          return;
        }
        const symbol = checker.getSymbolAtLocation(node.moduleSpecifier);
        const declaration = symbol?.declarations?.find(
          (item) => ts.isSourceFile(item) || item.getSourceFile().isDeclarationFile,
        );
        if (declaration) {
          const filename = declaration.getSourceFile().fileName;
          resolutions.set(
            node.moduleSpecifier.text,
            sources.find((source) => checkerId(source) === filename)?.resolverId ?? filename,
          );
        }
      });
    }

    return result;
  }

  /** Discards the language service after file or configuration changes. */
  invalidate(): void {
    this.#service?.dispose();
    this.#service = undefined;
  }
  /** Releases the checker. Subsequent queries reject. */
  close(): void {
    this.#closed = true;
    this.invalidate();
  }

  /**
   * Captures the inventory, builds checker buffers, and updates the reusable language service.
   * @param sources - Prepared sources with original probes and checker environments.
   * @param assets - Declaration records used to insert identities into producer calls.
   * @param inputs - Overlays and ambient declarations, which replace matching configured inventory entries.
   * @returns The current program, checker mappings, and serialized declaration identities.
   * @throws If the service is closed, probes cannot be mapped, tsconfig is invalid, or no program is available.
   */
  #prepare(
    sources: readonly TypeFactSource[],
    assets: ReadonlyMap<string, readonly NativeAssetRecord[]>,
    inputs: SemanticInputs = {},
  ) {
    if (this.#closed) {
      throw new Error("Browser checker is closed");
    }

    const files = new Map([
      ...this.#options.files,
      ...(inputs.overlayFiles ?? []),
      ...(inputs.ambientDeclarations ?? []),
    ]);
    const mappings: CheckerSource[] = [];
    const identities = new Map<string, NativeIdentity>();
    const ids = new Map(sources.map((source) => [source.resolverId, checkerId(source)]));
    const extraRoots = new Set<string>(inputs.ambientDeclarations?.keys());
    for (const source of sources) {
      if (source.kind === "projected" && source.projection.checkerGlobals) {
        const id = path.join(this.#options.root, `.stillwind-globals-${mappings.length}.d.ts`);
        files.set(id, source.projection.checkerGlobals);
        extraRoots.add(id);
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
        const mapping = new CheckerSource(
          source,
          buffer,
          index,
          assets.get(source.moduleId) ?? [],
          ids,
        );
        files.set(mapping.id, mapping.text);
        mappings.push(mapping);
        for (const [key, identity] of mapping.identities) {
          identities.set(key, identity);
        }
        if (mapping.helperModule) {
          files.set(mapping.helperModule.id, mapping.helperModule.text);
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
      for (const id of source.checkerTypes ?? []) {
        extraRoots.add(id);
      }
    }
    for (const [id, text] of files) {
      if (this.#files.get(id) !== text) {
        this.#versions.set(id, (this.#versions.get(id) ?? 0) + 1);
      }
    }
    this.#files = files;
    this.#revision++;
    const configPath = path.resolve(this.#options.root, this.#options.tsconfig || "tsconfig.json");
    let configured: ts.ParsedCommandLine | undefined;
    if (this.#options.tsconfig !== false && files.has(configPath)) {
      const config = ts.readConfigFile(configPath, (id) => files.get(id));
      if (config.error) {
        throw new Error(ts.flattenDiagnosticMessageText(config.error.messageText, "\n"));
      }
      configured = ts.parseJsonConfigFileContent(
        config.config,
        {
          useCaseSensitiveFileNames: true,
          fileExists: (id) => files.has(id),
          readFile: (id) => files.get(id),
          readDirectory: (directory, extensions, excludes, includes, depth) =>
            [...files.keys()].filter(
              (id) =>
                id.startsWith(directory + "/") &&
                (!extensions || extensions.some((ext) => id.endsWith(ext))) &&
                (depth === undefined ||
                  path.relative(directory, id).split("/").length - 1 <= depth) &&
                (includes ?? ["**/*"]).some((pattern) => matchesFile(directory, id, pattern)) &&
                !(excludes ?? []).some((pattern) => matchesFile(directory, id, pattern)),
            ),
        },
        path.dirname(configPath),
      );
      const error = configured.errors.find((item) => item.code !== 18003);
      if (error) {
        throw new Error(ts.flattenDiagnosticMessageText(error.messageText, "\n"));
      }
    } else if (typeof this.#options.tsconfig === "string") {
      throw new Error(`TypeScript configuration does not exist: ${configPath}`);
    }
    this.#compilerOptions = {
      target: ts.ScriptTarget.ESNext,
      module: ts.ModuleKind.ESNext,
      moduleResolution: ts.ModuleResolutionKind.Bundler,
      jsx: ts.JsxEmit.Preserve,
      strict: true,
      types: [],
      ...configured?.options,
      allowJs: true,
      checkJs: true,
      allowImportingTsExtensions: true,
      noEmit: true,
      skipLibCheck: true,
    };
    this.#roots = [
      ...new Set([
        ...mappings.map((mapping) => mapping.id),
        ...extraRoots,
        ...(configured?.fileNames.filter((id) => /\.d\.[cm]?ts$/.test(id)) ?? []),
      ]),
    ];
    if (!this.#service) {
      /**
       * Infers a virtual directory from the captured files beneath it.
       * @param directory - Absolute virtual directory with an optional trailing slash.
       * @returns Whether at least one file key starts with the directory prefix.
       */
      const directoryExists = (directory: string) =>
        [...this.#files.keys()].some((id) => id.startsWith(directory.replace(/\/$/, "") + "/"));
      this.#service = ts.createLanguageService({
        getCompilationSettings: () => this.#compilerOptions,
        getScriptFileNames: () => this.#roots,
        getScriptVersion: (id) => String(this.#versions.get(id) ?? 0),
        getProjectVersion: () => String(this.#revision),
        getScriptSnapshot: (id) => {
          const text = this.#files.get(id);
          return text === undefined ? undefined : ts.ScriptSnapshot.fromString(text);
        },
        getCurrentDirectory: () => this.#options.root,
        getDefaultLibFileName: () => "/node_modules/typescript/lib/lib.esnext.full.d.ts",
        fileExists: (id) => this.#files.has(id),
        readFile: (id) => this.#files.get(id),
        directoryExists,
        getDirectories: (directory) => [
          ...new Set(
            [...this.#files.keys()]
              .filter((id) => id.startsWith(directory + "/"))
              .map((id) => id.slice(directory.length + 1).split("/", 1)[0]),
          ),
        ],
        useCaseSensitiveFileNames: () => true,
      });
    }

    const program = this.#service.getProgram();
    if (!program) {
      throw new Error("Browser checker could not open the Stillwind project");
    }

    return {
      program,
      mappings,
      identities,
    };
  }

  /**
   * Decodes current checker expressions and enforces finite declaration identities for probes.
   * @param sources - Prepared sources with validated checker correspondences.
   * @param assets - Native declarations keyed by compiler identity.
   * @param inputs - Overlays and ambient declarations for this snapshot.
   * @param available - Whether unavailable queries are recorded instead of thrown.
   * @returns Facts grouped by compiler identity and original UTF-8 ranges that could not be verified.
   * @throws If preparation fails, checker results conflict, or strict mode encounters unavailable facts.
   */
  #collect(
    sources: readonly TypeFactSource[],
    assets: ReadonlyMap<string, readonly NativeAssetRecord[]>,
    inputs: SemanticInputs | undefined,
    available: boolean,
  ): AvailableFacts {
    const { program, mappings, identities } = this.#prepare(sources, assets, inputs);
    const checker = program.getTypeChecker();
    const facts = new Map<string, NativeTypeFact[]>();
    const unverified: UnverifiedRange[] = [];
    const brands = new Map<string, NativeIdentity["kind"]>();
    const producers: ts.Type[] = [];
    for (const mapping of mappings) {
      const file = program.getSourceFile(mapping.id);
      if (!file) {
        continue;
      }
      visit(file, (node) => {
        const kind = mapping.producers.get(`${node.getStart(file)}:${node.end}`);
        if (!kind) {
          return;
        }
        producers.push(checker.getTypeAtLocation(node));
        if (kind === "style") {
          return;
        }
        const prefix = {
          ref: "refIdentity",
          keyframes: "keyframesIdentity",
          viewTransition: "viewTransitionIdentity",
        }[kind];
        const property = checker
          .getPropertiesOfType(checker.getTypeAtLocation(node))
          .find((item) => item.name.startsWith(`__@${prefix}@`));
        if (property) {
          brands.set(property.name, kind);
        }
      });
    }
    /**
     * Records an unverifiable use when partial facts are allowed.
     * @param error - Failure with the original module identity and UTF-8 range, when available.
     * @throws The supplied error when this collection requires complete facts.
     */
    const unavailable = (error: TypeFactsError) => {
      if (!available) {
        throw error;
      }
      unverified.push({
        moduleId: error.moduleId,
        start: error.start,
        end: error.end,
        reason: error.message,
      });
    };
    for (const mapping of mappings) {
      const file = program.getSourceFile(mapping.id);
      if (!file) {
        throw new Error(`Browser checker omitted ${mapping.id}`);
      }
      const diagnostic = [
        ...program.getSyntacticDiagnostics(file),
        ...program.getSemanticDiagnostics(file),
      ].find(
        (item) =>
          !producerArgumentDiagnostic(item, file, mapping) &&
          /stillwind\/v1:|StyleResult|StyleRef|StyleKeyframes|StyleViewTransition|__stillwindDeclarationIdentity|data-__stillwindStyleIdentity/.test(
            ts.flattenDiagnosticMessageText(item.messageText, "\n"),
          ),
      );
      if (diagnostic) {
        const start = mapping.originalOffset(diagnostic.start ?? 0) ?? undefined;
        const end =
          mapping.originalOffset((diagnostic.start ?? 0) + (diagnostic.length ?? 0), true) ??
          undefined;
        unavailable(
          new TypeFactsError(
            mapping.source.moduleId,
            start,
            end,
            "assignment",
            `TS${diagnostic.code}: ${ts.flattenDiagnosticMessageText(diagnostic.messageText, "\n")} Preserve declaration candidates with an explicit union or a generic parameter.`,
          ),
        );
      }
      const matched = new Set<string>();
      const values = facts.get(mapping.source.moduleId) ?? [];
      visit(file, (node) => {
        if (!ts.isExpression(node)) {
          return;
        }
        const key = `${node.getStart(file)}:${node.end}`;
        const probe = mapping.probes.get(key);
        if (!probe) {
          return;
        }
        matched.add(key);
        const type = checker.getTypeAtLocation(node);
        const decoded = decode(checker, type, node, identities, brands, producers);
        if (
          decoded.kind === "asset" ||
          decoded.kind === "nonAsset" ||
          (decoded.kind === "unbranded" && probe.recognitionOnly)
        ) {
          const value = decoded.kind === "asset" ? decoded.value : null;
          const existing = values.find(
            (item) => item.start === probe.start && item.end === probe.end,
          );
          if (existing && JSON.stringify(existing.value) !== JSON.stringify(value)) {
            throw new TypeFactsError(
              mapping.source.moduleId,
              probe.start,
              probe.end,
              "checker",
              "Ambiguous checker results for this original expression",
            );
          }
          if (!existing) {
            values.push({ ...probe, value });
          }
        } else if (!(probe.recognitionOnly && !available)) {
          unavailable(
            new TypeFactsError(
              mapping.source.moduleId,
              probe.start,
              probe.end,
              "identity",
              "Cannot determine a finite set of declaration identities. Preserve the value's inferred type, use an explicit union, or use a generic parameter.",
            ),
          );
        }
      });
      for (const [key, probe] of mapping.probes) {
        if (!matched.has(key)) {
          unavailable(
            new TypeFactsError(
              mapping.source.moduleId,
              probe.start,
              probe.end,
              "checker",
              "Projected expression has no matching checker AST node",
            ),
          );
        }
      }
      facts.set(mapping.source.moduleId, values);
    }
    return { facts, unverified };
  }
}

/**
 * Applies tsconfig file and directory patterns to paths in the virtual filesystem.
 * @param directory - Absolute virtual base directory.
 * @param id - Absolute virtual file path.
 * @param pattern - Absolute or directory-relative glob. A plain directory name expands to its descendants.
 * @returns Whether the relative path matches, including dot-prefixed segments.
 */
function matchesFile(directory: string, id: string, pattern: string): boolean {
  const name = path.relative(directory, id);
  const glob = path.isAbsolute(pattern) ? path.relative(directory, pattern) : pattern;
  const expanded = !/[?*]/.test(glob) && !/\.[^/]+$/.test(glob) ? `${glob}/**/*` : glob;
  return minimatch(name, expanded, { dot: true });
}

/**
 * Visits an AST in parent-first order.
 * @param node - Root of the subtree to traverse.
 * @param callback - Operation invoked for the root and every child node. Exceptions propagate.
 */
function visit(node: ts.Node, callback: (node: ts.Node) => void): void {
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
 * @param checker - Checker that owns type and location.
 * @param type - Probed type, including unions of declaration candidates.
 * @param location - AST node used to obtain property types.
 * @param identities - Serialized keys mapped to declarations inserted into this snapshot.
 * @param brands - Checker symbol names registered from asset producers and mapped to their families.
 * @param producers - Producer call types of this snapshot, used to detect object types that may hold
 * a widened asset.
 * @returns Finite candidates, a confirmed non-asset, an object type that may hold a widened
 * asset, or an unverified value whose declarations cannot be determined.
 */
function decode(
  checker: ts.TypeChecker,
  type: ts.Type,
  location: ts.Node,
  identities: ReadonlyMap<string, NativeIdentity>,
  brands: ReadonlyMap<string, NativeIdentity["kind"]>,
  producers: readonly ts.Type[],
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
    const widened = members.some(
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
 * @param diagnostic - Checker diagnostic for `sourceFile`.
 * @param sourceFile - Checker source file that contains the diagnostic.
 * @param mapping - Producer call ranges of the same source.
 * @returns Whether the diagnostic is an argument error (TS2345 or TS2769) whose innermost
 * enclosing call is a recorded producer call.
 */
function producerArgumentDiagnostic(
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
