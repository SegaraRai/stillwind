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
import { SyntaxKind } from "typescript/unstable/ast";
import {
  isExpression,
  isIdentifier,
  isVariableDeclaration,
  isBinaryExpression,
  isConditionalExpression,
  isParenthesizedExpression,
  isPropertyAccessExpression,
  isCallExpression,
  isFunctionDeclaration,
  isFunctionExpression,
  isArrowFunction,
  isBlock,
  isReturnStatement,
  isPropertyAssignment,
  isObjectLiteralExpression,
  isShorthandPropertyAssignment,
} from "typescript/unstable/ast/is";
import { API, SymbolFlags, TypeFlags } from "typescript/unstable/sync";
import type { Checker, Snapshot, Type } from "typescript/unstable/sync";

export interface TypeFactSource {
  id: string;
  path: string;
  source: string;
  /** Byte spans requested by the macro frontend; unrelated expressions are not queried. */
  probes: readonly { start: number; end: number }[];
  /** Host-selected local modules; spans include the import's string quotes. */
  resolutions?: readonly { start: number; end: number; id: string }[];
}

export type IdentityId = NativeIdentity;
export type AssetRecord = NativeAssetRecord;
export type TypeFact = NativeTypeFact;

interface ShadowEdit {
  at: number;
  end: number;
  text: string;
}

const normalize = (fileName: string) => {
  const name = path.resolve(fileName).replaceAll("\\", "/");
  return process.platform === "win32" ? name.toLowerCase() : name;
};

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
  const edits = assets.map((asset): ShadowEdit => {
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
      end: callStart + closingParen,
      text: `${hasArguments ? "," : ""}undefined as unknown as ${identityType}`,
    };
  });
  for (const resolution of source.resolutions ?? []) {
    edits.push({
      at: byteToUtf16(source.source, resolution.start),
      end: byteToUtf16(source.source, resolution.end),
      text: JSON.stringify(path.resolve(resolution.id).replaceAll("\\", "/")),
    });
  }
  edits.sort((left, right) => left.at - right.at);

  let text = source.source;
  for (const edit of [...edits].reverse()) {
    text = text.slice(0, edit.at) + edit.text + text.slice(edit.end);
  }
  return { text, edits, identities };
}

function shadowToOriginal(
  position: number,
  edits: readonly ShadowEdit[],
  end = false,
): number | null {
  let inserted = 0;
  for (const edit of edits) {
    const shadowStart = edit.at + inserted;
    const shadowEnd = shadowStart + edit.text.length;
    if (position < shadowStart) break;
    // A real argument can end exactly where the hidden identity is inserted.
    // Its end belongs to the original source, not to the inserted argument.
    if (end && position === shadowStart) return edit.at;
    if (position < shadowEnd) return null;
    inserted += edit.text.length - (edit.end - edit.at);
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
  const subtrees: IdentityId[] = [];
  const keyframes: IdentityId[] = [];
  const viewTransitions: IdentityId[] = [];
  let viewTransitionKeyed = false;
  const styles: IdentityId[] = [];

  for (const constituent of constituents) {
    const symbol = constituent.getAliasSymbol() ?? constituent.getSymbol();
    const args = typeArguments(checker, constituent);
    if (symbol?.name === "StyleResult") {
      const candidates = knownIdentities(args[0], identities, "style");
      if (!candidates) return null;
      styles.push(...candidates);
      continue;
    }
    if (symbol?.name === "StyleSubtree") {
      const candidates = knownIdentities(args[0], identities, "scope");
      if (!candidates) return null;
      subtrees.push(...candidates);
      continue;
    }
    if (symbol?.name === "StyleRef") {
      const references = knownIdentities(args[0], identities, "ref");
      if (!references) return null;
      const scopeText = args[1] ? checker.typeToString(args[1]) : "never";
      const permittedSubtrees =
        scopeText === "never" ? [] : knownIdentities(args[1], identities, "scope");
      if (!permittedSubtrees) return null;
      for (const reference of references) {
        refs.push({
          reference,
          permitted_scopes:
            permittedSubtrees.length === 0 ? null : symbolic("scope", permittedSubtrees),
        });
      }
      continue;
    }
    if (symbol?.name === "StyleKeyframes" || symbol?.name === "StyleKeyframesUse") {
      const candidates = knownIdentities(args[0], identities, "animation");
      if (!candidates) return null;
      keyframes.push(...candidates);
      continue;
    }
    if (symbol?.name === "StyleViewTransition" || symbol?.name === "StyleViewTransitionUse") {
      const candidates = knownIdentities(args[0], identities, "view-transition");
      if (!candidates) return null;
      viewTransitions.push(...candidates);
      viewTransitionKeyed ||= symbol.name === "StyleViewTransitionUse";
      continue;
    }
    return null;
  }

  const populated = [refs, subtrees, keyframes, viewTransitions, styles].filter(
    (values) => values.length > 0,
  );
  if (populated.length !== 1) return null;
  if (styles.length > 0) return { kind: "styles", value: symbolic("style", styles) };
  if (refs.length > 0) return { kind: "refs", value: refs };
  if (subtrees.length > 0) return { kind: "scopes", value: symbolic("scope", subtrees) };
  if (keyframes.length > 0) return { kind: "animations", value: symbolic("animation", keyframes) };
  if (viewTransitions.length > 0)
    return viewTransitionKeyed
      ? {
          kind: "view-transition-keys",
          value: symbolic("view-transition", viewTransitions),
        }
      : {
          kind: "view-transitions",
          value: symbolic("view-transition", viewTransitions),
        };
  return null;
}

/** Hidden nominal identities must not discard later, source-valid assignments.
 * Collect every assignment to the checker-resolved binding, irrespective of flow.
 */
function styleFlow(
  checker: Checker,
  sources: readonly SourceFile[],
  identities: ReadonlyMap<string, IdentityId>,
) {
  const definitions: { target: Node; value: Node }[] = [];
  for (const source of sources)
    visit(source, (node) => {
      if (isVariableDeclaration(node) && isIdentifier(node.name) && node.initializer)
        definitions.push({ target: node.name, value: node.initializer });
      if (isFunctionDeclaration(node) && node.name)
        definitions.push({ target: node.name, value: node });
      if (isPropertyAssignment(node))
        definitions.push({ target: node.name, value: node.initializer });
      if (
        isBinaryExpression(node) &&
        node.operatorToken.kind === SyntaxKind.EqualsToken &&
        (isIdentifier(node.left) || isPropertyAccessExpression(node.left))
      )
        definitions.push({ target: node.left, value: node.right });
    });
  const cachedSymbols = new Map(
    checker
      .getSymbolAtLocation(definitions.map(({ target }) => target))
      .map((symbol, index) => [definitions[index].target, symbol]),
  );
  const symbolId = (node: Node) => {
    if (!cachedSymbols.has(node)) cachedSymbols.set(node, checker.getSymbolAtLocation(node));
    let symbol = cachedSymbols.get(node);
    if (symbol && symbol.flags & SymbolFlags.Alias) symbol = checker.getAliasedSymbol(symbol);
    return symbol?.getExportSymbol().id;
  };
  const assignments = new Map<number, Node[]>();
  for (const { target, value } of definitions) {
    const id = symbolId(target);
    if (id === undefined) continue;
    const values = assignments.get(id) ?? [];
    values.push(value);
    assignments.set(id, values);
  }
  const unite = (values: (IdentityId[] | null)[]): IdentityId[] | null => {
    if (values.some((value) => value === null)) return null;
    const unique = new Map(
      values.flatMap((value) => value ?? []).map((identity) => [identityKey(identity), identity]),
    );
    return [...unique.values()];
  };
  const resolve = (
    node: Node,
    pending: Set<number>,
    parameters = new Map<number, Node>(),
  ): IdentityId[] | null => {
    if (isParenthesizedExpression(node)) return resolve(node.expression, pending, parameters);
    if (isConditionalExpression(node))
      return unite([
        resolve(node.whenTrue, pending, parameters),
        resolve(node.whenFalse, pending, parameters),
      ]);
    if (isPropertyAccessExpression(node) && isIdentifier(node.expression)) {
      const objectId = symbolId(node.expression);
      const objects = objectId === undefined ? undefined : assignments.get(objectId);
      if (objects?.every(isObjectLiteralExpression)) {
        const propertyId = symbolId(node);
        if (propertyId !== undefined && pending.has(propertyId)) return null;
        const next = new Set(pending);
        if (propertyId !== undefined) next.add(propertyId);
        const property = node.name.text;
        const values: Node[] = [];
        for (const object of objects)
          for (const entry of object.properties) {
            if (
              isPropertyAssignment(entry) &&
              isIdentifier(entry.name) &&
              entry.name.text === property
            )
              values.push(entry.initializer);
            if (
              isShorthandPropertyAssignment(entry) &&
              isIdentifier(entry.name) &&
              entry.name.text === property
            )
              values.push(entry.name);
          }
        for (const assignment of definitions)
          if (
            isPropertyAccessExpression(assignment.target) &&
            assignment.target.name.text === property &&
            symbolId(assignment.target.expression) === objectId
          )
            values.push(assignment.value);
        return values.length
          ? unite(values.map((value) => resolve(value, next, parameters)))
          : null;
      }
    }
    if (isCallExpression(node) && isIdentifier(node.expression)) {
      const id = symbolId(node.expression);
      const values = id === undefined ? undefined : assignments.get(id);
      if (
        id !== undefined &&
        values?.every(
          (value) =>
            isArrowFunction(value) || isFunctionDeclaration(value) || isFunctionExpression(value),
        )
      ) {
        if (pending.has(id)) return null;
        const results: (IdentityId[] | null)[] = [];
        for (const value of values) {
          if (
            !(
              isArrowFunction(value) ||
              isFunctionDeclaration(value) ||
              isFunctionExpression(value)
            ) ||
            !value.body
          )
            return null;
          const frame = new Map(parameters);
          for (let index = 0; index < value.parameters.length; index++) {
            const parameter = value.parameters[index];
            if (!isIdentifier(parameter.name) || parameter.dotDotDotToken) return null;
            const argument = node.arguments[index] ?? parameter.initializer;
            const parameterId = symbolId(parameter.name);
            if (argument && parameterId !== undefined) frame.set(parameterId, argument);
          }
          const returned: Node[] = [];
          if (!isBlock(value.body)) returned.push(value.body);
          else {
            const collect = (child: Node) => {
              if (
                isArrowFunction(child) ||
                isFunctionDeclaration(child) ||
                isFunctionExpression(child)
              )
                return;
              if (isReturnStatement(child)) {
                if (child.expression) returned.push(child.expression);
                return;
              }
              child.forEachChild(collect);
            };
            value.body.forEachChild(collect);
          }
          results.push(
            ...returned.map((returned) => resolve(returned, new Set(pending).add(id), frame)),
          );
        }
        return unite(results);
      }
    }
    if (isIdentifier(node) || isPropertyAccessExpression(node)) {
      const id = symbolId(node);
      const values =
        id === undefined
          ? undefined
          : [...(assignments.get(id) ?? []), ...(parameters.has(id) ? [parameters.get(id)!] : [])];
      if (id !== undefined && values?.length) {
        if (pending.has(id)) return null;
        const next = new Set(pending).add(id);
        return unite(values.map((value) => resolve(value, next, parameters)));
      }
    }
    const type = checker.getTypeAtLocation(node);
    const value = type && typeFactValue(checker, type, identities);
    return value?.kind === "styles" && value.value.candidates.kind === "finite"
      ? value.value.candidates.values
      : null;
  };
  return (node: Node): NativeExportedValue | null => {
    const candidates = resolve(node, new Set());
    return candidates?.length ? { kind: "styles", value: symbolic("style", candidates) } : null;
  };
}

interface PreparedProject {
  virtualFiles: Map<string, string>;
  mappings: Map<
    string,
    { source: TypeFactSource; edits: ShadowEdit[]; identities: Map<string, IdentityId> }
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
    { source: TypeFactSource; edits: ShadowEdit[]; identities: Map<string, IdentityId> }
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
        directoryExists: (directoryName) => {
          const prefix = normalize(directoryName).replace(/\/$/, "") + "/";
          return [...this.#virtualFiles.keys()].some((name) => name.startsWith(prefix))
            ? true
            : undefined;
        },
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
      if (!this.#virtualFiles.has(fileName))
        (existsSync(fileName) ? changed : created).push(fileName);
      else if (this.#virtualFiles.get(fileName) !== contents) changed.push(fileName);
    }
    for (const fileName of this.#virtualFiles.keys()) {
      if (!prepared.virtualFiles.has(fileName))
        (existsSync(fileName) ? changed : deleted).push(fileName);
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
      const sourceFiles = [...prepared.mappings.values()].map((mapping) => {
        const sourceFile = project.program.getSourceFile(mapping.source.id);
        if (!sourceFile) throw new Error(`TypeScript 7 omitted ${mapping.source.id}`);
        return sourceFile;
      });
      const possibleStyles = styleFlow(project.checker, sourceFiles, prepared.identities);
      const result = new Map<string, TypeFact[]>();
      for (const mapping of prepared.mappings.values()) {
        const sourceFile = project.program.getSourceFile(mapping.source.id);
        if (!sourceFile) throw new Error(`TypeScript 7 omitted ${mapping.source.id}`);
        result.set(
          mapping.source.path,
          factsForSource(project.checker, sourceFile, mapping, prepared.identities, possibleStyles),
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
  mapping: { source: TypeFactSource; edits: ShadowEdit[] },
  identities: ReadonlyMap<string, IdentityId>,
  possibleStyles: (node: Node) => NativeExportedValue | null,
): TypeFact[] {
  const expressions: Node[] = [];
  const probes = new Set(mapping.source.probes.map(({ start, end }) => `${start}:${end}`));
  if (probes.size === 0) return [];
  visit(sourceFile, (node) => {
    if (!isExpression(node)) return;
    const start = shadowToOriginal(node.getStart(sourceFile), mapping.edits);
    const end = shadowToOriginal(node.end, mapping.edits, true);
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
    const checked = typeFactValue(checker, type, identities);
    const value =
      checked?.kind === "styles" || (!checked && type.flags & TypeFlags.Any)
        ? possibleStyles(expressions[index])
        : checked;
    if (!value) continue;
    const start = shadowToOriginal(expressions[index].getStart(sourceFile), mapping.edits);
    const end = shadowToOriginal(expressions[index].end, mapping.edits, true);
    if (start === null || end === null) continue;
    facts.push({
      start: utf16ToByte(mapping.source.source, start),
      end: utf16ToByte(mapping.source.source, end),
      value,
    });
  }
  return facts;
}
