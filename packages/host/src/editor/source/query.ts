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
      const marker = current ? candidates.marker(request.candidate, literal.context) : null;
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

    const tokens = this.#sourceTokens(source);
    if (request.kind === "colors") {
      result.colors = tokens.flatMap((token) => {
        if (!this.#colorInput(token.literal) || !this.#preview(token.literal, token.text)) {
          return [];
        }
        const color = this.#candidates.color(token.literal.prefix + token.text);
        return color
          ? [
              {
                start: token.start,
                end: token.end,
                red: color[0],
                green: color[1],
                blue: color[2],
                alpha: color[3],
              },
            ]
          : [];
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
      if (candidate && this.#preview(token.literal, candidate)) {
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
      if (token) {
        const css = this.#preview(token.literal, token.text);
        const marker = this.#candidates.marker(
          token.literal.prefix + token.text,
          token.literal.context,
        );
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
      .filter(
        ({ label, marker }) =>
          marker ||
          label.endsWith(":") ||
          !literal.site ||
          this.#preview(literal, label.slice(literal.prefix.length)),
      );
    const known = new Set(native.map(({ label }) => label));
    result.completions = [
      ...native,
      ...this.#catalogCompletions(literal.prefix + prefix, literal.context)
        .filter((candidate) => !known.has(candidate))
        .map((label) => ({
          label,
          marker: false,
        })),
    ].map(({ label: candidate, marker }) => {
      const label = candidate.slice(literal.prefix.length);
      const source = (text: string) => escapeContent(text, literal);
      return {
        label,
        candidate,
        input: literal.boundaries[0],
        snapshot: this.#revision,
        start,
        end,
        newText: escapeContent(label, literal),
        ...(marker ? { marker } : {}),
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
      if (site) {
        const preview = this.#index.previews.preview(site.native);
        const range = this.#index.range(source, node);
        return preview && range
          ? {
              start: range[0],
              end: range[1],
              css: this.#query.formatCss(preview.css, this.#annotations),
              ...(preview.runtimeNames.length ? { runtimeNames: preview.runtimeNames } : {}),
            }
          : undefined;
      }
      if (!reference) {
        const asset = this.#index.asset(source, node);
        const preview = asset && this.#index.previews.previewIdentity(asset.identity);
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
      ? {
          start: reference.range[0],
          end: reference.range[1],
          css: this.#query.formatCss(reference.preview.css, this.#annotations),
          ...(reference.preview.runtimeNames.length
            ? {
                runtimeNames: reference.preview.runtimeNames,
              }
            : {}),
        }
      : undefined;
  }

  /**
   * Renders candidate text in a captured input plan or the standalone input registry.
   * A captured plan already retains those variants; a standalone query receives the prefix
   * explicitly. The original source remains unchanged.
   *
   * @param literal - Indexed literal that supplies the input role, enclosing variants, and plan.
   * @param candidate - Decoded text without the literal's enclosing group prefix.
   * @returns CSS for the replacement, or undefined for failed or unavailable emission.
   * @throws If a matching ordinary utility requirement is malformed in a standalone query.
   */
  #preview(literal: UtilityLiteral, candidate: string): string | undefined {
    if (literal.site) {
      return this.#index.previews.previewInput(
        literal.source,
        literal.site.native.start,
        candidate,
      );
    }
    return this.#candidates.preview(literal.prefix + candidate, literal.context) ?? undefined;
  }

  /**
   * Completes dynamic-family candidates that plugins list but compilation has not defined yet.
   *
   * @param text - Decoded partial candidate, including enclosing and written variants.
   * @param context - Input role of the literal. Only utility roles complete catalog names.
   * @returns Candidates with the written variants, for names that start with the partial
   * utility, at most {@link CATALOG_COMPLETIONS} of them in catalog order.
   */
  #catalogCompletions(text: string, context: NativeCandidateContext): string[] {
    if (!this.#catalogContext(context)) {
      return [];
    }
    const parts = segments(text, ":");
    const partial = parts.pop()!;
    if (!partial) {
      return [];
    }
    const variants = parts.map((part) => `${part}:`).join("");
    return [...new Set([...this.#catalog, ...(this.#inputCatalogs.get(partial) ?? [])])]
      .filter((name) => name.startsWith(partial))
      .slice(0, CATALOG_COMPLETIONS)
      .map((name) => variants + name);
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
   * Option and definition-prefix inputs use different value contracts.
   *
   * @param literal - Indexed literal whose context supplies the input role.
   * @returns Whether its role supports a utility candidate's concrete color and replacement.
   */
  #colorInput(literal: UtilityLiteral): boolean {
    return ["utility", "keyframesFrame", "viewTransitionUtilities"].includes(literal.context);
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
    return [...literal.text.matchAll(/\S+/gu)].map((match) => ({
      text: match[0],
      start: literal.boundaries[match.index],
      end: literal.boundaries[match.index + match[0].length],
    }));
  }
}
