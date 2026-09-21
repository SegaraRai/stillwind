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
import { API, TypeFlags } from "typescript/unstable/sync";
import type { Checker, Snapshot, Type, Diagnostic } from "typescript/unstable/sync";

import { CheckerSource, checkerId, byteToUtf16 } from "./checker-source.ts";
import type { TypeFactSource } from "./checker-source.ts";
export type { TypeFactSource } from "./checker-source.ts";

/** A failed checker contract, reported at original UTF-8 coordinates. */
export class TypeFactsError extends Error {
  readonly path: string;
  readonly start: number;
  readonly end: number;
  readonly code: string;
  constructor(path: string, start: number, end: number, code: string, message: string) {
    super(`${code}: ${message} (${path}:${start})`);
    this.path = path;
    this.start = start;
    this.end = end;
    this.code = code;
  }

  /** Formats original UTF-8 bounds and original UTF-16 editor coordinates. */
  diagnostic(source: string) {
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
      file: this.path,
      start: this.start,
      end: this.end,
      start_utf16: start.utf16,
      end_utf16: end.utf16,
      start_line: start.line,
      end_line: end.line,
      start_column: start.column,
      end_column: end.column,
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

function typeFactValue(
  checker: Checker,
  type: Type,
  identities: ReadonlyMap<string, IdentityId>,
): NativeExportedValue | null {
  const constituents = type.isUnionType() ? type.getTypes() : [type];
  const refs = [];
  const keyframes: IdentityId[] = [];
  const viewTransitions: IdentityId[] = [];
  let viewTransitionKeyed = false;
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
          permitted_boundaries:
            permittedBoundaries.length === 0 ? null : symbolic("ref", permittedBoundaries),
        });
      }
      continue;
    }
    if (symbol?.name === "StyleKeyframes" || symbol?.name === "StyleKeyframesUse") {
      const candidates = knownIdentities(args[0], identities, "keyframes");
      if (!candidates) {
        return null;
      }
      keyframes.push(...candidates);
      continue;
    }
    if (symbol?.name === "StyleViewTransition" || symbol?.name === "StyleViewTransitionUse") {
      const candidates = knownIdentities(args[0], identities, "view-transition");
      if (!candidates) {
        return null;
      }
      viewTransitions.push(...candidates);
      viewTransitionKeyed ||= symbol.name === "StyleViewTransitionUse";
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
    return { kind: "styles", value: symbolic("style", styles) };
  }
  if (refs.length > 0) {
    return { kind: "refs", value: refs };
  }
  if (keyframes.length > 0) {
    return { kind: "keyframes", value: symbolic("keyframes", keyframes) };
  }
  if (viewTransitions.length > 0) {
    return viewTransitionKeyed
      ? {
          kind: "view-transition-keys",
          value: symbolic("view-transition", viewTransitions),
        }
      : {
          kind: "view-transitions",
          value: symbolic("view-transition", viewTransitions),
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
  assetsByPath: ReadonlyMap<string, readonly AssetRecord[]>,
  configFile: string,
): PreparedProject {
  const virtualFiles = new Map<string, string>();
  const mappings: CheckerSource[] = [];
  const allIdentities = new Map<string, IdentityId>();
  const ids = new Map(sources.map((source) => [source.id, checkerId(source)]));
  for (const source of sources) {
    const buffers = source.checker_buffers ?? [
      {
        source: source.source,
        language:
          source.language === "tsx" || source.language === "jsx"
            ? ("tsx" as const)
            : ("ts" as const),
      },
    ];
    for (const [index, buffer] of buffers.entries()) {
      const shadow = new CheckerSource(
        source,
        buffer,
        index,
        assetsByPath.get(source.path) ?? [],
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
          source.path,
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
        ...new Set(sources.flatMap((source) => source.checker_types ?? [])),
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
    assetsByPath: ReadonlyMap<string, readonly AssetRecord[]>,
  ): Map<string, TypeFact[]> {
    if (this.#closed) {
      throw new Error("TypeScript facts service is closed");
    }
    const prepared = prepareProject(sources, assetsByPath, this.#configFile);
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
            mapping.source.path,
            start,
            end,
            "SHTS002",
            `TS${diagnostic.code}: ${message} Preserve declaration candidates with an explicit union or a generic parameter.`,
          );
        }
        const facts = factsForSource(project.checker, sourceFile, mapping, prepared.identities);
        const previous = result.get(mapping.source.path) ?? [];
        for (const fact of facts) {
          const existing = previous.find(
            (value) => value.start === fact.start && value.end === fact.end,
          );
          if (existing && JSON.stringify(existing.value) !== JSON.stringify(fact.value)) {
            throw new TypeFactsError(
              mapping.source.path,
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
        result.set(mapping.source.path, previous);
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
  assetsByPath: ReadonlyMap<string, readonly AssetRecord[]>,
  projectRoot: string,
): Map<string, TypeFact[]> {
  const service = new TypeScriptFactsService(projectRoot);
  try {
    return service.collect(sources, assetsByPath);
  } finally {
    service.close();
  }
}

/** Includes nested checker explanations that may carry the declaration identity. */
function diagnosticText(diagnostic: Diagnostic): string {
  return [diagnostic.text, ...(diagnostic.messageChain ?? []).map(diagnosticText)].join("\n");
}

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
        mapping.source.path,
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
      const name = type && (type.getAliasSymbol() ?? type.getSymbol())?.name;
      if (
        !type ||
        (type.flags & (TypeFlags.Any | TypeFlags.Unknown) &&
          (isIdentifier(expression) ||
            isCallExpression(expression) ||
            isPropertyAccessExpression(expression) ||
            isElementAccessExpression(expression))) ||
        (name && /^Style(?:Result|Ref|Keyframes(?:Use)?|ViewTransition(?:Use)?)$/.test(name)) ||
        (type.isUnionType() &&
          type
            .getTypes()
            .some((t) =>
              /^Style(?:Result|Ref|Keyframes(?:Use)?|ViewTransition(?:Use)?)$/.test(
                (t.getAliasSymbol() ?? t.getSymbol())?.name ?? "",
              ),
            ))
      ) {
        throw new TypeFactsError(
          mapping.source.path,
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
