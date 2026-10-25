import type { NativeAssetRecord, NativeLintComment, NativeProjectSession } from "@stillwind/native";
import type { PreparedSource } from "@stillwind/projection";
import ts from "typescript-6";

import type { ModuleAnalysis } from "../../compilation/request.ts";
import { projectionPoints } from "../../semantics/checker-source.ts";

/**
 * Resolves declaration identities and original ranges in a fixed source graph.
 */
export class SourceBindings {
  /** In-memory files keyed by checker filename with slash separators and preserved casing. */
  readonly files = new Map<
    string,
    {
      /** Prepared source that owns this AST and its projection correspondences. */
      source: PreparedSource;
      /** AST parsed from direct source text or the prepared projection. */
      ast: ts.SourceFile;
    }
  >();

  /** Producers keyed first by module identity, then by `start:end` in original UTF-16 coordinates. */
  readonly #assets = new Map<string, Map<string, NativeAssetRecord>>();

  /** Comments with `swlint-*` directives keyed by module identity, from each source's scan. */
  readonly #lintComments = new Map<string, NativeLintComment[]>();

  /** Resolved producers and cached misses for nodes in the captured checker ASTs. */
  readonly #nodeAssets = new WeakMap<ts.Node, NativeAssetRecord | undefined>();

  /** Generated UTF-16 positions mapped to original positions. Null marks conflicting origins. */
  readonly #points = new Map<string, Map<number, number | null>>();

  /** Binder over the in-memory files and the graph's resolved import dependencies. */
  readonly #checker: ts.TypeChecker;

  /** Symbols found in assignment or increment/decrement targets, excluded from stable aliases. */
  readonly #mutated = new Set<ts.Symbol>();

  /**
   * Parses sources, scans declaration records, and binds imports without reading disk.
   *
   * @param graph - Fixed sources and resolved dependencies from one analysis. Do not mutate its sources.
   * @param session - Session used to scan those sources. Capture previews before constructing this binder.
   * @throws If source scanning or projection mapping fails.
   */
  constructor(
    graph: Pick<ModuleAnalysis, "sources" | "dependencies">,
    session: NativeProjectSession,
  ) {
    for (const source of graph.sources) {
      const text = source.kind === "direct" ? source.document.code : source.projection.code;
      const fileName = checkerName(source.checkerFileName);
      const ast = ts.createSourceFile(
        fileName,
        text,
        ts.ScriptTarget.Latest,
        true,
        ts.ScriptKind.TSX,
      );
      this.files.set(fileName, {
        source,
        ast,
      });
      this.#points.set(source.moduleId, projectionPoints(source));
      const scan = session.scanSource(source);
      this.#assets.set(
        source.moduleId,
        assetRanges(source.document.code, scan.kind === "ready" ? scan.summary.assets : []),
      );
      this.#lintComments.set(
        source.moduleId,
        scan.kind === "ready" ? scan.summary.lintComments : [],
      );
    }

    const host: ts.CompilerHost = {
      getSourceFile: (name) => this.files.get(checkerName(name))?.ast,
      getDefaultLibFileName: () => "lib.d.ts",
      writeFile: () => {},
      getCurrentDirectory: () => "/",
      getDirectories: () => [],
      fileExists: (name) => this.files.has(checkerName(name)),
      readFile: (name) => this.files.get(checkerName(name))?.ast.text,
      getCanonicalFileName: checkerName,
      useCaseSensitiveFileNames: () => true,
      getNewLine: () => "\n",
      resolveModuleNames: (names, containingFile) =>
        names.map((name) => {
          const source = this.files.get(checkerName(containingFile))?.source;
          const dependency = graph.dependencies.find(
            (item) => item.importer === source?.resolverId && item.specifier === name,
          );
          const target = graph.sources.find((item) => item.resolverId === dependency?.id);
          return target
            ? { resolvedFileName: checkerName(target.checkerFileName), extension: ts.Extension.Tsx }
            : undefined;
        }),
    };
    const program = ts.createProgram(
      [...this.files.keys()],
      {
        noLib: true,
        allowJs: true,
      },
      host,
    );
    this.#checker = program.getTypeChecker();

    for (const { ast } of this.files.values()) {
      walk(ast, (node) => {
        const target =
          ts.isBinaryExpression(node) &&
          node.operatorToken.kind >= ts.SyntaxKind.FirstAssignment &&
          node.operatorToken.kind <= ts.SyntaxKind.LastAssignment
            ? node.left
            : (ts.isPrefixUnaryExpression(node) || ts.isPostfixUnaryExpression(node)) &&
                [ts.SyntaxKind.PlusPlusToken, ts.SyntaxKind.MinusMinusToken].includes(node.operator)
              ? node.operand
              : undefined;
        if (target) {
          /**
           * Records symbols in assignment targets that can contain identifier bindings.
           * Property accesses do not mark their receiver as a reassigned binding.
           *
           * @param node - Identifier or supported destructuring subtree of the target.
           * Recognized symbols are added to `#mutated`, including nested pattern members.
           */
          const mark = (node: ts.Node): void => {
            if (ts.isIdentifier(node)) {
              const symbol = this.#checker.getSymbolAtLocation(node);
              if (symbol) {
                this.#mutated.add(symbol);
              }
            } else if (
              ts.isArrayLiteralExpression(node) ||
              ts.isObjectLiteralExpression(node) ||
              ts.isPropertyAssignment(node) ||
              ts.isShorthandPropertyAssignment(node) ||
              ts.isSpreadAssignment(node) ||
              ts.isSpreadElement(node) ||
              ts.isParenthesizedExpression(node)
            ) {
              ts.forEachChild(node, mark);
            }
          };
          mark(target);
        }
      });
    }
  }

  /**
   * Recognizes a direct method of an imported Stillwind style producer.
   * Other property accesses with the same spelling do not establish producer authority.
   *
   * @param node - Expression in one of this index's checker ASTs.
   * @param name - Exact method spelling, such as `keyframes` or `viewTransition`.
   * @returns Whether the receiver is a supported value import of `style` and the method matches.
   */
  producer(node: ts.Expression, name: string): boolean {
    return (
      ts.isPropertyAccessExpression(node) &&
      node.name.text === name &&
      this.styleImport(node.expression)
    );
  }

  /**
   * Checks direct named imports of `style` and namespace accesses to that export.
   *
   * @param node - Expression in this index's checker ASTs. Supported imports come from
   * Stillwind's framework and vanilla entrypoints.
   * @returns Whether a value import establishes that producer. Type-only imports and symbols
   * marked as mutated fail this check.
   */
  styleImport(node: ts.Expression): boolean {
    const namespace = ts.isPropertyAccessExpression(node) && node.name.text === "style";
    if (!ts.isIdentifier(node) && !(namespace && ts.isIdentifier(node.expression))) {
      return false;
    }

    const symbol = this.#checker.getSymbolAtLocation(namespace ? node.expression : node);
    if (!symbol || this.#mutated.has(symbol)) {
      return false;
    }

    return !!symbol.declarations?.some((declaration) => {
      if (
        namespace
          ? !ts.isNamespaceImport(declaration)
          : !ts.isImportSpecifier(declaration) ||
            (declaration.propertyName ?? declaration.name).text !== "style" ||
            declaration.isTypeOnly
      ) {
        return false;
      }

      const owner = ts.isNamespaceImport(declaration)
        ? declaration.parent.parent
        : declaration.parent.parent.parent;
      return (
        ts.isImportDeclaration(owner) &&
        owner.importClause?.phaseModifier !== ts.SyntaxKind.TypeKeyword &&
        ts.isStringLiteral(owner.moduleSpecifier) &&
        /^stillwind\/(?:alpine|astro|lit|preact|qwik|react|solid|svelte|vanilla|vue)$/.test(
          owner.moduleSpecifier.text,
        )
      );
    });
  }

  /**
   * Maps a checker node to an exact range in its original document.
   * For projections, an anchor is accepted only when the intervening and selected text match
   * the original buffer. A conflicting mapping at the node's start prevents fallback to an anchor.
   *
   * @param source - Prepared source whose checker AST contains `node`.
   * @param node - Node from this index, with leading trivia excluded from its selected range.
   * @returns Inclusive start and exclusive end in original UTF-16 coordinates, or undefined
   * when a projection cannot map the text exactly.
   */
  range(source: PreparedSource, node: ts.Node): [number, number] | undefined {
    const ast = node.getSourceFile();
    const start = node.getStart(ast);
    const end = node.end;
    if (source.kind === "direct") {
      return [start, end];
    }
    const points = this.#points.get(source.moduleId)!;
    const exact = points.get(start);
    const anchors =
      exact === undefined
        ? [...points]
            .filter(([position, original]) => position <= start && original !== null)
            .sort((a, b) => b[0] - a[0])
        : [[start, exact] as const];
    for (const [position, original] of anchors) {
      if (original === null || original === undefined) {
        continue;
      }
      const offset = original + start - position;
      if (
        source.document.code.slice(original, offset + end - start) === ast.text.slice(position, end)
      ) {
        return [offset, offset + end - start];
      }
    }
    return undefined;
  }

  /**
   * Resolves a native producer or an alias through stable variable initializers and imports.
   * Both successful resolutions and misses are cached for the AST node.
   *
   * @param source - Prepared source whose checker AST contains `node`.
   * @param node - Node from this index. Alias chains may reach another source in the same graph.
   * @returns A shared native record that callers must keep unchanged, or undefined when no
   * producer is found. Mutated bindings and cyclic aliases stop alias resolution.
   */
  asset(source: PreparedSource, node: ts.Node): NativeAssetRecord | undefined {
    if (!this.#nodeAssets.has(node)) {
      this.#nodeAssets.set(node, this.#resolveAsset(source, node, new Set()));
    }
    return this.#nodeAssets.get(node);
  }

  /**
   * Returns the comments that contain `swlint-*` directives in `source`'s original buffer.
   *
   * @param source - Prepared source of this index.
   * @returns Comment bodies in original UTF-8 byte offsets, or an empty list when the scan
   * failed. Callers must keep the returned list unchanged.
   */
  lintComments(source: PreparedSource): NativeLintComment[] {
    return this.#lintComments.get(source.moduleId) ?? [];
  }

  /**
   * Follows a producer range or a stable alias chain without caching intermediate nodes.
   * Exact native producer records take precedence over checks on alias bindings.
   *
   * @param source - Prepared source whose checker AST contains `node`.
   * @param node - Expression to resolve, including supported TypeScript wrappers.
   * @param seen - Symbols already visited in the current chain. Visited symbols are added
   * even when resolution fails, so a separate chain needs a fresh set.
   * @returns The native record reached by the chain, or undefined for mutated, cyclic,
   * missing, or unsupported bindings.
   */
  #resolveAsset(
    source: PreparedSource,
    node: ts.Node,
    seen: Set<ts.Symbol>,
  ): NativeAssetRecord | undefined {
    const range = this.range(source, node);
    if (range) {
      const found = this.#assets.get(source.moduleId)?.get(`${range[0]}:${range[1]}`);
      if (found) {
        return found;
      }
    }
    if (
      ts.isParenthesizedExpression(node) ||
      ts.isAsExpression(node) ||
      ts.isSatisfiesExpression(node) ||
      ts.isNonNullExpression(node)
    ) {
      return this.#resolveAsset(source, node.expression, seen);
    }
    if (ts.isPropertyAccessExpression(node) && ts.isIdentifier(node.expression)) {
      const receiver = this.#checker.getSymbolAtLocation(node.expression);
      if (!receiver?.declarations?.some(ts.isNamespaceImport)) {
        return undefined;
      }
    } else if (!ts.isIdentifier(node)) {
      return undefined;
    }
    let symbol = this.#checker.getSymbolAtLocation(node);
    if (!symbol || this.#mutated.has(symbol) || seen.has(symbol)) {
      return undefined;
    }
    seen.add(symbol);
    if (symbol.flags & ts.SymbolFlags.Alias) {
      symbol = this.#checker.getAliasedSymbol(symbol);
      if (this.#mutated.has(symbol) || seen.has(symbol)) {
        return undefined;
      }
      seen.add(symbol);
    }
    const declaration = symbol.valueDeclaration;
    if (declaration && ts.isVariableDeclaration(declaration) && declaration.initializer) {
      const owner = this.files.get(declaration.getSourceFile().fileName)?.source;
      return owner ? this.#resolveAsset(owner, declaration.initializer, seen) : undefined;
    }
    return undefined;
  }

  /**
   * Finds the smallest mapped AST node that contains an original document position.
   *
   * @param source - Prepared source whose checker filename selects a file in this index.
   * @param offset - Original UTF-16 cursor offset. Containment uses half-open ranges.
   * @returns The containing node, or undefined when the file is absent or no mapped node
   * contains the position. A position at a node's exclusive end does not select that node.
   */
  nodeAt(source: PreparedSource, offset: number): ts.Node | undefined {
    const file = this.files.get(checkerName(source.checkerFileName));
    let best: ts.Node | undefined;
    if (file) {
      walk(file.ast, (node) => {
        const range = this.range(source, node);
        if (
          range &&
          range[0] <= offset &&
          offset < range[1] &&
          (!best || node.end - node.pos < best.end - best.pos)
        ) {
          best = node;
        }
      });
    }
    return best;
  }
}

/**
 * Converts native producer call ranges with one traversal of the original source.
 *
 * @param text - Original document text that owns every producer range.
 * @param assets - Native records whose call ranges use original UTF-8 byte offsets.
 * @returns Records keyed by `start:end`, where both offsets use original UTF-16 coordinates.
 * @throws If a call boundary falls inside a UTF-8 sequence or outside the source.
 */
function assetRanges(
  text: string,
  assets: readonly NativeAssetRecord[],
): Map<string, NativeAssetRecord> {
  const offsets = [...new Set(assets.flatMap((asset) => [asset.callStart, asset.callEnd]))].sort(
    (a, b) => a - b,
  );
  const positions = new Map<number, number>();
  let byte = 0;
  let utf16 = 0;
  for (const offset of offsets) {
    while (byte < offset && utf16 < text.length) {
      const point = text.codePointAt(utf16)!;
      byte += point <= 0x7f ? 1 : point <= 0x7ff ? 2 : point <= 0xffff ? 3 : 4;
      utf16 += point > 0xffff ? 2 : 1;
    }
    if (byte !== offset) {
      throw new Error("Invalid original UTF-8 boundary");
    }
    positions.set(offset, utf16);
  }
  return new Map(
    assets.map((asset) => [
      `${positions.get(asset.callStart)}:${positions.get(asset.callEnd)}`,
      asset,
    ]),
  );
}

/**
 * Provides the separator spelling used by the in-memory checker file map.
 *
 * @param name - Checker filename with either slash or backslash separators.
 * @returns The filename with slash separators. Casing and path segments remain unchanged.
 */
function checkerName(name: string): string {
  return name.replaceAll("\\", "/");
}

/**
 * Visits an AST node before its descendants in TypeScript's child order.
 *
 * @param node - Root of the subtree to traverse.
 * @param visit - Callback for each node. It must preserve the tree during traversal.
 * Returning from the callback does not skip descendants.
 * @throws If the callback throws, traversal stops and propagates that error.
 */
export function walk(node: ts.Node, visit: (node: ts.Node) => void): void {
  visit(node);
  ts.forEachChild(node, (child) => walk(child, visit));
}
