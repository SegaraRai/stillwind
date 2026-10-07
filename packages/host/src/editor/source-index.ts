import type { NativeAssetRecord, NativeProjectSession } from "@stillwind/native";
import type { PreparedSource } from "@stillwind/projection";
import ts from "typescript-6";

import type { ModuleAnalysis } from "../compilation/compiler.ts";
import { projectionPoints } from "../semantics/checker-source.ts";
import type { InputOrder } from "./input-order.ts";

/** A literal in a compiler-recognized style input, with decoded-to-source boundaries. */
export interface UtilityLiteral {
  source: PreparedSource;
  node: ts.StringLiteralLike | ts.TemplateLiteralToken;
  text: string;
  /** Variants supplied by surrounding named style groups, including trailing colons. */
  prefix: string;
  /** Original UTF-16 boundaries for each decoded UTF-16 position, including the end. */
  boundaries: number[];
}

/** An array, argument list, or object written directly in a style input. */
export interface StyleInputContainer {
  source: PreparedSource;
  node: ts.ArrayLiteralExpression | ts.CallExpression | ts.ObjectLiteralExpression;
}

/**
 * Indexes producer identities, lexical aliases, and sortable inputs in one source graph.
 * Native producer identities provide authority; the TypeScript binder resolves aliases.
 * The graph and its source buffers must remain unchanged while this index is used.
 */
export class SourceIndex {
  readonly literals: UtilityLiteral[] = [];
  readonly containers: StyleInputContainer[] = [];
  readonly files = new Map<
    string,
    {
      source: PreparedSource;
      ast: ts.SourceFile;
    }
  >();

  /** Original UTF-16 call ranges identify the producers in each module. */
  readonly #assets = new Map<string, Map<string, NativeAssetRecord>>();
  readonly #nodeAssets = new WeakMap<ts.Node, NativeAssetRecord | undefined>();
  readonly #points = new Map<string, Map<number, number | null>>();
  readonly #checker: ts.TypeChecker;
  readonly #mutated = new Set<ts.Symbol>();

  constructor(graph: ModuleAnalysis, session: NativeProjectSession) {
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

    for (const { source, ast } of this.files.values()) {
      walk(ast, (node) => {
        if (!ts.isCallExpression(node)) {
          return;
        }
        const asset = this.asset(source, node);
        if (asset?.identity.kind === "style" || this.#styleImport(node.expression)) {
          if (node.arguments.length > 1) {
            this.containers.push({
              source,
              node,
            });
          }
          for (const argument of node.arguments) {
            this.#input(source, argument);
          }
        }
      });
    }
  }

  #styleImport(node: ts.Expression): boolean {
    const namespace = ts.isPropertyAccessExpression(node) && node.name.text === "style";
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
        /^stillwind\/(?:react|vue|svelte|astro|solid|preact)$/.test(owner.moduleSpecifier.text)
      );
    });
  }

  /** Classifies direct input syntax and bound identities without reading base bodies or CSS. */
  inputOrder(source: PreparedSource, node: ts.Node): InputOrder {
    if (ts.isPropertyAssignment(node) || ts.isShorthandPropertyAssignment(node)) {
      if (ts.isComputedPropertyName(node.name)) {
        return {
          kind: "barrier",
        };
      }
      const name = ts.isStringLiteralLike(node.name) ? node.name.text : node.name.getText();
      if (name.startsWith("--")) {
        return {
          kind: "customProperties",
        };
      }
      if (!name.endsWith(":") && /^(?:aria|data)-/.test(name)) {
        return {
          kind: "attributes",
          name,
        };
      }
      return name.endsWith(":") || name === "class"
        ? {
            kind: "utilities",
          }
        : {
            kind: "barrier",
          };
    }
    if (ts.isStringLiteralLike(node) || ts.isTemplateExpression(node)) {
      return {
        kind: "utilities",
      };
    }
    const asset = this.asset(source, node);
    if (asset?.identity.kind === "style") {
      return {
        kind: "bases",
      };
    }
    if (asset?.identity.kind === "ref") {
      return {
        kind: "ref",
        identity: JSON.stringify(asset.identity),
        projection: false,
      };
    }
    if (
      ts.isCallExpression(node) &&
      ts.isPropertyAccessExpression(node.expression) &&
      ["asAnchor", "asContainer", "positionAnchor"].includes(node.expression.name.text)
    ) {
      const ref = this.asset(source, node.expression.expression);
      if (ref?.identity.kind === "ref") {
        return {
          kind: "ref",
          identity: JSON.stringify(ref.identity),
          projection: true,
        };
      }
    }
    if (
      ts.isParenthesizedExpression(node) ||
      ts.isAsExpression(node) ||
      ts.isSatisfiesExpression(node) ||
      ts.isNonNullExpression(node)
    ) {
      return this.inputOrder(source, node.expression);
    }
    if (ts.isArrayLiteralExpression(node) || ts.isObjectLiteralExpression(node)) {
      const elements = ts.isArrayLiteralExpression(node) ? node.elements : node.properties;
      const orders = elements.map((element) => this.inputOrder(source, element));
      const first = orders[0];
      if (
        first &&
        first.kind !== "barrier" &&
        first.kind !== "ref" &&
        orders.every((order) => order.kind === first.kind)
      ) {
        return first.kind === "attributes"
          ? {
              kind: "attributes",
            }
          : first;
      }
    }
    return {
      kind: "barrier",
    };
  }

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

  asset(source: PreparedSource, node: ts.Node): NativeAssetRecord | undefined {
    if (!this.#nodeAssets.has(node)) {
      this.#nodeAssets.set(node, this.#resolveAsset(source, node, new Set()));
    }
    return this.#nodeAssets.get(node);
  }

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

  #input(source: PreparedSource, node: ts.Node, prefix = ""): void {
    if (ts.isStringLiteralLike(node)) {
      this.#literal(source, node, prefix);
    } else if (ts.isTemplateExpression(node)) {
      this.#literal(source, node.head, prefix);
      for (const span of node.templateSpans) {
        this.#literal(source, span.literal, prefix);
      }
    } else if (ts.isArrayLiteralExpression(node)) {
      this.containers.push({
        source,
        node,
      });
      for (const element of node.elements) {
        this.#input(source, element, prefix);
      }
    } else if (ts.isConditionalExpression(node)) {
      this.#input(source, node.whenTrue, prefix);
      this.#input(source, node.whenFalse, prefix);
    } else if (
      ts.isBinaryExpression(node) &&
      [
        ts.SyntaxKind.AmpersandAmpersandToken,
        ts.SyntaxKind.BarBarToken,
        ts.SyntaxKind.QuestionQuestionToken,
      ].includes(node.operatorToken.kind)
    ) {
      this.#input(source, node.right, prefix);
    } else if (
      ts.isParenthesizedExpression(node) ||
      ts.isAsExpression(node) ||
      ts.isSatisfiesExpression(node)
    ) {
      this.#input(source, node.expression, prefix);
    } else if (ts.isObjectLiteralExpression(node)) {
      this.containers.push({
        source,
        node,
      });
      for (const property of node.properties) {
        if (!ts.isPropertyAssignment(property) || ts.isComputedPropertyName(property.name)) {
          continue;
        }
        const name = ts.isStringLiteralLike(property.name)
          ? property.name.text
          : property.name.getText();
        if (!name.endsWith(":")) {
          continue;
        }
        this.#input(source, property.initializer, prefix + name);
      }
    }
  }

  #literal(source: PreparedSource, node: UtilityLiteral["node"], prefix: string): void {
    const range = this.range(source, node);
    if (!range) {
      return;
    }
    const raw = source.document.code.slice(...range);
    const suffix =
      ts.isTemplateHead(node) || ts.isTemplateMiddle(node) ? 2 : node.isUnterminated ? 0 : 1;
    const content = raw.slice(1, suffix ? -suffix : undefined);
    const decoded = decodeLiteral(content, range[0] + 1);
    if (
      decoded?.text === node.text &&
      !this.literals.some(
        (literal) => literal.source === source && literal.boundaries[0] === decoded.boundaries[0],
      )
    ) {
      this.literals.push({
        source,
        node,
        prefix,
        ...decoded,
      });
    }
  }
}

/** Converts native call boundaries in one source traversal. Invalid UTF-8 boundaries throw. */
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

function checkerName(name: string): string {
  return name.replaceAll("\\", "/");
}

export function walk(node: ts.Node, visit: (node: ts.Node) => void): void {
  visit(node);
  ts.forEachChild(node, (child) => walk(child, visit));
}

/** Decodes JS escapes while retaining the exact source boundaries used by editor replacements. */
function decodeLiteral(
  raw: string,
  start: number,
):
  | {
      text: string;
      boundaries: number[];
    }
  | undefined {
  let text = "";
  const boundaries = [start];
  for (let index = 0; index < raw.length;) {
    const begin = index;
    let value = raw[index++];
    if (value === "\\") {
      const escape = raw[index++];
      if (escape === undefined) {
        return undefined;
      }
      if (escape === "\n" || escape === "\r") {
        if (escape === "\r" && raw[index] === "\n") {
          index++;
        }
        boundaries[boundaries.length - 1] = start + index;
        continue;
      }
      if (escape === "x" || escape === "u") {
        const brace = escape === "u" && raw[index] === "{";
        const end = brace ? raw.indexOf("}", index + 1) : index + (escape === "x" ? 2 : 4);
        const digits = raw.slice(index + (brace ? 1 : 0), end);
        if (end < index || !/^[\da-f]+$/i.test(digits)) {
          return undefined;
        }
        const point = Number.parseInt(digits, 16);
        if (point > 0x10ffff) {
          return undefined;
        }
        value = String.fromCodePoint(point);
        index = end + (brace ? 1 : 0);
      } else {
        const escapes: Record<string, string> = {
          n: "\n",
          r: "\r",
          t: "\t",
          b: "\b",
          f: "\f",
          v: "\v",
          "0": "\0",
        };
        value = escapes[escape] ?? escape;
      }
    } else if (value === "\r") {
      if (raw[index] === "\n") {
        index++;
      }
      value = "\n";
    }
    text += value;
    for (let unit = 1; unit <= value.length; unit++) {
      boundaries.push(start + (unit === value.length ? index : begin));
    }
  }
  return {
    text,
    boundaries,
  };
}
