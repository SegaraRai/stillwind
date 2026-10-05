import { createHash } from "node:crypto";
import { existsSync } from "node:fs";
import path from "node:path";

import type {
  NativeIdentity,
  NativeAssetRecord,
  NativeTypeFact,
  NativeSymbolicValue,
  NativeExportedValue,
  NativeSourceIssue,
} from "@stillwind/native";
import type { Node, SourceFile } from "typescript/unstable/ast";
import {
  isExpression,
  isIdentifier,
  isCallExpression,
  isPropertyAccessExpression,
  isElementAccessExpression,
} from "typescript/unstable/ast/is";
import {
  API,
  TypeFlags,
  SignatureKind,
  type Checker,
  Snapshot,
  type Type,
  type Diagnostic,
} from "typescript/unstable/sync";

import { CheckerSource, checkerId, type TypeFactSource } from "./checker-source.ts";

export type { TypeFactSource } from "./checker-source.ts";

/** A failed checker contract, reported at original UTF-8 coordinates. */
export class TypeFactsError extends Error {
  readonly moduleId: string;
  readonly start?: number;
  readonly end?: number;
  readonly issue: NativeSourceIssue;

  constructor(
    moduleId: string,
    start: number | undefined,
    end: number | undefined,
    kind: NativeSourceIssue["kind"],
    detail: string,
  ) {
    super(detail);
    this.moduleId = moduleId;
    this.start = start;
    this.end = end;
    this.issue = { kind, detail };
  }
}

export type IdentityId = NativeIdentity;
export type AssetRecord = NativeAssetRecord;
export type TypeFact = NativeTypeFact;

const normalize = (fileName: string) => {
  const name = path.resolve(fileName).replaceAll("\\", "/");
  return process.platform === "win32" ? name.toLowerCase() : name;
};

function visit(node: Node, callback: (node: Node) => void) {
  callback(node);
  node.forEachChild((child) => visit(child, callback));
}

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

function symbolic(
  kind: IdentityId["kind"],
  identities: readonly IdentityId[],
): NativeSymbolicValue {
  return { kind, candidates: { kind: "finite", values: [...identities] } };
}

/** Registered identities and the core module's canonical unique-symbol brands. */
class AssetTypeDecoder {
  readonly #checker: Checker;
  readonly #identities: ReadonlyMap<string, IdentityId>;
  readonly #modules = new Set<number>();
  readonly #brands = new Map<string, IdentityId["kind"]>();

  constructor(checker: Checker, identities: ReadonlyMap<string, IdentityId>) {
    this.#checker = checker;
    this.#identities = identities;
  }

  /** Loads canonical unique-symbol properties from a registered producer's core module. */
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

  decode(type: Type): { value: NativeExportedValue | null; recognized: boolean } {
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

interface PreparedProject {
  virtualFiles: Map<string, string>;
  mappings: CheckerSource[];
  identities: Map<string, IdentityId>;
}

export interface TypeScriptFactsStats {
  starts: number;
  snapshots: number;
  incrementalSnapshots: number;
  restarts: number;
  overlayReads: number;
  diskReadRequests: number;
}

function prepareProject(
  sources: readonly TypeFactSource[],
  assetsByModuleId: ReadonlyMap<string, readonly AssetRecord[]>,
  configFile: string,
): PreparedProject {
  const virtualFiles = new Map<string, string>();
  const mappings: CheckerSource[] = [];
  const allIdentities = new Map<string, IdentityId>();
  const globals = new Set<string>();
  const ids = new Map(sources.map((source) => [source.resolverId, checkerId(source)]));
  for (const source of sources) {
    const declarations = source.kind === "projected" && source.projection.checkerGlobals;
    if (declarations) {
      const fingerprint = createHash("sha256").update(declarations).digest("hex").slice(0, 16);
      const id = normalize(
        path.join(path.dirname(configFile), `.stillwind-globals-${fingerprint}.d.ts`),
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
    normalize(configFile),
    JSON.stringify({
      compilerOptions: {
        allowJs: true,
        checkJs: true,
        jsx: "preserve",
        module: "esnext",
        moduleResolution: "bundler",
        noEmit: true,
        skipLibCheck: true,
        strict: true,
        target: "esnext",
        allowImportingTsExtensions: true,
        types: [],
      },
      files: [
        ...globals,
        ...mappings.map((mapping) => mapping.id),
        ...new Set(sources.flatMap((source) => source.checkerTypes ?? [])),
      ],
    }),
  );
  return { virtualFiles, mappings, identities: allIdentities };
}

/** Owns one TS7 process and a chain of incremental snapshots for a Vite project. */
export class TypeScriptFactsService {
  readonly stats: TypeScriptFactsStats = {
    starts: 0,
    snapshots: 0,
    incrementalSnapshots: 0,
    restarts: 0,
    overlayReads: 0,
    diskReadRequests: 0,
  };

  readonly #configFile: string;
  readonly #projectRoot: string;
  #api: API;
  #closed = false;
  #snapshot: Snapshot | undefined;
  #virtualFiles = new Map<string, string>();

  constructor(projectRoot: string) {
    this.#projectRoot = projectRoot;
    this.#configFile = path.join(projectRoot, ".stillwind-tsconfig.json");
    this.#api = this.#startApi();
  }

  collect(
    sources: readonly TypeFactSource[],
    assetsByModuleId: ReadonlyMap<string, readonly AssetRecord[]>,
  ): Map<string, TypeFact[]> {
    if (this.#closed) {
      throw new Error("TypeScript facts service is closed");
    }
    const prepared = prepareProject(sources, assetsByModuleId, this.#configFile);
    try {
      return this.#collectPrepared(prepared);
    } catch (firstError) {
      if (firstError instanceof TypeFactsError) {
        throw firstError;
      }
      this.stats.restarts += 1;
      this.#restartApi();
      try {
        return this.#collectPrepared(prepared);
      } catch (retryError) {
        throw new AggregateError(
          [firstError, retryError],
          `TypeScript 7 checker failed before and after a clean restart: ${String(retryError)}`,
        );
      }
    }
  }

  close(): void {
    if (this.#closed) {
      return;
    }
    this.#closed = true;
    this.#snapshot?.dispose();
    this.#snapshot = undefined;
    this.#api.close();
  }

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
        fileExists: (fileName) => (this.#virtualFiles.has(normalize(fileName)) ? true : undefined),
        readFile: (fileName) => {
          const overlay = this.#virtualFiles.get(normalize(fileName));
          if (overlay === undefined) {
            this.stats.diskReadRequests += 1;
          } else {
            this.stats.overlayReads += 1;
          }
          return overlay;
        },
      },
    });
  }

  #restartApi(): void {
    this.#snapshot?.dispose();
    this.#snapshot = undefined;
    this.#api.close();
    this.#api = this.#startApi();
  }

  #collectPrepared(prepared: PreparedProject): Map<string, TypeFact[]> {
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
      ? this.#api.updateSnapshot({ fileChanges: { changed, created, deleted } })
      : this.#api.updateSnapshot({ openProjects: [this.#configFile] });
    this.stats.snapshots += 1;
    if (this.#snapshot) {
      this.stats.incrementalSnapshots += 1;
    }
    try {
      const project = next.getProject(this.#configFile);
      if (!project) {
        throw new Error("TypeScript 7 could not open the Stillwind shadow project");
      }
      const result = new Map<string, TypeFact[]>();
      const decoder = new AssetTypeDecoder(project.checker, prepared.identities);
      const producers: Node[] = [];
      for (const mapping of prepared.mappings) {
        const file = project.program.getSourceFile(mapping.id);
        if (!file) {
          continue;
        }
        visit(file, (node) => {
          if (
            isCallExpression(node) &&
            mapping.producers.has(`${node.getStart(file)}:${node.end}`)
          ) {
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
          throw new TypeFactsError(
            mapping.source.moduleId,
            start,
            end,
            "assignment",
            `TS${diagnostic.code}: ${message} Preserve declaration candidates with an explicit union or a generic parameter.`,
          );
        }
        const facts = factsForSource(project.checker, sourceFile, mapping, decoder);
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
      return result;
    } finally {
      this.#snapshot?.dispose();
      this.#snapshot = next;
    }
  }
}

/** One-shot convenience API for tests and non-Vite callers. */
export function collectTypeScriptFacts(
  sources: readonly TypeFactSource[],
  assetsByModuleId: ReadonlyMap<string, readonly AssetRecord[]>,
  projectRoot: string,
): Map<string, TypeFact[]> {
  const service = new TypeScriptFactsService(projectRoot);
  try {
    return service.collect(sources, assetsByModuleId);
  } finally {
    service.close();
  }
}

/** Includes nested checker explanations that may carry the declaration identity. */
function diagnosticText(diagnostic: Diagnostic): string {
  return [diagnostic.text, ...(diagnostic.messageChain ?? []).map(diagnosticText)].join("\n");
}

/**
 * Queries the types of expressions in the shadow AST described by `mapping`.
 * `sourceFile` must be the checker source whose offsets are used in that mapping.
 * Returns facts in AST traversal order, with each probe's original UTF-8 range.
 *
 * Throws TypeFactsError if a probe has no matching AST expression, even when it is
 * optional. It also throws when a required query needs Stillwind declarations but
 * cannot resolve their identities. Unrecognized optional probes and ordinary
 * expressions that do not represent assets produce no fact.
 */
function factsForSource(
  checker: Checker,
  sourceFile: SourceFile,
  mapping: CheckerSource,
  decoder: AssetTypeDecoder,
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
      throw new TypeFactsError(
        mapping.source.moduleId,
        probe.start,
        probe.end,
        "checker",
        "Projected expression has no matching checker AST node",
      );
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
      // Recognition queries only identify Stillwind receivers for unconsumed-use diagnostics.
      // Missing facts leave the receiver as ordinary JavaScript without an error.
      if (probe.recognitionOnly) {
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
        throw new TypeFactsError(
          mapping.source.moduleId,
          probe.start,
          probe.end,
          "identity",
          "Cannot determine a finite set of declaration identities. Preserve the value's inferred type, use an explicit union, or use a generic parameter.",
        );
      }
      continue;
    }
    facts.push({ ...probe, value });
  }
  return facts;
}
