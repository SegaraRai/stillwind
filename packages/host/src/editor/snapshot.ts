import type {
  BaseConfig,
  LintSeverity,
  PreviewOptions,
  UtilitySortOptions,
} from "@stillwind/config";
import {
  fingerprint,
  type BindingEdit,
  type BindingReport,
  type NativeDesignSystemLoader,
  type NativeEditorQuery,
  type NativeProjectSession,
} from "@stillwind/native";
import type { PreparedSource } from "@stillwind/projection";

import type { ModuleAnalysis } from "../compilation/request.ts";
import type { PluginRuntime } from "../css/plugins/runtime.ts";
import { pluginLoader } from "../css/plugins/session.ts";
import { CandidateQueries } from "./candidates.ts";
import { editorResult, type EditorEdit, type EditorRequest, type EditorResult } from "./request.ts";
import { sortSource } from "./sort.ts";
import { SourceIndex } from "./source/inputs.ts";
import { SourceQueries } from "./source/query.ts";

/**
 * Runs source editor requests and sorting diagnostics against captured compiler output.
 */
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

  /** Targets for explicit sorting and sorting fixes. False disables both. */
  readonly #sort: Readonly<UtilitySortOptions> | false;

  /** Effective prefer-sorted-utilities severity. Off skips its diagnostics and sorting in fixes. */
  readonly #sortSeverity: LintSeverity;

  /** Sorting reports cached by full compiler module identity, including empty results. */
  readonly #reports = new Map<string, BindingReport[]>();

  /** Completion, hover, and color queries over the same captured sources. */
  readonly #queries: SourceQueries;

  /**
   * Captures recognized producers and compiler output for one set of source revisions.
   * Copies source metadata, dependencies, fix actions, and sorting settings before indexing them.
   * Building the source index also scans graph sources in the session.
   *
   * @param graph - Original sources, demanded dependencies, emitted CSS, and lint reports.
   * @param session - Session that compiled the graph and recognizes its producer identities.
   * @param loader - Completed design system with the same compiler options as the session.
   * @param config - Sorting targets and preview comments to capture. Effective lint policy comes
   * from the loader.
   * @param catalog - Finite plugin candidates offered without running their callbacks.
   * @param plugins - Runtime for resolving selected plugin completions without changing the session.
   * @throws If source transport, a projection map, or an original UTF-8 boundary is invalid.
   */
  constructor(
    graph: ModuleAnalysis,
    session: NativeProjectSession,
    loader: NativeDesignSystemLoader,
    config: BaseConfig & {
      preview?: PreviewOptions;
    },
    catalog: readonly string[] = [],
    plugins?: PluginRuntime,
  ) {
    const inputs = structuredClone({
      sources: graph.sources,
      dependencies: graph.dependencies,
    });
    this.#sources = inputs.sources;
    this.#fixes = structuredClone(graph.fixes);
    this.#index = new SourceIndex(
      inputs,
      session,
      graph.modules.map((module) => module.moduleId),
    );
    // Plugin definitions added during compilation live in the session's latest loader.
    this.#query = (pluginLoader(session) ?? loader).editorQuery();
    this.#candidates = new CandidateQueries(this.#query);
    const css = this.#query.lintSettings();
    this.#sort =
      config.sort === false
        ? false
        : {
            stringLiterals: config.sort?.stringLiterals ?? true,
            arrays: config.sort?.arrays ?? true,
          };
    this.#sortSeverity = (css["prefer-sorted-utilities"] as LintSeverity) ?? "off";
    const annotations = {
      ...config.preview,
    };
    const revision = fingerprint(JSON.stringify([this.#index.previews.revision, config]));
    this.#queries = new SourceQueries(
      this.#index,
      this.#query,
      this.#candidates,
      revision,
      annotations,
      catalog,
      plugins,
    );
  }

  /**
   * Loads plugin suggestions for a completion input, and callback-generated CSS for a selected
   * completion from this captured snapshot, so that a following {@link EditorSnapshot.query} can
   * use them. Other requests are ignored.
   *
   * @param id - Full resolver identity of the requested source.
   * @param request - Editor request. A `completion` request loads the plugin suggestions for the
   * candidate at its cursor. A `resolve` request for this snapshot's revision loads CSS, once for
   * each candidate.
   * @throws If a plugin fails or the native loader rejects the generated CSS.
   */
  async prepare(id: string, request: EditorRequest): Promise<void> {
    const source = this.#sources.find((source) => source.resolverId === id);
    await this.#queries.prepare(source, request);
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

    const result: EditorResult = editorResult(source.document.code);
    if (request.kind === "definition") {
      return {
        ...result,
        definitions: [],
      };
    }
    if (request.kind === "links") {
      return {
        ...result,
        links: [],
      };
    }
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
                      this.#index.lintComments(source),
                    ),
            )
          : seed;
      return result;
    }
    return this.#queries.query(source, request, result);
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
      this.#index.lintComments(source),
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
