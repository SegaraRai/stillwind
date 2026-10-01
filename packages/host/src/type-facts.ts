import { createHash } from "node:crypto";
import { existsSync } from "node:fs";
import path from "node:path";

import type {
  NativeIdentity,
  NativeAssetRecord,
  NativeTypeFact,
  NativeSymbolicValue,
  NativeExportedValue,
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
  type Checker,
  Snapshot,
  type Type,
  type Diagnostic,
} from "typescript/unstable/sync";

import { CheckerSource, checkerId, byteToUtf16, type TypeFactSource } from "./checker-source.ts";

export type { TypeFactSource } from "./checker-source.ts";

/** A failed checker contract, reported at original UTF-8 coordinates. */
export class TypeFactsError extends Error {
  readonly moduleId: string;
  readonly start: number;
  readonly end: number;
  readonly code: string;
  constructor(moduleId: string, start: number, end: number, code: string, message: string) {
    super(`${code}: ${message} (${moduleId}:${start})`);
    this.moduleId = moduleId;
    this.start = start;
    this.end = end;
    this.code = code;
  }

  /** Formats original UTF-8 bounds and original UTF-16 editor coordinates. */
  diagnostic(source: string, uri: string) {
    const position = (offset: number) => {
      const utf16 = byteToUtf16(source, offset);
      const lines = source.slice(0, utf16).split("\n");
      return {
        utf16,
        line: lines.length - 1,
        column: lines.at(-1)!.length,
      };
    };
    const start = position(this.start),
      end = position(this.end);
    return {
      code: this.code,
      message: this.message,
      uri,
      revision: createHash("sha256").update(source).digest("hex"),
      start: this.start,
      end: this.end,
      startUtf16: start.utf16,
      endUtf16: end.utf16,
      startLine: start.line,
      endLine: end.line,
      startColumn: start.column,
      endColumn: end.column,
      notes: [],
    };
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

function typeArguments(checker: Checker, type: Type): readonly Type[] {
  const aliasArguments = type.getAliasTypeArguments();
  if (aliasArguments.length > 0) {
    return aliasArguments;
  }
  return type.isTypeReference() ? checker.getTypeArguments(type) : [];
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

/**
 * Returns the finite set of declaration identities represented by a checker type.
 * Each identity must be registered in `identities`. Every member of a union must
 * belong to the same asset family.
 *
 * Returns null for unsupported types, types that represent macro uses, unions of
 * different asset families, or unknown identities. The function reads type arguments
 * from the checker and leaves the identity registry unchanged.
 */
function typeFactValue(
  checker: Checker,
  type: Type,
  identities: ReadonlyMap<string, IdentityId>,
): NativeExportedValue | null {
  const constituents = type.isUnionType() ? type.getTypes() : [type];
  const refs = [];
  const keyframes: IdentityId[] = [];
  const viewTransitions: IdentityId[] = [];
  const styles: IdentityId[] = [];

  for (const constituent of constituents) {
    const symbol = constituent.getAliasSymbol() ?? constituent.getSymbol();
    const args = typeArguments(checker, constituent);
    if (symbol?.name === "StyleResult") {
      const candidates = knownIdentities(args[0], identities, "style");
      if (!candidates) {
        return null;
      }
      styles.push(...candidates);
      continue;
    }
    if (symbol?.name === "StyleRef") {
      const references = knownIdentities(args[0], identities, "ref");
      if (!references) {
        return null;
      }
      const boundaryText = args[1] ? checker.typeToString(args[1]) : "never";
      const permittedBoundaries =
        boundaryText === "never" ? [] : knownIdentities(args[1], identities, "ref");
      if (!permittedBoundaries) {
        return null;
      }
      for (const reference of references) {
        refs.push({
          reference,
          permittedBoundaries:
            permittedBoundaries.length === 0 ? null : symbolic("ref", permittedBoundaries),
        });
      }
      continue;
    }
    if (symbol?.name === "StyleKeyframes") {
      const candidates = knownIdentities(args[0], identities, "keyframes");
      if (!candidates) {
        return null;
      }
      keyframes.push(...candidates);
      continue;
    }
    if (symbol?.name === "StyleViewTransition") {
      const candidates = knownIdentities(args[0], identities, "viewTransition");
      if (!candidates) {
        return null;
      }
      viewTransitions.push(...candidates);
      continue;
    }
    return null;
  }

  const populated = [refs, keyframes, viewTransitions, styles].filter(
    (values) => values.length > 0,
  );
  if (populated.length !== 1) {
    return null;
  }
  if (styles.length > 0) {
    return {
      kind: "styles",
      value: symbolic("style", styles),
    };
  }
  if (refs.length > 0) {
    return {
      kind: "refs",
      value: refs,
    };
  }
  if (keyframes.length > 0) {
    return {
      kind: "keyframes",
      value: symbolic("keyframes", keyframes),
    };
  }
  if (viewTransitions.length > 0) {
    return {
      kind: "viewTransitions",
      value: symbolic("viewTransition", viewTransitions),
    };
  }
  return null;
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
  const ids = new Map(sources.map((source) => [source.resolverId, checkerId(source)]));
  for (const source of sources) {
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
          "SHTS003",
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
          const start = mapping.originalOffset(diagnostic.pos) ?? 0;
          const end = mapping.originalOffset(diagnostic.end, true) ?? start;
          const message = diagnosticText(diagnostic).replace(
            /stillwind\/v1:[A-Za-z0-9_-]+/g,
            "declaration identity",
          );
          throw new TypeFactsError(
            mapping.source.moduleId,
            start,
            end,
            "SHTS002",
            `TS${diagnostic.code}: ${message} Preserve declaration candidates with an explicit union or a generic parameter.`,
          );
        }
        const facts = factsForSource(project.checker, sourceFile, mapping, prepared.identities);
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
              "SHTS003",
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
  identities: ReadonlyMap<string, IdentityId>,
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
        "SHTS003",
        "Projected expression has no matching checker AST node",
      );
    }
  }
  const types = checker.getTypeAtLocation(expressions);
  const facts: TypeFact[] = [];
  for (const [index, expression] of expressions.entries()) {
    const probe = mapping.probes.get(`${expression.getStart(sourceFile)}:${expression.end}`)!;
    const type = types[index];
    const value = type && typeFactValue(checker, type, identities);
    if (!value) {
      // Recognition queries only identify Stillwind receivers for unconsumed-use diagnostics.
      // Missing facts leave the receiver as ordinary JavaScript without an error.
      if (probe.recognitionOnly) {
        continue;
      }
      const name = type && (type.getAliasSymbol() ?? type.getSymbol())?.name;
      if (
        !type ||
        (type.flags & (TypeFlags.Any | TypeFlags.Unknown) &&
          (isIdentifier(expression) ||
            isCallExpression(expression) ||
            isPropertyAccessExpression(expression) ||
            isElementAccessExpression(expression))) ||
        (name && /^Style(?:Result|Ref|Keyframes|ViewTransition)$/.test(name)) ||
        (type.isUnionType() &&
          type
            .getTypes()
            .some((t) =>
              /^Style(?:Result|Ref|Keyframes|ViewTransition)$/.test(
                (t.getAliasSymbol() ?? t.getSymbol())?.name ?? "",
              ),
            ))
      ) {
        throw new TypeFactsError(
          mapping.source.moduleId,
          probe.start,
          probe.end,
          "SHTS003",
          "Cannot determine a finite set of declaration identities. Preserve the value's inferred type, use an explicit union, or use a generic parameter.",
        );
      }
      continue;
    }
    facts.push({ ...probe, value });
  }
  return facts;
}
