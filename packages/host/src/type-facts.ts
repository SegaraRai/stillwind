import path from "node:path";

import type { Node, SourceFile } from "typescript/unstable/ast";
import { isExpression } from "typescript/unstable/ast/is";
import { API } from "typescript/unstable/sync";
import type { Checker, Snapshot, Type } from "typescript/unstable/sync";

export interface TypeFactSource {
  id: string;
  path: string;
  source: string;
  /** Byte spans requested by the macro frontend; unrelated expressions are not queried. */
  probes: readonly { start: number; end: number }[];
}

export interface IdentityId {
  kind: "scope" | "ref" | "animation" | "view-transition";
  module_path: string;
  start: number;
  end: number;
  binding_name: string;
}

export interface AssetRecord {
  identity: IdentityId;
  metadata: { label: string | null };
  call_start: number;
  call_end: number;
}

export interface TypeFact {
  start: number;
  end: number;
  value: unknown;
}

interface Insertion {
  at: number;
  text: string;
}

const normalize = (fileName: string) => path.resolve(fileName).replaceAll("\\", "/").toLowerCase();

function visit(node: Node, callback: (node: Node) => void) {
  callback(node);
  node.forEachChild((child) => visit(child, callback));
}

function byteToUtf16(text: string, offset: number): number {
  return Buffer.from(text).subarray(0, offset).toString("utf8").length;
}

function utf16ToByte(text: string, offset: number): number {
  return Buffer.byteLength(text.slice(0, offset), "utf8");
}

function identityKey(identity: IdentityId): string {
  return `stillwind/v1:${Buffer.from(JSON.stringify(identity)).toString("base64url")}`;
}

function shadowSource(source: TypeFactSource, assets: readonly AssetRecord[]) {
  const identities = new Map<string, IdentityId>();
  const insertions = assets
    .map((asset): Insertion => {
      const key = identityKey(asset.identity);
      identities.set(key, asset.identity);
      const callStart = byteToUtf16(source.source, asset.call_start);
      const callEnd = byteToUtf16(source.source, asset.call_end);
      const call = source.source.slice(callStart, callEnd);
      const closingParen = call.lastIndexOf(")");
      if (closingParen < 0) throw new Error(`Stillwind producer has no closing parenthesis`);
      const openingParen = call.indexOf("(");
      const hasArguments = call.slice(openingParen + 1, closingParen).trim().length > 0;
      const identityType = `{readonly __stillwindDeclarationIdentity:${JSON.stringify(key)}}`;
      return {
        at: callStart + closingParen,
        text: `${hasArguments ? "," : ""}undefined as unknown as ${identityType}`,
      };
    })
    .sort((left, right) => left.at - right.at);

  let text = source.source;
  for (const insertion of [...insertions].reverse()) {
    text = text.slice(0, insertion.at) + insertion.text + text.slice(insertion.at);
  }
  return { text, insertions, identities };
}

function shadowToOriginal(position: number, insertions: readonly Insertion[]): number | null {
  let inserted = 0;
  for (const insertion of insertions) {
    const shadowStart = insertion.at + inserted;
    const shadowEnd = shadowStart + insertion.text.length;
    if (position < shadowStart) break;
    if (position < shadowEnd) return null;
    inserted += insertion.text.length;
  }
  return position - inserted;
}

function typeArguments(checker: Checker, type: Type): readonly Type[] {
  const aliasArguments = type.getAliasTypeArguments();
  if (aliasArguments.length > 0) return aliasArguments;
  return type.isTypeReference() ? checker.getTypeArguments(type) : [];
}

function knownIdentities(
  type: Type | undefined,
  identities: ReadonlyMap<string, IdentityId>,
  kind: IdentityId["kind"],
): IdentityId[] | null {
  if (!type) return null;
  const types = type.isUnionType() ? type.getTypes() : [type];
  const result: IdentityId[] = [];
  for (const candidate of types) {
    if (!candidate.isStringLiteralType()) return null;
    const identity = identities.get(candidate.value);
    if (!identity || identity.kind !== kind) return null;
    result.push(identity);
  }
  return result;
}

function symbolic(kind: IdentityId["kind"], identities: readonly IdentityId[]) {
  return { kind, candidates: { Finite: identities } };
}

function typeFactValue(
  checker: Checker,
  type: Type,
  identities: ReadonlyMap<string, IdentityId>,
): object | null {
  const constituents = type.isUnionType() ? type.getTypes() : [type];
  const refs = [];
  const scopes: IdentityId[] = [];
  const animations: IdentityId[] = [];
  const viewTransitions: IdentityId[] = [];

  for (const constituent of constituents) {
    const symbol = constituent.getAliasSymbol() ?? constituent.getSymbol();
    const args = typeArguments(checker, constituent);
    if (symbol?.name === "StyleScope") {
      const candidates = knownIdentities(args[0], identities, "scope");
      if (!candidates) return null;
      scopes.push(...candidates);
      continue;
    }
    if (symbol?.name === "StyleRef") {
      const references = knownIdentities(args[0], identities, "ref");
      if (!references) return null;
      const scopeText = args[1] ? checker.typeToString(args[1]) : "never";
      const permittedScopes =
        scopeText === "never" ? [] : knownIdentities(args[1], identities, "scope");
      if (!permittedScopes) return null;
      for (const reference of references) {
        refs.push({
          reference,
          permitted_scopes:
            permittedScopes.length === 0 ? null : symbolic("scope", permittedScopes),
        });
      }
      continue;
    }
    if (symbol?.name === "StyleAnimation" || symbol?.name === "StyleAnimationUse") {
      const candidates = knownIdentities(args[0], identities, "animation");
      if (!candidates) return null;
      animations.push(...candidates);
      continue;
    }
    if (symbol?.name === "StyleViewTransition" || symbol?.name === "StyleViewTransitionUse") {
      const candidates = knownIdentities(args[0], identities, "view-transition");
      if (!candidates) return null;
      viewTransitions.push(...candidates);
      continue;
    }
    return null;
  }

  const populated = [refs, scopes, animations, viewTransitions].filter(
    (values) => values.length > 0,
  );
  if (populated.length !== 1) return null;
  if (refs.length > 0) return { kind: "refs", value: refs };
  if (scopes.length > 0) return { kind: "scopes", value: symbolic("scope", scopes) };
  if (animations.length > 0)
    return { kind: "animations", value: symbolic("animation", animations) };
  if (viewTransitions.length > 0)
    return {
      kind: "view-transitions",
      value: symbolic("view-transition", viewTransitions),
    };
  return null;
}

interface PreparedProject {
  virtualFiles: Map<string, string>;
  mappings: Map<
    string,
    { source: TypeFactSource; insertions: Insertion[]; identities: Map<string, IdentityId> }
  >;
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
  const mappings = new Map<
    string,
    { source: TypeFactSource; insertions: Insertion[]; identities: Map<string, IdentityId> }
  >();
  const allIdentities = new Map<string, IdentityId>();

  for (const source of sources) {
    const shadow = shadowSource(source, assetsByPath.get(source.path) ?? []);
    virtualFiles.set(normalize(source.id), shadow.text);
    mappings.set(normalize(source.id), { source, ...shadow });
    for (const [key, identity] of shadow.identities) allIdentities.set(key, identity);
  }

  virtualFiles.set(
    normalize(configFile),
    JSON.stringify({
      compilerOptions: {
        allowJs: true,
        checkJs: false,
        jsx: "react-jsx",
        module: "esnext",
        moduleResolution: "bundler",
        noEmit: true,
        skipLibCheck: true,
        strict: true,
        target: "esnext",
      },
      files: sources.map((source) => source.id),
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
    if (this.#closed) throw new Error("TypeScript facts service is closed");
    const prepared = prepareProject(sources, assetsByPath, this.#configFile);
    try {
      return this.#collectPrepared(prepared);
    } catch (firstError) {
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
    if (this.#closed) return;
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
        fileExists: (fileName) => (this.#virtualFiles.has(normalize(fileName)) ? true : undefined),
        readFile: (fileName) => {
          const overlay = this.#virtualFiles.get(normalize(fileName));
          if (overlay === undefined) this.stats.diskReadRequests += 1;
          else this.stats.overlayReads += 1;
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
      if (!this.#virtualFiles.has(fileName)) created.push(fileName);
      else if (this.#virtualFiles.get(fileName) !== contents) changed.push(fileName);
    }
    for (const fileName of this.#virtualFiles.keys()) {
      if (!prepared.virtualFiles.has(fileName)) deleted.push(fileName);
    }
    this.#virtualFiles = prepared.virtualFiles;

    const next = this.#snapshot
      ? this.#api.updateSnapshot({ fileChanges: { changed, created, deleted } })
      : this.#api.updateSnapshot({ openProjects: [this.#configFile] });
    this.stats.snapshots += 1;
    if (this.#snapshot) this.stats.incrementalSnapshots += 1;
    try {
      const project = next.getProject(this.#configFile);
      if (!project) throw new Error("TypeScript 7 could not open the Stillwind shadow project");
      const result = new Map<string, TypeFact[]>();
      for (const mapping of prepared.mappings.values()) {
        const sourceFile = project.program.getSourceFile(mapping.source.id);
        if (!sourceFile) throw new Error(`TypeScript 7 omitted ${mapping.source.id}`);
        result.set(
          mapping.source.path,
          factsForSource(project.checker, sourceFile, mapping, prepared.identities),
        );
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

function factsForSource(
  checker: Checker,
  sourceFile: SourceFile,
  mapping: { source: TypeFactSource; insertions: Insertion[] },
  identities: ReadonlyMap<string, IdentityId>,
): TypeFact[] {
  const expressions: Node[] = [];
  const probes = new Set(mapping.source.probes.map(({ start, end }) => `${start}:${end}`));
  if (probes.size === 0) return [];
  visit(sourceFile, (node) => {
    if (!isExpression(node)) return;
    const start = shadowToOriginal(node.getStart(sourceFile), mapping.insertions);
    const end = shadowToOriginal(node.end, mapping.insertions);
    if (start === null || end === null) return;
    if (
      probes.has(
        `${utf16ToByte(mapping.source.source, start)}:${utf16ToByte(mapping.source.source, end)}`,
      )
    )
      expressions.push(node);
  });
  const types = checker.getTypeAtLocation(expressions);
  const facts: TypeFact[] = [];
  for (let index = 0; index < expressions.length; index += 1) {
    const type = types[index];
    if (!type) continue;
    const value = typeFactValue(checker, type, identities);
    if (!value) continue;
    const start = shadowToOriginal(expressions[index].getStart(sourceFile), mapping.insertions);
    const end = shadowToOriginal(expressions[index].end, mapping.insertions);
    if (start === null || end === null) continue;
    facts.push({
      start: utf16ToByte(mapping.source.source, start),
      end: utf16ToByte(mapping.source.source, end),
      value,
    });
  }
  return facts;
}
