import type { BaseConfig, LintSeverity, UtilitySortOptions } from "@stillwind/config";
import {
  fingerprint,
  type BindingEdit,
  type BindingReport,
  type NativeDesignSystemLoader,
  type NativeEditorQuery,
  type NativeProjectSession,
} from "@stillwind/native";
import type { PreparedSource } from "@stillwind/projection";
import ts from "typescript-6";

import type { ModuleAnalysis } from "../compilation/compiler.ts";
import { CandidateQueries, segments } from "./candidates.ts";
import type { GeneratedPreview } from "./previews.ts";
import { escapeContent, sortSource } from "./sort.ts";
import { SourceIndex, type UtilityLiteral } from "./source-index.ts";

/** A replacement in the original document, using half-open UTF-16 offsets. */
export interface EditorEdit {
  /** Inclusive UTF-16 offset in the original document. */
  start: number;
  /** Exclusive UTF-16 offset in the original document. */
  end: number;
  /** Source spelling inserted at the range, including any necessary JS escapes. */
  newText: string;
}

/** Completion text replaces the entire candidate at the supplied original range. */
export interface EditorCompletion extends EditorEdit {
  /** Visible completion text; surrounding group variants are omitted. */
  label: string;
  /** Full candidate used for lazy CSS resolution, including surrounding group variants. */
  candidate: string;
  /** Original UTF-16 boundary of the literal's first decoded position. */
  input: number;
  /** Fingerprint of the captured graph and configuration. Resolve requires this exact value. */
  snapshot: string;
}

/** A concrete sRGB color at an original candidate range. */
export interface EditorColor {
  /** Inclusive UTF-16 offset of the authored candidate. */
  start: number;
  /** Exclusive UTF-16 offset of the authored candidate. */
  end: number;
  /** Red component in the inclusive range 0..1. */
  red: number;
  /** Green component in the inclusive range 0..1. */
  green: number;
  /** Blue component in the inclusive range 0..1. */
  blue: number;
  /** Effective opacity after utility modifiers, in the inclusive range 0..1. */
  alpha: number;
}

/** Requests address original document UTF-16 offsets, including incomplete candidate text. */
export type EditorRequest =
  | {
      /** Requests CSS for a candidate or a recognized expression occurrence. */
      kind: "hover";
      /** Original UTF-16 cursor offset at a utility or recognized style expression. */
      offset: number;
    }
  | {
      /** Requests matching candidates and variant prefixes for an insertion point. */
      kind: "completion";
      /** Original UTF-16 insertion point; text before this point determines the prefix. */
      offset: number;
    }
  | {
      /** Requests CSS for a completion from the same captured snapshot. */
      kind: "resolve";
      /** Full candidate returned by completion, including surrounding group variants. */
      candidate: string;
      /** Original UTF-16 boundary returned as `EditorCompletion.input`. */
      input: number;
      /** Captured fingerprint returned by completion; a mismatch omits hover output. */
      snapshot: string;
    }
  | {
      /** Requests concrete colors for recognized utility candidates in the source. */
      kind: "colors";
    }
  | {
      /** Requests each report's first fix action for this source revision, with enabled sorting. */
      kind: "fixes";
    }
  | {
      /** Requests configured sorting independently of lint severity. */
      kind: "sort";
      /** Half-open original UTF-16 ranges that sorting must leave untouched. */
      protectedRanges?: readonly {
        /** Inclusive original UTF-16 offset of the protected text. */
        start: number;
        /** Exclusive original UTF-16 offset of the protected text. */
        end: number;
      }[];
    }
  | {
      /** Requests a replacement candidate for a range returned by the colors query. */
      kind: "colorPresentation";
      /** Inclusive original UTF-16 offset returned by the colors query. */
      start: number;
      /** Exclusive original UTF-16 offset returned by the colors query. */
      end: number;
      /** Desired sRGB components and effective opacity, each in 0..1. */
      color: Pick<EditorColor, "red" | "green" | "blue" | "alpha">;
    };

/** Editor data belongs to the exact source fingerprint in revision. Absent fields mean no answer. */
export interface EditorResult {
  /** Fingerprint of the original source text used to compute every range and edit. */
  revision: string;
  /** Formatted CSS at a half-open original UTF-16 range. Generated element subjects use `&`.
   * Lazy completion resolution uses the sentinel range `[0, 0)`.
   */
  hover?: {
    /** Inclusive original UTF-16 boundary, or zero for completion resolution. */
    start: number;
    /** Exclusive original UTF-16 boundary, or zero for completion resolution. */
    end: number;
    /** CSS with readable generated identifiers; expression hovers include required definitions. */
    css: string;
    /** Authored runtime key expressions, deduplicated in dependency traversal order.
     * Absent when the preview has no runtime key expression to explain.
     */
    runtimeNames?: string[];
  };
  /** Prefix-matching candidates with their full replacement ranges. CSS loads on resolve. */
  completions?: EditorCompletion[];
  /** Concrete colors resolved against the active design system. */
  colors?: EditorColor[];
  /** Nonoverlapping replacements in ascending original offset order. */
  edits?: EditorEdit[];
}

/** Runs source queries and actionable sorting diagnostics using captured compiler output. */
export class EditorSnapshot {
  /** Owned source buffers and projection metadata captured before publication. */
  readonly #sources: readonly PreparedSource[];
  /** First fix actions from compilation reports, captured in original coordinates. */
  readonly #fixes: readonly (readonly BindingEdit[])[];
  /** Checker ASTs, stable aliases, and captured emission plans for the graph's sources. */
  readonly #index: SourceIndex;
  /** Native design system and effective lint policy captured during construction. */
  readonly #query: NativeEditorQuery;
  /** Candidate caches shared by all queries against this snapshot. */
  readonly #candidates: CandidateQueries;
  /** Fingerprint of captures, graph dependencies, and configuration used for lazy resolution. */
  readonly #revision: string;
  /** Targets for explicit sorting and sorting fixes. False disables both. */
  readonly #sort: Readonly<UtilitySortOptions> | false;
  /** Effective prefer-sorted-utilities severity. Off skips its diagnostics and sorting in fixes. */
  readonly #sortSeverity: LintSeverity;
  /** Sorting reports cached by full compiler module identity, including empty results. */
  readonly #reports = new Map<string, BindingReport[]>();

  /**
   * Captures recognized producers and compiler output for one set of source revisions.
   * Copies source metadata, dependencies, fix actions, and sorting settings before indexing them.
   * Building the source index also scans graph sources in the session.
   *
   * @param graph - Original sources, demanded dependencies, emitted CSS, and lint reports.
   * @param session - Session that compiled the graph and recognizes its producer identities.
   * @param loader - Completed design system with the same compiler options as the session.
   * @param config - Sorting targets to capture. Effective lint policy comes from the loader.
   * @throws If source transport, a projection map, or an original UTF-8 boundary is invalid.
   */
  constructor(
    graph: ModuleAnalysis,
    session: NativeProjectSession,
    loader: NativeDesignSystemLoader,
    config: BaseConfig,
  ) {
    const inputs = structuredClone({
      sources: graph.sources,
      dependencies: graph.dependencies,
    });
    this.#sources = inputs.sources;
    this.#fixes = structuredClone(graph.fixes);
    this.#index = new SourceIndex(inputs, session);
    this.#query = loader.editorQuery();
    this.#candidates = new CandidateQueries(this.#query);
    const css = this.#query.lintSettings();
    this.#sort =
      config.sort === false
        ? false
        : {
            stringLiterals: config.sort?.stringLiterals ?? true,
            arrays: config.sort?.arrays ?? true,
            attributes: config.sort?.attributes ?? false,
          };
    this.#sortSeverity = (css["prefer-sorted-utilities"] as LintSeverity) ?? "off";
    this.#revision = fingerprint(JSON.stringify([this.#index.previews.revision, config]));
  }

  /**
   * Computes sorting diagnostics for all graph sources and caches them by module identity.
   * Effective lint severity and source suppression determine which reports are returned.
   * Compilation reports already present in the graph remain separate.
   *
   * @returns Independent copies of sorting reports for the captured sources.
   * @throws If candidate ranking fails or native report construction rejects a source URI or range.
   */
  reports(): BindingReport[] {
    for (const source of this.#sources) {
      this.#lint(source);
    }
    return structuredClone([...this.#reports.values()].flat());
  }

  /**
   * Queries a source by resolver identity. Unknown or unmapped positions produce no data.
   * Queries may populate caches, and returned edits leave the original document unchanged.
   *
   * @param id - Full resolver identity captured in the graph, including any query suffix.
   * @param request - Operation and positions in the original document's UTF-16 coordinate space.
   * Completion resolution must reuse the candidate, input boundary, and snapshot fingerprint.
   * @returns Data with the original source fingerprint, or undefined for an unknown source.
   * A known source with no applicable answer returns a result with the relevant field absent or empty.
   * @throws If native candidate resolution, CSS formatting, or lint suppression checks fail.
   */
  query(id: string, request: EditorRequest): EditorResult | undefined {
    const source = this.#sources.find((source) => source.resolverId === id);
    if (!source) {
      return undefined;
    }

    const result: EditorResult = { revision: fingerprint(source.document.code) };
    if (request.kind === "fixes" || request.kind === "sort") {
      const seed =
        request.kind === "fixes"
          ? uniqueEdits(
              this.#fixes
                .filter((edits) =>
                  edits.every(
                    (edit) =>
                      edit.location.uri === source.document.uri &&
                      edit.location.revision === result.revision,
                  ),
                )
                .flat()
                .map((edit) => ({
                  start: edit.location.startUtf16,
                  end: edit.location.endUtf16,
                  newText: edit.replacement,
                })),
            )
          : [];
      result.edits =
        request.kind === "sort" || this.#sortSeverity !== "off"
          ? sortSource(
              source,
              this.#index,
              this.#candidates,
              this.#sort,
              seed,
              request.kind === "sort"
                ? request.protectedRanges?.length
                  ? (start, end) =>
                      !request.protectedRanges!.some(
                        (range) => start < range.end && range.start < end,
                      )
                  : undefined
                : (start, end) =>
                    !this.#query.lintSuppressed(
                      source.document.code,
                      bytes(source.document.code, start),
                      bytes(source.document.code, end),
                      "prefer-sorted-utilities",
                    ),
            )
          : seed;
      return result;
    }
    if (request.kind === "resolve") {
      const literal = this.#index.literals.find(
        (literal) => literal.source === source && literal.boundaries[0] === request.input,
      );
      const css =
        literal && request.snapshot === this.#revision
          ? this.#preview(literal, request.candidate.slice(literal.prefix.length))
          : undefined;
      if (css) {
        result.hover = {
          start: 0,
          end: 0,
          css: this.#query.formatCss(css),
        };
      }
      return result;
    }

    const literals = this.#index.literals.filter((literal) => literal.source === source);
    const tokens = literals.flatMap((literal) =>
      this.#tokens(literal).map((token) => ({ ...token, literal })),
    );
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
        if (css) {
          result.hover = {
            start: token.start,
            end: token.end,
            css: this.#query.formatCss(css),
          };
        }
      } else {
        result.hover = this.#expressionPreview(source, request.offset);
      }
      return result;
    }

    const literal =
      token?.literal ??
      literals.find(
        (literal) =>
          literal.boundaries[0] <= request.offset && request.offset <= literal.boundaries.at(-1)!,
      );
    if (!literal) {
      return result;
    }

    const start = token?.start ?? request.offset;
    const end = token?.end ?? request.offset;
    const decoded = literal.boundaries.findIndex((position) => position >= request.offset);
    const decodedStart = literal.boundaries.indexOf(start);
    const prefix = literal.text.slice(Math.max(0, decodedStart), Math.max(0, decoded));
    result.completions = this.#candidates
      .complete(literal.prefix + prefix, literal.context)
      .filter(
        (candidate) =>
          candidate.endsWith(":") ||
          !literal.site ||
          this.#preview(literal, candidate.slice(literal.prefix.length)),
      )
      .map((candidate) => ({
        label: candidate.slice(literal.prefix.length),
        candidate,
        input: literal.boundaries[0],
        snapshot: this.#revision,
        start,
        end,
        newText: escapeContent(candidate.slice(literal.prefix.length), literal),
      }));

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
              css: this.#query.formatCss(preview.css),
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
          css: this.#query.formatCss(reference.preview.css),
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

  /**
   * Computes sorting edits once and records reports through the native lint policy.
   * UTF-16 edit boundaries are converted to original UTF-8 byte ranges before reporting.
   * Empty results are cached too, and a failed computation leaves the module uncached.
   *
   * @param source - Unchanged prepared source stored in this snapshot's graph.
   * @throws If candidate ranking fails or native reporting rejects the source URI or byte ranges.
   */
  #lint(source: PreparedSource): void {
    if (this.#reports.has(source.moduleId)) {
      return;
    }

    const edits =
      this.#sortSeverity === "off"
        ? []
        : sortSource(source, this.#index, this.#candidates, this.#sort);
    const reports = this.#query.reportSortEdits(
      source.document.uri,
      source.document.code,
      edits.map((edit) => ({
        start: bytes(source.document.code, edit.start),
        end: bytes(source.document.code, edit.end),
        replacement: edit.newText,
      })),
    );
    this.#reports.set(source.moduleId, reports);
  }
}

/**
 * Converts an original document position to the native compiler's byte coordinates.
 *
 * @param text - Original source text whose encoding supplies the byte count.
 * @param offset - UTF-16 integer offset within `text`, at a Unicode scalar boundary.
 * The caller must ensure that the offset does not split a surrogate pair.
 * @returns Number of UTF-8 bytes before that position.
 */
function bytes(text: string, offset: number): number {
  return new TextEncoder().encode(text.slice(0, offset)).length;
}

/**
 * Sorts the supplied array in place and greedily keeps edits that do not overlap earlier ones.
 *
 * @param edits - Edits in the same original UTF-16 coordinate space. The array is reordered by
 * start and then end. Earlier starts win, and a shorter edit wins when starts are equal.
 * @returns A new array of accepted edit objects. Touching ranges and identical zero-width
 * insertions are retained. The edit objects themselves remain unchanged.
 */
function uniqueEdits(edits: EditorEdit[]): EditorEdit[] {
  const result: EditorEdit[] = [];
  for (const edit of edits.sort((a, b) => a.start - b.start || a.end - b.end)) {
    if (result.some((other) => other.start < edit.end && edit.start < other.end)) {
      continue;
    }
    result.push(edit);
  }
  return result;
}

/**
 * Finds a spelling with an arbitrary color that preserves the candidate's variants, importance,
 * and modifier.
 * Existing opacity modifiers remain present, so their maximum opacity can prevent a replacement.
 * The emitted color must agree with every requested component within 2/255.
 *
 * @param text - Decoded candidate without enclosing group variants. Its current color must resolve.
 * @param outer - Enclosing decoded variants, including trailing colons, used when probing colors.
 * @param color - Finite sRGB components and effective alpha. Values are clamped to 0..1.
 * @param candidates - Query for the snapshot's design system, used to validate each proposed spelling.
 * @returns Decoded replacement text without `outer`, or undefined when the original color cannot
 * resolve or no supported spelling with an arbitrary color reproduces the requested components.
 * @throws If a matching utility definition has a malformed requirement.
 */
function colorReplacement(
  text: string,
  outer: string,
  color: Pick<EditorColor, "red" | "green" | "blue" | "alpha">,
  candidates: CandidateQueries,
): string | undefined {
  if (!candidates.color(outer + text)) {
    return undefined;
  }

  const parts = segments(text, ":");
  const utility = parts.pop()!;
  const prefix = parts.length ? `${parts.join(":")}:` : "";
  const importance = utility.startsWith("!") || utility.endsWith("!") ? "!" : "";
  const [base, modifier] = segments(utility.replace(/^!|!$/g, ""), "/");
  const spellings: ((value: string) => string)[] = [];
  if (base.startsWith("[") && base.endsWith("]")) {
    const property = base.slice(1).split(":", 1)[0];
    spellings.push((value) => `[${property}:${value}]`);
  } else {
    for (let index = base.length - 1; index > 0; index--) {
      if (base[index] === "-") {
        const root = base.slice(0, index);
        spellings.push((value) => `${root}-[${value}]`);
      }
    }
  }

  const rgba = [color.red, color.green, color.blue, color.alpha].map((value) =>
    Math.min(1, Math.max(0, value)),
  );
  const rgb = rgba.slice(0, 3).map((value) => Math.round(value * 255));
  for (const spelling of spellings) {
    /**
     * Rebuilds a color candidate with its variants, importance, and opacity modifier preserved.
     * @param value - CSS color encoded for an arbitrary utility value.
     * @returns A candidate using the current spelling, without the separately supplied outer variants.
     */
    const build = (value: string) =>
      `${prefix}${importance}${spelling(value)}${modifier ? `/${modifier}` : ""}`;
    const probe = candidates.color(outer + build("#ffffff"));
    if (!probe || rgba[3] > probe[3] + 1 / 255) {
      continue;
    }

    const alpha = probe[3] ? rgba[3] / probe[3] : 1;
    const value =
      alpha >= 1
        ? `#${rgb.map((value) => value.toString(16).padStart(2, "0")).join("")}`
        : `rgb(${rgb.join("_")}_/_${Number(alpha.toFixed(4))})`;
    const candidate = build(value);
    const rendered = candidates.color(outer + candidate);
    if (rendered?.every((component, index) => Math.abs(component - rgba[index]) <= 2 / 255)) {
      return candidate;
    }
  }

  return undefined;
}
