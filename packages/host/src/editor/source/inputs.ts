import type {
  NativeAssetRecord,
  NativeCandidateContext,
  NativeLintComment,
  NativeProjectSession,
} from "@stillwind/native";
import type { PreparedSource } from "@stillwind/projection";
import ts from "typescript-6";

import type { ModuleAnalysis } from "../../compilation/request.ts";
import { GeneratedPreviews, type PreviewSite } from "../previews.ts";
import { SourceBindings, walk } from "./bindings.ts";
import { decodeLiteral, type UtilityLiteral } from "./literal.ts";
import { classifyInput, type InputOrder } from "./order.ts";

/**
 * An array, argument list, or object written directly in a style input.
 */
export interface StyleInputContainer {
  /** Prepared source whose original text supplies the inputs to reorder. */
  source: PreparedSource;
  /** Checker AST node. Use `SourceIndex.range` to obtain original document coordinates. */
  node: ts.ArrayLiteralExpression | ts.CallExpression | ts.ObjectLiteralExpression;
}

/**
 * Indexes utility literals and sortable inputs in a fixed source graph.
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

  /** Binder and source ranges shared by all indexed inputs. */
  readonly #bindings: SourceBindings;

  /**
   * Captures generated previews, rescans graph sources for producer records, and binds aliases.
   * Checker ASTs use projected text when present. Editor ranges address original buffers.
   * Source scanning updates the session's source registry and can replace reports for a failed scan.
   *
   * @param graph - Original buffers and resolved dependencies for the analyzed source revisions.
   * Its prepared sources must remain unchanged while the index is used.
   * @param session - Session whose analyses for modules match these revisions. Captures are taken
   * before rescanning, and later queries use the captured plans and the in-memory ASTs.
   * @param modules - Module IDs successfully compiled from these buffers. Failed scans have no capture.
   * @throws If source transport, a projection map, or an original UTF-8 boundary is invalid.
   */
  constructor(
    graph: Pick<ModuleAnalysis, "sources" | "dependencies">,
    session: NativeProjectSession,
    modules: readonly string[],
  ) {
    this.previews = new GeneratedPreviews(graph, session, modules);
    this.#bindings = new SourceBindings(graph, session);

    for (const { source, ast } of this.#bindings.files.values()) {
      walk(ast, (node) => {
        if (!ts.isCallExpression(node)) {
          return;
        }

        const asset = this.asset(source, node);
        if (asset?.identity.kind === "style" || this.#bindings.styleImport(node.expression)) {
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
        if (
          asset?.identity.kind === "keyframes" ||
          this.#bindings.producer(node.expression, "keyframes")
        ) {
          this.#keyframes(source, node.arguments[0]);
          return;
        }
        if (
          asset?.identity.kind === "viewTransition" ||
          this.#bindings.producer(node.expression, "viewTransition")
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
                this.#literal(source, property.initializer, "", "viewTransitionUtilities");
              }
            }
          }
          return;
        }
        if (this.#bindings.producer(node.expression, "css")) {
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
        if (this.#bindings.producer(node.expression, "media")) {
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
   * Classifies direct input syntax and bound identities without reading base bodies or CSS.
   * Arrays and objects receive a category only when all children share one non-ref category.
   *
   * @param source - Prepared source stored in this index.
   * @param node - Input or property in that source's checker AST.
   * @returns The grouping category, or a barrier for unknown, mixed, empty, or computed input.
   * Ref markers and supported projection calls share the declaration's opaque identity.
   */
  inputOrder(source: PreparedSource, node: ts.Node): InputOrder {
    return classifyInput(source, node, (source, node) => this.asset(source, node));
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
    return this.#bindings.range(source, node);
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
    return this.#bindings.asset(source, node);
  }

  /**
   * Returns the comments that contain `swlint-*` directives in `source`'s original buffer.
   *
   * @param source - Prepared source of this index.
   * @returns Comment bodies in original UTF-8 byte offsets, or an empty list when the scan
   * failed. Callers must keep the returned list unchanged.
   */
  lintComments(source: PreparedSource): NativeLintComment[] {
    return this.#bindings.lintComments(source);
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
   * Finds the smallest mapped AST node that contains an original document position.
   *
   * @param source - Prepared source whose checker filename selects a file in this index.
   * @param offset - Original UTF-16 cursor offset. Containment uses half-open ranges.
   * @returns The containing node, or undefined when the file is absent or no mapped node
   * contains the position. A position at a node's exclusive end does not select that node.
   */
  nodeAt(source: PreparedSource, offset: number): ts.Node | undefined {
    return this.#bindings.nodeAt(source, offset);
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
