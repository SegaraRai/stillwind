import type { PreviewOptions } from "@stillwind/config";
import type { NativeCandidateContext, NativeEditorQuery } from "@stillwind/native";
import type { PreparedSource } from "@stillwind/projection";
import ts from "typescript-6";

import type { PluginRuntime } from "../../css/plugins/runtime.ts";
import { CandidateQueries, markerDocumentation, segments } from "../candidates.ts";
import { colorReplacement } from "../colors.ts";
import { completionSnippet, markerModifierSnippet } from "../completion.ts";
import type { GeneratedPreview } from "../previews.ts";
import type { EditorRequest, EditorResult } from "../request.ts";
import { escapeContent } from "../sort.ts";
import { SourceIndex } from "./inputs.ts";
import type { UtilityLiteral } from "./literal.ts";

/** Most catalog candidates that one completion request adds. */
const CATALOG_COMPLETIONS = 200;

/**
 * Answers source completion, hover, and color queries using a captured editor snapshot.
 */
export class SourceQueries {
  /** Checker ASTs, stable aliases, and captured emission plans for the graph's sources. */
  readonly #index: SourceIndex;

  /** Native design system and effective lint policy captured during construction. */
  readonly #query: NativeEditorQuery;

  /** Candidate caches shared by all queries against this snapshot. */
  readonly #candidates: CandidateQueries;

  /** Dynamic-family candidates that plugins list for completion, sorted. */
  readonly #catalog: readonly string[];

  /** Plugin suggestions for editor inputs that {@link SourceQueries.prepare} loaded, keyed by input. */
  readonly #inputCatalogs = new Map<string, readonly string[]>();

  /** Fingerprint of captures, graph dependencies, and configuration used for lazy resolution. */
  readonly #revision: string;

  /** Comments added to formatted CSS previews. Omitted fields use native defaults. */
  readonly #annotations: PreviewOptions;

  /** Queries that include callback CSS for selected completions in this snapshot. */
  readonly #resolved = new Map<string, CandidateQueries>();

  /** Resolves completion CSS separately from the captured compiler session. */
  readonly #plugins: PluginRuntime | undefined;

  /**
   * Shares the snapshot's source index and candidate caches for completion, hover, and color queries.
   *
   * @param index - Fixed source graph and captured preview plans.
   * @param query - Native query for the same CSS definitions.
   * @param candidates - Candidate caches also used by sorting.
   * @param revision - Snapshot fingerprint copied into lazy completion requests.
   * @param annotations - Comments to add to CSS previews.
   * @param catalog - Plugin candidates available without callback execution.
   * @param plugins - Optional plugin runtime used for lazy suggestions and selected candidates.
   */
  constructor(
    index: SourceIndex,
    query: NativeEditorQuery,
    candidates: CandidateQueries,
    revision: string,
    annotations: PreviewOptions,
    catalog: readonly string[],
    plugins?: PluginRuntime,
  ) {
    this.#index = index;
    this.#query = query;
    this.#candidates = candidates;
    this.#revision = revision;
    this.#annotations = annotations;
    this.#catalog = catalog;
    this.#plugins = plugins;
  }

  /**
   * Loads plugin suggestions for a completion input, and callback-generated CSS for a selected
   * completion from this captured snapshot, so that a following {@link SourceQueries.query} can
   * use them. Other requests are ignored.
   *
   * @param source - Fixed source for this request, or undefined for an unknown document.
   * @param request - Editor request. A `completion` request loads the plugin suggestions for the
   * candidate at its cursor. A `resolve` request for this snapshot's revision loads CSS, once for
   * each candidate.
   * @throws If a plugin fails or the native loader rejects the generated CSS.
   */
  async prepare(source: PreparedSource | undefined, request: EditorRequest): Promise<void> {
    if (request.kind === "completion" && source && this.#plugins) {
      const input = this.#completionInput(source, request.offset);
      const partial =
        input && this.#catalogContext(input.literal.context)
          ? segments(input.literal.prefix + input.prefix, ":").pop()!
          : "";
      if (partial && !this.#inputCatalogs.has(partial)) {
        this.#inputCatalogs.set(partial, await this.#plugins.catalog(partial));
      }
    }
    if (
      request.kind === "resolve" &&
      request.snapshot === this.#revision &&
      this.#plugins &&
      !this.#resolved.has(request.candidate)
    ) {
      this.#resolved.set(
        request.candidate,
        new CandidateQueries(await this.#plugins.query(request.candidate)),
      );
    }
  }

  /**
   * Answers a source completion, hover, resolution, or color request.
   *
   * @param source - Fixed source from this query's index.
   * @param request - Request with offsets in the original UTF-16 document.
   * @param result - Empty answer for the same source. This query fills its data fields.
   * @returns The supplied answer. Queries may fill caches but do not edit source text.
   * @throws If candidate resolution, CSS formatting, or native color queries fail.
   */
  query(
    source: PreparedSource,
    request: Exclude<EditorRequest, { kind: "sort" | "fixes" | "definition" | "links" }>,
    result: EditorResult,
  ): EditorResult {
    if (request.kind === "resolve") {
      const literal = this.#index.literals.find(
        (literal) => literal.source === source && literal.boundaries[0] === request.input,
      );
      const candidates = this.#resolved.get(request.candidate) ?? this.#candidates;
      const current = literal && request.snapshot === this.#revision;
      const css = current
        ? request.candidate.endsWith(":")
          ? candidates.variant(request.candidate, literal.context)
          : (this.#preview(literal, request.candidate.slice(literal.prefix.length)) ??
            candidates.preview(request.candidate, literal.context))
        : undefined;
      const marker =
        current && !literal.site?.native.enclosingVariant
          ? candidates.marker(request.candidate, literal.context)
          : null;
      if (css || marker) {
        const formatted = css ? this.#query.formatCss(css, this.#annotations) : "";
        result.hover = {
          start: 0,
          end: 0,
          css: formatted,
          detail: css ? this.#candidates.detail(formatted) : "marker class",
          ...(marker
            ? {
                documentation: markerDocumentation(marker),
              }
            : {}),
        };
      }
      return result;
    }

    if (request.kind === "previews") {
      result.previews = this.#index.previews
        .sites(source)
        .filter(
          (site) =>
            site.native.kind !== "input" &&
            site.native.kind !== "variant" &&
            site.native.kind !== "media",
        )
        .flatMap((site) => {
          const preview = this.#index.previews.assemble(site.native);
          const range = {
            start: site.start,
            end: site.end,
          };
          if (!preview) {
            return [
              {
                ...range,
                css: "",
                valid: false,
              },
            ];
          }
          return [
            {
              ...range,
              css: preview.css ? this.#query.formatCss(preview.css, this.#annotations) : "",
              valid: true,
              ...(preview.runtimeNames.length ? { runtimeNames: preview.runtimeNames } : {}),
            },
          ];
        })
        .sort((a, b) => a.start - b.start || b.end - a.end);
      let end = -1;
      // An enclosing occurrence includes the CSS of the references inside it.
      result.previews = result.previews.filter((preview) => {
        const outer = preview.end > end;
        end = Math.max(end, preview.end);
        return outer;
      });
      return result;
    }

    const tokens = this.#sourceTokens(source);
    if (request.kind === "colors") {
      result.colors = tokens.flatMap((token) => {
        if (
          !this.#colorInput(token.literal) ||
          !this.#preview(token.literal, token.text, token.site)
        ) {
          return [];
        }
        return this.#candidates
          .colors(token.literal.prefix + token.text)
          .map(({ scheme, ...color }) => ({
            start: token.start,
            end: token.end,
            ...color,
            ...(scheme ? { scheme } : {}),
          }));
      });
      return result;
    }

    if (request.kind === "colorPresentation") {
      const token = tokens.find(
        (token) => token.start === request.start && token.end === request.end,
      );
      if (!token || !this.#colorInput(token.literal)) {
        return result;
      }

      const candidate = colorReplacement(
        token.text,
        token.literal.prefix,
        request.color,
        this.#candidates,
      );
      if (candidate && this.#preview(token.literal, candidate, token.site)) {
        result.edits = [
          {
            start: token.start,
            end: token.end,
            newText: escapeContent(candidate, token.literal),
          },
        ];
      }
      return result;
    }

    const token = tokens.find(
      (token) => token.start <= request.offset && request.offset <= token.end,
    );
    if (request.kind === "hover") {
      const variant = this.#variantPreview(source, request.offset);
      if (variant !== false) {
        result.hover = variant;
        return result;
      }
      if (token) {
        const candidate = token.literal.prefix + token.text;
        const css = token.literal.key
          ? this.#candidates.variant(candidate, token.literal.context)
          : this.#preview(token.literal, token.text, token.site);
        const marker =
          token.literal.key || token.site?.native.enclosingVariant
            ? null
            : this.#candidates.marker(candidate, token.literal.context);
        if (css || marker) {
          result.hover = {
            start: token.start,
            end: token.end,
            css: css ? this.#query.formatCss(css, this.#annotations) : "",
            ...(marker
              ? {
                  documentation: markerDocumentation(marker),
                }
              : {}),
          };
        }
      } else {
        result.hover = this.#expressionPreview(source, request.offset);
      }
      return result;
    }

    const input = this.#completionInput(source, request.offset);
    if (!input) {
      return result;
    }

    const { literal, start, end, prefix } = input;
    const native = this.#candidates
      .complete(literal.prefix + prefix, literal.context, request.snippets === true)
      .filter(({ label, marker }) =>
        literal.key
          ? !marker && label.endsWith(":")
          : (marker && !literal.site?.native.enclosingVariant) ||
            label.endsWith(":") ||
            !literal.site ||
            this.#preview(literal, label.slice(literal.prefix.length)),
      );
    const known = new Set(native.map(({ label }) => label));
    // A variant key accepts no utility, so the plugin catalog of dynamic utilities is skipped.
    const { names: catalog, incomplete } = literal.key
      ? { names: [], incomplete: false }
      : this.#catalogCompletions(literal.prefix + prefix, literal.context);
    if (incomplete) {
      result.completionsIncomplete = true;
    }
    result.completions = [
      ...native,
      ...catalog
        .filter((candidate) => !known.has(candidate))
        .map((label) => ({
          label,
          marker: false,
        })),
    ].map(({ label: candidate, marker }) => {
      const label = candidate.slice(literal.prefix.length);
      const source = (text: string) => escapeContent(text, literal);
      const color = marker || candidate.endsWith(":") ? undefined : this.#color(candidate);
      return {
        label,
        candidate,
        input: literal.boundaries[0],
        snapshot: this.#revision,
        start,
        end,
        newText: escapeContent(label, literal),
        ...(marker ? { marker } : {}),
        ...(color ? { color } : {}),
        snippet: !request.snippets
          ? undefined
          : marker && candidate.endsWith("/")
            ? markerModifierSnippet(label, source)
            : candidate.endsWith(":")
              ? completionSnippet(label, source)
              : undefined,
        retrigger: candidate.endsWith(":") || candidate.endsWith("-"),
      };
    });

    return result;
  }

  /**
   * Finds formatted CSS at an original UTF-16 `offset`, walking ancestors through the first call.
   * A captured occurrence takes precedence over a declaration reached through an alias. Its
   * missing or invalid output ends the search, preventing an enclosing partial preview.
   * Without a captured occurrence, the nearest expression with a resolvable declaration supplies
   * the fallback preview and range.
   *
   * @param source - Prepared source stored in this snapshot's index.
   * @param offset - Cursor offset in the original document's UTF-16 coordinates.
   * @returns Formatted CSS and its original range, or undefined when no complete preview is available.
   * @throws If the native formatter rejects the assembled CSS.
   */
  #expressionPreview(source: PreparedSource, offset: number): EditorResult["hover"] {
    let node = this.#index.nodeAt(source, offset);
    let reference:
      | {
          /** Declaration preview reached through the nearest resolvable expression. */
          preview: GeneratedPreview;
          /** Inclusive start and exclusive end of that expression in original UTF-16 coordinates. */
          range: readonly [number, number];
        }
      | undefined;
    while (node) {
      const site = this.#index.previewSite(source, node);
      if (site && (!reference || site.native.kind === "use")) {
        const preview = this.#index.previews.preview(site.native);
        const range = this.#index.range(source, node);
        return preview && range ? this.#formatExpressionPreview(preview, range) : undefined;
      }
      if (!reference) {
        const asset = this.#index.asset(source, node);
        let preview = asset && this.#index.previews.previewIdentity(asset.identity);
        if (!preview) {
          const init = this.#index.initializer(source, node);
          if (init) {
            const initSite = this.#index.previewSite(init.source, init.node);
            if (initSite) {
              preview = this.#index.previews.preview(initSite.native);
            } else {
              const initRange = this.#index.range(init.source, init.node);
              const variantSite =
                initRange &&
                this.#index.previews
                  .sites(init.source)
                  .find(
                    (site) =>
                      site.native.kind === "variant" &&
                      initRange[0] <= site.start &&
                      site.end <= initRange[1],
                  );
              preview = variantSite && this.#index.previews.preview(variantSite.native);
            }
          }
        }
        const range = this.#index.range(source, node);
        if (preview && range) {
          reference = {
            preview,
            range,
          };
        }
      }
      if (ts.isCallExpression(node)) {
        break;
      }
      node = node.parent;
    }
    return reference
      ? this.#formatExpressionPreview(reference.preview, reference.range)
      : undefined;
  }

  /**
   * Formats a resolved expression preview into an editor hover payload.
   *
   * @param preview - Assembled CSS or lowered JavaScript preview.
   * @param range - Inclusive start and exclusive end in original UTF-16 coordinates.
   * @returns Hover payload for CSS or JavaScript output.
   * @throws If the native formatter rejects the CSS preview.
   */
  #formatExpressionPreview(
    preview: GeneratedPreview,
    range: readonly [number, number],
  ): NonNullable<EditorResult["hover"]> {
    if (preview.js) {
      return {
        start: range[0],
        end: range[1],
        css: "",
        documentation: `\`\`\`js\n${preview.js}\n\`\`\``,
      };
    }
    return {
      start: range[0],
      end: range[1],
      css: this.#query.formatCss(preview.css, this.#annotations),
      ...(preview.runtimeNames.length ? { runtimeNames: preview.runtimeNames } : {}),
    };
  }

  /**
   * Renders candidate text in a captured input plan or the standalone input registry.
   * A captured plan already retains those variants; a standalone query receives the prefix
   * explicitly. The original source remains unchanged.
   *
   * @param literal - Indexed literal that supplies the input role, enclosing variants, and plan.
   * @param candidate - Decoded text without the literal's enclosing group prefix.
   * @param site - Captured input whose plan is rendered. Defaults to the literal's capture.
   * A template token can have its own capture.
   * @returns CSS for the replacement, or undefined for failed or unavailable emission.
   * @throws If a matching ordinary utility requirement is malformed in a standalone query.
   */
  #preview(literal: UtilityLiteral, candidate: string, site = literal.site): string | undefined {
    if (site) {
      return this.#index.previews.previewInput(literal.source, site.native.start, candidate);
    }
    return this.#candidates.preview(literal.prefix + candidate, literal.context) ?? undefined;
  }

  /**
   * Completes dynamic-family candidates that plugins list but compilation has not defined yet.
   *
   * @param text - Decoded partial candidate, including enclosing and written variants.
   * @param context - Input role of the literal. Only utility roles complete catalog names.
   * @returns `names`: candidates with the written variants, for names that start with the
   * partial utility, at most {@link CATALOG_COMPLETIONS} of them in catalog order. `incomplete`:
   * whether more names matched than `names` lists.
   */
  #catalogCompletions(
    text: string,
    context: NativeCandidateContext,
  ): {
    /** Listed candidates in catalog order. */
    names: string[];
    /** Whether the limit omitted matching names. */
    incomplete: boolean;
  } {
    if (!this.#catalogContext(context)) {
      return {
        names: [],
        incomplete: false,
      };
    }
    const parts = segments(text, ":");
    const partial = parts.pop()!;
    if (!partial) {
      return {
        names: [],
        incomplete: false,
      };
    }
    const variants = parts.map((part) => `${part}:`).join("");
    const matches = [
      ...new Set([...this.#catalog, ...(this.#inputCatalogs.get(partial) ?? [])]),
    ].filter((name) => name.startsWith(partial));
    return {
      names: matches.slice(0, CATALOG_COMPLETIONS).map((name) => variants + name),
      incomplete: matches.length > CATALOG_COMPLETIONS,
    };
  }

  /**
   * Formats the concrete color of a color utility for a completion swatch.
   *
   * @param candidate - Full decoded candidate, including enclosing and written variants.
   * @returns `#rrggbb`, or `#rrggbbaa` below full opacity. Undefined for other candidates and for
   * colors that depend on the color scheme.
   * @throws If a matching utility definition has a malformed requirement.
   */
  #color(candidate: string): string | undefined {
    const components = this.#candidates.color(candidate);
    if (!components) {
      return undefined;
    }
    const [red, green, blue, alpha] = components.map((value) =>
      Math.round(Math.min(1, Math.max(0, value)) * 255)
        .toString(16)
        .padStart(2, "0"),
    );
    return `#${red}${green}${blue}${alpha === "ff" ? "" : alpha}`;
  }

  /**
   * Reports whether an input role completes plugin catalog names.
   *
   * @param context - Input role of a literal.
   * @returns Whether the role accepts utilities.
   */
  #catalogContext(context: NativeCandidateContext): boolean {
    return ["utility", "document", "anchoredDocument"].includes(context);
  }

  /**
   * Lists the utility tokens of every literal in one source.
   *
   * @param source - Captured source.
   * @returns Tokens with their literals, in literal order.
   */
  #sourceTokens(source: PreparedSource) {
    return this.#index.literals
      .filter((literal) => literal.source === source)
      .flatMap((literal) =>
        this.#tokens(literal).map((token) => ({
          ...token,
          literal,
          // The compiler captures each utility of a template that substitutes variants
          // with the token's own range, so a template token uses that capture.
          site:
            literal.site ??
            (ts.isTemplateLiteralToken(literal.node)
              ? this.#index.previews.input(source, token.start, token.end)
              : undefined),
        })),
      );
  }

  /**
   * Finds the completion input at a cursor: the token that contains it, or an empty position in a
   * literal.
   *
   * @param source - Captured source.
   * @param offset - Original UTF-16 cursor offset.
   * @returns The literal, the replaced UTF-16 range, and the decoded text before the cursor, or
   * undefined outside every literal.
   */
  #completionInput(
    source: PreparedSource,
    offset: number,
  ): { literal: UtilityLiteral; start: number; end: number; prefix: string } | undefined {
    const token = this.#sourceTokens(source).find(
      (token) => token.start <= offset && offset <= token.end,
    );
    const literal =
      token?.literal ??
      this.#index.literals.find(
        (literal) =>
          literal.source === source &&
          literal.boundaries[0] <= offset &&
          offset <= literal.boundaries.at(-1)!,
      );
    if (!literal) {
      return undefined;
    }
    const start = token?.start ?? offset;
    const end = token?.end ?? offset;
    const decoded = literal.boundaries.findIndex((position) => position >= offset);
    const decodedStart = literal.boundaries.indexOf(start);
    return {
      literal,
      start,
      end,
      prefix: literal.text.slice(Math.max(0, decodedStart), Math.max(0, decoded)),
    };
  }

  /**
   * Allows color operations only for input roles that emit utility declarations.
   * Option and definition-prefix inputs use different value contracts, and variant keys
   * emit no declarations.
   *
   * @param literal - Indexed literal whose context supplies the input role.
   * @returns Whether its role supports a utility candidate's concrete color and replacement.
   */
  #colorInput(literal: UtilityLiteral): boolean {
    return (
      !literal.key &&
      ["utility", "keyframesFrame", "viewTransitionUtilities"].includes(literal.context)
    );
  }

  /**
   * Maps whitespace-separated decoded tokens to their exact original spellings.
   *
   * @param literal - Literal with one source boundary for every decoded UTF-16 position,
   * including the end. Whitespace is recognized by JavaScript's `\S` regular expression.
   * @returns Tokens in source order, without trimming or decoding them again.
   */
  #tokens(literal: UtilityLiteral): {
    /** Decoded non-whitespace text without enclosing group variants. */
    text: string;
    /** Inclusive original UTF-16 offset of the token's source spelling. */
    start: number;
    /** Exclusive original UTF-16 offset of the token's source spelling. */
    end: number;
  }[] {
    // In `${trigger("hover:")}:block`, the `:` after a substitution separates the
    // substituted variant from the utility, so it is not part of the utility token.
    const continued = ts.isTemplateMiddle(literal.node) || ts.isTemplateTail(literal.node);
    return [...literal.text.matchAll(/\S+/gu)].flatMap((match) => {
      const skip = continued && match.index === 0 && match[0].startsWith(":") ? 1 : 0;
      const text = match[0].slice(skip);
      return text
        ? [
            {
              text,
              start: literal.boundaries[match.index + skip],
              end: literal.boundaries[match.index + match[0].length],
            },
          ]
        : [];
    });
  }

  /**
   * Finds the ref or container variant expression at `offset`, such as `trigger("hover:")`,
   * ``trigger`hover:` ``, `trigger.at("width >= 40rem")`, or `trigger().in(card)`, and its
   * captured variant preview. The expression extends from the cursor through enclosing
   * property accesses, calls, and tags, and ends before a call of a style producer.
   *
   * @param source - Prepared source stored in this snapshot's index.
   * @param offset - Cursor offset in the original document's UTF-16 coordinates.
   * @returns The formatted selectors and conditions of the first captured variant in that
   * expression, with the expression's original range; undefined when the expression is a ref
   * variant without a valid capture, such as one whose stored variant has no use; or false when
   * the cursor is not in a ref variant expression.
   * @throws If the native formatter rejects the captured CSS.
   */
  #variantPreview(source: PreparedSource, offset: number): EditorResult["hover"] | false {
    let node = this.#index.nodeAt(source, offset);
    let expression: ts.Node | undefined;
    let reference = false;
    while (
      node &&
      (ts.isIdentifier(node) ||
        ts.isStringLiteralLike(node) ||
        ts.isPropertyAccessExpression(node) ||
        ts.isParenthesizedExpression(node) ||
        ts.isTaggedTemplateExpression(node) ||
        (ts.isCallExpression(node) && !this.#index.styleCall(node)))
    ) {
      const callee = ts.isCallExpression(node)
        ? node.expression
        : ts.isTaggedTemplateExpression(node)
          ? node.tag
          : undefined;
      if (callee && this.#refCallee(source, callee)) {
        reference = true;
      }
      expression = node;
      node = node.parent;
    }
    const range = expression && this.#index.range(source, expression);
    if (!reference || !range) {
      return false;
    }
    const site = this.#index.previews
      .sites(source)
      .find(
        (site) => site.native.kind === "variant" && range[0] <= site.start && site.end <= range[1],
      );
    const preview = site && this.#index.previews.preview(site.native);
    return preview
      ? {
          start: range[0],
          end: range[1],
          css: this.#query.formatCss(preview.css, this.#annotations),
        }
      : undefined;
  }

  /**
   * Reports whether `callee` calls a ref asset directly or through `.at()` / `.in()` chains.
   *
   * @param source - Prepared source stored in this snapshot's index.
   * @param callee - Call target or template tag to inspect.
   * @returns Whether the root receiver resolves to a `ref` asset.
   */
  #refCallee(source: PreparedSource, callee: ts.Expression): boolean {
    while (ts.isParenthesizedExpression(callee)) {
      callee = callee.expression;
    }
    if (this.#index.asset(source, callee)?.identity.kind === "ref") {
      return true;
    }
    if (ts.isPropertyAccessExpression(callee) && ["at", "in"].includes(callee.name.text)) {
      let receiver: ts.Expression = callee.expression;
      while (ts.isParenthesizedExpression(receiver)) {
        receiver = receiver.expression;
      }
      if (ts.isCallExpression(receiver)) {
        return this.#refCallee(source, receiver.expression);
      }
      if (ts.isTaggedTemplateExpression(receiver)) {
        return this.#refCallee(source, receiver.tag);
      }
      return this.#index.asset(source, receiver)?.identity.kind === "ref";
    }
    return false;
  }
}
