import type { BaseConfig, UtilitySortOptions, LintSeverity } from "@stillwind/config";
import {
  fingerprint,
  type NativeDesignSystemLoader,
  type NativeProjectSession,
  type BindingReport,
  type NativeEditorQuery,
} from "@stillwind/native";
import type { PreparedSource } from "@stillwind/projection";
import ts from "typescript-6";

import type { ModuleAnalysis } from "../compilation/compiler.ts";
import { CandidateQueries, segments } from "./candidates.ts";
import { sortSource, escapeContent } from "./sort.ts";
import { SourceIndex, type UtilityLiteral } from "./source-index.ts";
import { stylePreview } from "./style-preview.ts";

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
}

/** A concrete sRGB color at an original candidate range. Components are in 0..1. */
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
      kind: "hover";
      /** Original UTF-16 cursor offset at a utility or recognized style expression. */
      offset: number;
    }
  | {
      kind: "completion";
      /** Original UTF-16 insertion point; text before this point determines the prefix. */
      offset: number;
    }
  | {
      kind: "resolve";
      /** Full candidate returned by completion, including surrounding group variants. */
      candidate: string;
    }
  | {
      kind: "colors";
    }
  | {
      kind: "fixes";
    }
  | {
      kind: "sort";
    }
  | {
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
  /** Formatted CSS at a half-open original UTF-16 range. The main selector is `&`. */
  hover?: {
    start: number;
    end: number;
    css: string;
  };
  /** Prefix-matching candidates with their full replacement ranges. CSS loads on resolve. */
  completions?: EditorCompletion[];
  /** Concrete colors resolved against the active design system. */
  colors?: EditorColor[];
  /** Nonoverlapping replacements in ascending original offset order. */
  edits?: EditorEdit[];
}

/** Runs source queries and actionable lints against one immutable compiler graph. */
export class EditorSnapshot {
  readonly #graph: ModuleAnalysis;
  readonly #index: SourceIndex;
  readonly #query: NativeEditorQuery;
  readonly #candidates: CandidateQueries;
  readonly #sort: UtilitySortOptions;
  readonly #sortSeverity: LintSeverity;
  readonly #reports = new Map<string, BindingReport[]>();

  /**
   * Captures recognized producers and compiler output. The caller must keep all inputs unchanged.
   * @param graph - Original sources, demanded dependencies, emitted CSS, and lint reports.
   * @param session - Session that compiled the graph and recognizes its producer identities.
   * @param loader - Completed design system with the same compiler options as the session.
   * @param config - Shared sorting targets; effective lint policy comes from the loader.
   */
  constructor(
    graph: ModuleAnalysis,
    session: NativeProjectSession,
    loader: NativeDesignSystemLoader,
    config: BaseConfig,
  ) {
    this.#graph = graph;
    this.#index = new SourceIndex(graph, session);
    this.#query = loader.editorQuery();
    this.#candidates = new CandidateQueries(this.#query);
    const css = this.#query.lintSettings();
    this.#sort = config.sort ?? {};
    this.#sortSeverity = (css["sort-utilities"] as LintSeverity) ?? "off";
  }

  /** Returns revision-pinned reports for every source in the captured dependency graph. */
  reports(): BindingReport[] {
    for (const source of this.#graph.sources) {
      this.#lint(source);
    }
    return [...this.#reports.values()].flat();
  }

  /**
   * Queries a source by resolver identity. Unknown or unmapped positions produce no data.
   * @param id - Full resolver identity captured in the graph, including any query suffix.
   * @param request - Operation and positions in the original document's UTF-16 coordinate space.
   * @returns Revision-pinned data, or undefined when the graph does not contain the source.
   */
  query(id: string, request: EditorRequest): EditorResult | undefined {
    const source = this.#graph.sources.find((source) => source.resolverId === id);
    if (!source) {
      return undefined;
    }

    const result: EditorResult = { revision: fingerprint(source.document.code) };
    if (request.kind === "fixes" || request.kind === "sort") {
      const seed =
        request.kind === "fixes"
          ? uniqueEdits(
              this.#graph.batches
                .flatMap((batch) => batch.reports)
                .filter((report) => report.code !== "sort-utilities")
                .flatMap((report) => report.actions[0]?.edits ?? [])
                .filter(
                  (edit) =>
                    edit.location.uri === source.document.uri &&
                    edit.location.revision === result.revision,
                )
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
                ? undefined
                : (start, end) =>
                    !this.#query.lintSuppressed(
                      source.document.code,
                      bytes(source.document.code, start),
                      bytes(source.document.code, end),
                      "sort-utilities",
                    ),
            )
          : seed;
      return result;
    }
    if (request.kind === "resolve") {
      const css = this.#candidates.preview(request.candidate);
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
      if (!token) {
        return result;
      }

      const candidate = colorReplacement(
        token.text,
        token.literal.prefix,
        request.color,
        this.#candidates,
      );
      if (candidate) {
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
        const css = this.#candidates.preview(token.literal.prefix + token.text);
        if (css) {
          result.hover = {
            start: token.start,
            end: token.end,
            css: this.#query.formatCss(css),
          };
        }
      } else {
        let node = this.#index.nodeAt(source, request.offset);
        while (node) {
          const asset = this.#index.asset(source, node);
          if (asset?.identity.kind === "style") {
            const css = stylePreview(this.#graph, asset);
            const range = this.#index.range(source, node);
            if (css && range) {
              result.hover = {
                start: range[0],
                end: range[1],
                css: this.#query.formatCss(css),
              };
            }
            break;
          }
          if (ts.isCallExpression(node)) {
            break;
          }
          node = node.parent;
        }
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
    result.completions = this.#candidates.complete(literal.prefix + prefix).map((candidate) => ({
      label: candidate.slice(literal.prefix.length),
      candidate,
      start,
      end,
      newText: escapeContent(candidate.slice(literal.prefix.length), literal),
    }));

    return result;
  }

  #tokens(literal: UtilityLiteral): { text: string; start: number; end: number }[] {
    return [...literal.text.matchAll(/\S+/gu)].map((match) => ({
      text: match[0],
      start: literal.boundaries[match.index],
      end: literal.boundaries[match.index + match[0].length],
    }));
  }

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

function bytes(text: string, offset: number): number {
  return new TextEncoder().encode(text.slice(0, offset)).length;
}

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
    const property = base.slice(1).split(":")[0];
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
