import type {
  NativeAssetRecord,
  NativeCandidateContext,
  NativeLintComment,
  NativeProjectSession,
} from "@stillwind/native";
import type { PreparedSource } from "@stillwind/projection";
import ts from "typescript-6";

import type { ModuleAnalysis } from "../compilation/compiler.ts";
import { projectionPoints } from "../semantics/checker-source.ts";
import type { InputOrder } from "./input-order.ts";
import { GeneratedPreviews, type PreviewSite } from "./previews.ts";

/** A literal in a compiler-recognized style input, with decoded-to-source boundaries. */
export interface UtilityLiteral {
  /** Prepared source whose original buffer owns all recorded boundaries. */
  source: PreparedSource;
  /** Literal token in the source's checker AST, which may use projected text. */
  node: ts.StringLiteralLike | ts.TemplateLiteralToken;
  /** Decoded literal content without delimiters or enclosing group variants. */
  text: string;
  /** Variants supplied by surrounding named style groups, including trailing colons. */
  prefix: string;
  /** Input role that selects completion registries, validation, and permitted variants. */
  context: NativeCandidateContext;
  /**
   * Captured occurrence whose native plan retains original UTF-8 ranges.
   * Syntactically recognized incomplete input can have a context without a site.
   */
  site?: PreviewSite;
  /**
   * Original UTF-16 boundaries for every position in `text`, including its end.
   * The array has `text.length + 1` entries. One escape that emits several code units
   * can give several decoded positions the same source boundary.
   */
  boundaries: readonly number[];
}

/** An array, argument list, or object written directly in a style input. */
export interface StyleInputContainer {
  /** Prepared source whose original text supplies the inputs to reorder. */
  source: PreparedSource;
  /** Checker AST node. Use `SourceIndex.range` to obtain original document coordinates. */
  node: ts.ArrayLiteralExpression | ts.CallExpression | ts.ObjectLiteralExpression;
}

/**
 * Indexes producer identities, lexical aliases, and sortable inputs in one source graph.
 * Native producer identities provide authority; the TypeScript binder resolves aliases.
 * Prepared sources and checker ASTs must remain unchanged while this index is used.
 */
export class SourceIndex {
  /** Generated occurrences and replacement plans captured from the same source graph. */
  readonly previews: GeneratedPreviews;
  /** Indexed literals in discovery order, with fixed source boundaries. */
  readonly literals: readonly Readonly<UtilityLiteral>[];
  readonly #literals: UtilityLiteral[] = [];
  /** Fixed direct inputs eligible for grouping, including nested containers. */
  readonly containers: readonly Readonly<StyleInputContainer>[];
  readonly #containers: StyleInputContainer[] = [];
  /** In-memory files keyed by checker filename with slash separators and preserved casing. */
  readonly #files = new Map<
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
   * Captures generated previews, rescans graph sources for producer records, and binds aliases.
   * Checker ASTs use projected text when present. Editor ranges address original buffers.
   * Source scanning updates the session's source registry and can replace reports for a failed scan.
   *
   * @param graph - Original buffers and resolved dependencies for the analyzed source revisions.
   * Its prepared sources must remain unchanged while the index is used.
   * @param session - Session whose latest analyses match those revisions. Captures are taken
   * before rescanning, and later queries use the captured plans and the in-memory ASTs.
   * @throws If source transport, a projection map, or an original UTF-8 boundary is invalid.
   */
  constructor(
    graph: Pick<ModuleAnalysis, "sources" | "dependencies">,
    session: NativeProjectSession,
  ) {
    this.previews = new GeneratedPreviews(graph, session);

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
      this.#files.set(fileName, {
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
      getSourceFile: (name) => this.#files.get(checkerName(name))?.ast,
      getDefaultLibFileName: () => "lib.d.ts",
      writeFile: () => {},
      getCurrentDirectory: () => "/",
      getDirectories: () => [],
      fileExists: (name) => this.#files.has(checkerName(name)),
      readFile: (name) => this.#files.get(checkerName(name))?.ast.text,
      getCanonicalFileName: checkerName,
      useCaseSensitiveFileNames: () => true,
      getNewLine: () => "\n",
      resolveModuleNames: (names, containingFile) =>
        names.map((name) => {
          const source = this.#files.get(checkerName(containingFile))?.source;
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
      [...this.#files.keys()],
      {
        noLib: true,
        allowJs: true,
      },
      host,
    );
    this.#checker = program.getTypeChecker();

    for (const { ast } of this.#files.values()) {
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

    for (const { source, ast } of this.#files.values()) {
      walk(ast, (node) => {
        if (!ts.isCallExpression(node)) {
          return;
        }

        const asset = this.asset(source, node);
        if (asset?.identity.kind === "style" || this.#styleImport(node.expression)) {
          if (node.arguments.length > 1) {
            this.#containers.push({
              source,
              node,
            });
          }
          for (const argument of node.arguments) {
            this.#input(source, argument);
          }
          return;
        }
        if (asset?.identity.kind === "keyframes" || this.#producer(node.expression, "keyframes")) {
          this.#keyframes(source, node.arguments[0]);
          return;
        }
        if (
          asset?.identity.kind === "viewTransition" ||
          this.#producer(node.expression, "viewTransition")
        ) {
          const body = node.arguments[0];
          if (body && ts.isObjectLiteralExpression(body)) {
            for (const property of body.properties) {
              if (!ts.isPropertyAssignment(property) || ts.isComputedPropertyName(property.name)) {
                continue;
              }
              const name = ts.isStringLiteralLike(property.name)
                ? property.name.text
                : property.name.getText();
              if (
                ["group", "old", "new", "imagePair"].includes(name) &&
                ts.isStringLiteralLike(property.initializer)
              ) {
                this.#literal(
                  source,
                  property.initializer,
                  "",
                  name === "group" ? "viewTransitionOptions" : "viewTransitionUtilities",
                );
              }
            }
          }
          return;
        }
        if (this.#producer(node.expression, "css")) {
          const inputs = [...node.arguments];
          const options = inputs.at(-1);
          const anchored =
            options !== undefined &&
            isCssOptions(options) &&
            options.properties.some(
              (property) =>
                ts.isPropertyAssignment(property) && propertyName(property) === "selector",
            );
          if (options !== undefined && isCssOptions(options)) {
            inputs.pop();
            for (const property of options.properties) {
              if (
                ts.isPropertyAssignment(property) &&
                propertyName(property) === "template" &&
                ts.isStringLiteralLike(property.initializer)
              ) {
                this.#literal(source, property.initializer, "", "cssTemplate");
              }
            }
          }
          for (const input of inputs) {
            this.#cssInput(source, input, "", anchored ? "anchoredDocument" : "document");
          }
          return;
        }
        if (this.#producer(node.expression, "media")) {
          for (const argument of node.arguments) {
            if (ts.isStringLiteralLike(argument)) {
              this.#literal(source, argument, "", "mediaVariants");
            }
          }
          return;
        }
        if (this.asset(source, node.expression)?.identity.kind === "keyframes") {
          const options = node.arguments[0];
          if (options && ts.isStringLiteralLike(options)) {
            this.#literal(source, options, "", "animationOptions");
          }
        }
      });

      walk(ast, (node) => {
        if (!ts.isStringLiteralLike(node) && !ts.isTemplateLiteralToken(node)) {
          return;
        }
        const range = this.range(source, node);
        const site = range && this.previews.input(source, ...range);
        if (site?.native.context) {
          this.#literal(source, node, "", site.native.context, site);
        }
      });
    }

    this.literals = this.#literals;
    this.containers = this.#containers;
  }

  /**
   * Recognizes a direct method of an imported Stillwind style producer.
   * Other property accesses with the same spelling do not establish producer authority.
   *
   * @param node - Expression in one of this index's checker ASTs.
   * @param name - Exact method spelling, such as `keyframes` or `viewTransition`.
   * @returns Whether the receiver is a supported value import of `style` and the method matches.
   */
  #producer(node: ts.Expression, name: string): boolean {
    return (
      ts.isPropertyAccessExpression(node) &&
      node.name.text === name &&
      this.#styleImport(node.expression)
    );
  }

  /**
   * Indexes literal frame utilities and quoted condition keys in an authored keyframes body.
   * Nested object bodies retain definition contexts. Computed properties, nonliteral frame
   * values, and missing bodies add no literals.
   *
   * @param source - Prepared source stored in this index.
   * @param node - Body in that source's checker AST, or undefined for a missing argument.
   * Recognized quoted conditions and frame utilities are appended to `literals`.
   */
  #keyframes(source: PreparedSource, node: ts.Node | undefined): void {
    if (!node || !ts.isObjectLiteralExpression(node)) {
      return;
    }

    for (const property of node.properties) {
      if (!ts.isPropertyAssignment(property) || ts.isComputedPropertyName(property.name)) {
        continue;
      }

      const name = ts.isStringLiteralLike(property.name)
        ? property.name.text
        : property.name.getText();
      if (name.endsWith(":") || ts.isObjectLiteralExpression(property.initializer)) {
        if (ts.isStringLiteralLike(property.name)) {
          this.#literal(source, property.name, "", "definitionVariants");
        }
        this.#keyframes(source, property.initializer);
      } else if (
        /^(?:from|to|(?:\d+(?:\.\d+)?%)(?:\s*,\s*\d+(?:\.\d+)?%)*)$/.test(name) &&
        ts.isStringLiteralLike(property.initializer)
      ) {
        this.#literal(source, property.initializer, "", "keyframesFrame");
      }
    }
  }

  /**
   * Checks direct named imports of `style` and namespace accesses to that export.
   *
   * @param node - Expression in this index's checker ASTs. Supported imports come from
   * Stillwind's React, Vue, Svelte, Astro, Solid, or Preact entry points.
   * @returns Whether a value import establishes that producer. Type-only imports and symbols
   * marked as mutated fail this check.
   */
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

  /**
   * Classifies direct input syntax and bound identities without reading base bodies or CSS.
   * Arrays and objects receive a category only when all children share one non-ref category.
   *
   * @param source - Prepared source stored in this index.
   * @param node - Input or property in that source's checker AST.
   * @returns The grouping category, or a barrier for unknown, mixed, empty, or computed input.
   * Ref markers and supported projection calls share the declaration's opaque identity.
   */
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
      return name === "" || name.endsWith(":") || name === "class"
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
        return first;
      }
    }
    return {
      kind: "barrier",
    };
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
   * Finds a captured expression contribution at `node` in `source`'s checker AST.
   * Exact non-input ranges take precedence; calls then try their receiver's use site.
   * Invalid sites remain identifiable.
   *
   * @param source - Prepared source whose checker AST contains `node`.
   * @param node - Expression or call in this index.
   * @returns A shared captured occurrence, or undefined for unmapped or unrecognized expressions.
   * Callers must keep the returned site unchanged.
   */
  previewSite(source: PreparedSource, node: ts.Node): PreviewSite | undefined {
    const range = this.range(source, node);
    if (!range) {
      return undefined;
    }

    const sites = this.previews.sites(source);
    const exact = sites.find(
      (site) => site.native.kind !== "input" && site.start === range[0] && site.end === range[1],
    );
    if (exact || !ts.isCallExpression(node)) {
      return exact;
    }

    const receiver = ts.isPropertyAccessExpression(node.expression)
      ? node.expression.expression
      : node.expression;
    const receiverRange = this.range(source, receiver);
    return receiverRange
      ? sites.find(
          (site) =>
            site.native.kind === "use" &&
            site.start === receiverRange[0] &&
            site.end === receiverRange[1],
        )
      : undefined;
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
      const owner = this.#files.get(declaration.getSourceFile().fileName)?.source;
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
    const file = this.#files.get(checkerName(source.checkerFileName));
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

  /**
   * Indexes the static utility literals of one `style.css()` input with their group keys.
   * Runtime values and computed keys add no literals.
   *
   * @param source - Prepared source stored in this index.
   * @param node - Input in that source's checker AST.
   * @param prefix - Variants of the enclosing groups, each ending in `:`.
   * @param context - Input role of the call: `anchoredDocument` when it has a `selector`
   * option, and `document` otherwise.
   */
  #cssInput(
    source: PreparedSource,
    node: ts.Node,
    prefix: string,
    context: "document" | "anchoredDocument",
  ): void {
    if (ts.isStringLiteralLike(node)) {
      this.#literal(source, node, prefix, context);
      return;
    }
    if (ts.isArrayLiteralExpression(node)) {
      for (const element of node.elements) {
        this.#cssInput(source, element, prefix, context);
      }
      return;
    }
    if (
      ts.isParenthesizedExpression(node) ||
      ts.isAsExpression(node) ||
      ts.isSatisfiesExpression(node)
    ) {
      this.#cssInput(source, node.expression, prefix, context);
      return;
    }
    if (ts.isObjectLiteralExpression(node)) {
      for (const property of node.properties) {
        if (!ts.isPropertyAssignment(property) || ts.isComputedPropertyName(property.name)) {
          continue;
        }
        const name = propertyName(property);
        if (name === "" || name.endsWith(":")) {
          this.#cssInput(
            source,
            property.initializer,
            name === ":" ? prefix : prefix + name,
            context,
          );
        }
      }
    }
  }

  /**
   * Records literals and sortable containers in recognized direct style input syntax.
   * Arrays and both conditional branches are traversed. Logical `&&`, `||`, and `??`
   * expressions contribute only their right operand, and unrecognized syntax adds nothing.
   *
   * @param source - Prepared source whose checker AST contains `node`.
   * @param node - Input expression to inspect. Object properties ending in `:` contribute
   * their values under the accumulated group variants. The keys `""` and `":"` add no variants.
   * @param prefix - Decoded enclosing variants, including trailing colons. Nested group
   * names are appended before their literals are recorded.
   */
  #input(source: PreparedSource, node: ts.Node, prefix = ""): void {
    if (ts.isStringLiteralLike(node)) {
      this.#literal(source, node, prefix);
      return;
    }
    if (ts.isTemplateExpression(node)) {
      this.#literal(source, node.head, prefix);
      for (const span of node.templateSpans) {
        this.#literal(source, span.literal, prefix);
      }
      return;
    }
    if (ts.isArrayLiteralExpression(node)) {
      this.#containers.push({
        source,
        node,
      });
      for (const element of node.elements) {
        this.#input(source, element, prefix);
      }
      return;
    }
    if (ts.isConditionalExpression(node)) {
      this.#input(source, node.whenTrue, prefix);
      this.#input(source, node.whenFalse, prefix);
      return;
    }
    if (
      ts.isBinaryExpression(node) &&
      [
        ts.SyntaxKind.AmpersandAmpersandToken,
        ts.SyntaxKind.BarBarToken,
        ts.SyntaxKind.QuestionQuestionToken,
      ].includes(node.operatorToken.kind)
    ) {
      this.#input(source, node.right, prefix);
      return;
    }
    if (
      ts.isParenthesizedExpression(node) ||
      ts.isAsExpression(node) ||
      ts.isSatisfiesExpression(node)
    ) {
      this.#input(source, node.expression, prefix);
      return;
    }
    if (ts.isElementAccessExpression(node) || ts.isPropertyAccessExpression(node)) {
      this.#table(source, node.expression, prefix);
      return;
    }
    if (ts.isObjectLiteralExpression(node)) {
      this.#containers.push({
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
        if (name !== "" && !name.endsWith(":")) {
          continue;
        }
        this.#input(source, property.initializer, name === ":" ? prefix : prefix + name);
      }
    }
  }

  /**
   * Records the literal values of an array or object literal that a style input indexes.
   * Literal spreads are traversed, and keys are not utilities. A table is not a sortable
   * container, because reordering its entries would change which entry an index selects.
   *
   * @param source - Prepared source whose checker AST contains `node`.
   * @param node - Receiver of the lookup. Other receivers add nothing.
   * @param prefix - Decoded enclosing group variants, including trailing colons.
   */
  #table(source: PreparedSource, node: ts.Node, prefix: string): void {
    while (
      ts.isParenthesizedExpression(node) ||
      ts.isAsExpression(node) ||
      ts.isSatisfiesExpression(node)
    ) {
      node = node.expression;
    }
    const values: ts.Node[] = ts.isArrayLiteralExpression(node) ? [...node.elements] : [];
    if (ts.isObjectLiteralExpression(node)) {
      for (const property of node.properties) {
        if (ts.isPropertyAssignment(property)) {
          values.push(property.initializer);
        } else if (ts.isSpreadAssignment(property)) {
          this.#table(source, property.expression, prefix);
        }
      }
    }
    for (const value of values) {
      if (ts.isSpreadElement(value)) {
        this.#table(source, value.expression, prefix);
      } else if (ts.isStringLiteralLike(value)) {
        this.#literal(source, value, prefix);
      }
    }
  }

  /**
   * Records decoded content and original UTF-16 boundaries for a literal in `source`'s AST.
   * A captured `site` overrides an existing entry's context and plan at the same content start.
   * Unmapped ranges or decoding that disagrees with the AST add nothing.
   *
   * @param source - Prepared source whose checker AST contains `node`.
   * @param node - String or template token whose raw spelling supplies the source boundaries.
   * @param prefix - Decoded enclosing group variants, including trailing colons.
   * @param context - Input role stored with a new entry. Defaults to ordinary utilities.
   * @param site - Optional captured input. An existing entry keeps its prefix and boundaries
   * when this site replaces its context and plan.
   */
  #literal(
    source: PreparedSource,
    node: UtilityLiteral["node"],
    prefix: string,
    context: NativeCandidateContext = "utility",
    site?: PreviewSite,
  ): void {
    const range = this.range(source, node);
    if (!range) {
      return;
    }

    const raw = source.document.code.slice(...range);
    const suffix =
      ts.isTemplateHead(node) || ts.isTemplateMiddle(node) ? 2 : node.isUnterminated ? 0 : 1;
    const content = raw.slice(1, suffix ? -suffix : undefined);
    const decoded = decodeLiteral(content, range[0] + 1);
    if (decoded?.text === node.text) {
      const previous = this.#literals.find(
        (literal) => literal.source === source && literal.boundaries[0] === decoded.boundaries[0],
      );
      if (previous) {
        if (site) {
          previous.context = context;
          previous.site = site;
        }
        return;
      }

      this.#literals.push({
        source,
        node,
        prefix,
        context,
        site,
        ...decoded,
      });
    }
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

/**
 * Decodes JS literal content while mapping every decoded position to its source spelling.
 * CR and CRLF become LF. Escaped line continuations emit no text and move the preceding
 * boundary past the continuation. The caller must compare the result with the AST's decoded text.
 *
 * @param raw - Literal content without its opening delimiter or closing delimiter/interpolation.
 * @param start - Original UTF-16 offset of the first code unit in `raw`.
 * @returns Decoded text and `text.length + 1` source boundaries, or undefined for a trailing
 * backslash, invalid hex digits, a missing Unicode brace, or a code point above U+10FFFF.
 */
function decodeLiteral(
  raw: string,
  start: number,
):
  | {
      /** Decoded content. Literal line endings are normalized to LF. */
      text: string;
      /** Original UTF-16 offsets for every decoded position, including the end. */
      boundaries: readonly number[];
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

/**
 * Returns the written name of a property with a literal or identifier key.
 *
 * @param property - Property assignment whose key is not computed.
 * @returns The decoded key text.
 */
function propertyName(property: ts.PropertyAssignment): string {
  return ts.isStringLiteralLike(property.name) ? property.name.text : property.name.getText();
}

/**
 * Recognizes the options object of `style.css()`: an object literal whose properties
 * all have the name `template` or `selector`.
 *
 * @param node - Last argument of a `style.css()` call.
 * @returns Whether the compiler reads the argument as options rather than an input.
 */
function isCssOptions(node: ts.Node): node is ts.ObjectLiteralExpression {
  return (
    ts.isObjectLiteralExpression(node) &&
    node.properties.length > 0 &&
    node.properties.every(
      (property) =>
        ts.isPropertyAssignment(property) &&
        !ts.isComputedPropertyName(property.name) &&
        ["template", "selector"].includes(propertyName(property)),
    )
  );
}
