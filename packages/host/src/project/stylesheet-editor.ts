import type { BaseConfig, PreviewOptions } from "@stillwind/config";
import { fingerprint, type BindingReport, type NativeEditorQuery } from "@stillwind/native";

import { catalogInput, type PluginRuntime } from "../css/plugins.ts";
import { CandidateQueries } from "../editor/candidates.ts";
import { colorReplacement, cssColor } from "../editor/colors.ts";
import { completionSnippet } from "../editor/completion.ts";
import type { EditorEdit, EditorRequest, EditorResult } from "../editor/snapshot.ts";

/** Answers semantic CSS queries and lint fixes from one original buffer and captured design system. */
export class StylesheetEditor {
  /** Resolver identity of the stylesheet. */
  readonly #id: string;
  /** Fingerprint of the captured stylesheet text that every report range addresses. */
  readonly #revision: string;
  /** Lint reports whose primary location is this stylesheet. */
  readonly #reports: readonly BindingReport[];
  /** Original buffer whose UTF-16 coordinates are exposed to callers. */
  readonly #text: string;
  /** UTF-8 encoding of the original buffer used to convert native byte ranges. */
  readonly #bytes: Uint8Array;
  /** Query pinned to the design system captured with this buffer. */
  readonly #query: NativeEditorQuery;
  /** Captured comment settings. Omitted fields use native preview defaults. */
  readonly #preview: PreviewOptions;
  /** Fingerprint of source, design-system revision, and serialized configuration for completion resolution. */
  readonly #snapshot: string;
  /** Caches variant previews and completion summaries for this buffer's captured design system. */
  readonly #candidates: CandidateQueries;
  /** Queries that include callback CSS for selected completions in this stylesheet. */
  readonly #resolved = new Map<string, NativeEditorQuery>();
  /** Finite plugin suggestions captured with this stylesheet's design system. */
  readonly #catalog: readonly string[];
  /** Plugin suggestions for editor inputs that {@link StylesheetEditor.prepare} loaded, keyed by input. */
  readonly #inputCatalogs = new Map<string, readonly string[]>();
  /** Resolves completion CSS without changing reports or a session's used utilities. */
  readonly #plugins: PluginRuntime | undefined;

  /**
   * Captures original text, lint reports, and query settings for one stylesheet revision.
   * Callers must load the original CSS graph before using sorting or lint fixes.
   * @param id - Resolver identity of the stylesheet.
   * @param text - Complete stylesheet text that the reports were computed from.
   * @param reports - Reports whose primary location is this stylesheet. Reports and edits are copied.
   * Use an empty array when the design system was loaded from recovered CSS.
   * @param query - Immutable design system captured for this source revision.
   * @param config - Compiler and preview settings included in completion identity.
   * @param catalog - Finite plugin candidates offered without running their callbacks.
   * @param plugins - Runtime for resolving selected plugin completions without changing used CSS.
   * @throws If reports cannot be cloned or configuration cannot be serialized.
   */
  constructor(
    id: string,
    text: string,
    reports: readonly BindingReport[],
    query: NativeEditorQuery,
    config: BaseConfig & {
      preview?: PreviewOptions;
    } = {},
    catalog: readonly string[] = [],
    plugins?: PluginRuntime,
  ) {
    this.#id = id;
    this.#catalog = catalog;
    this.#plugins = plugins;
    this.#revision = fingerprint(text);
    this.#reports = structuredClone(reports);
    this.#text = text;
    this.#bytes = new TextEncoder().encode(text);
    this.#query = query;
    this.#candidates = new CandidateQueries(query);
    this.#preview = { ...config.preview };
    this.#snapshot = fingerprint(JSON.stringify([text, query.revision(), config]));
  }

  /**
   * Loads plugin suggestions for a completion input, and resolves callback CSS for a selected
   * completion without changing the captured source or reports, so that a following query can
   * use them. Other requests are ignored.
   *
   * @param id - Resolver identity of the requested source. Another identity is ignored.
   * @param request - Editor request. A `completion` request, and a `resolve` request for this
   * stylesheet's completion snapshot, load the plugin suggestions for the candidate at their input.
   * A `resolve` request also loads CSS, once for each candidate. A variant completion is resolved
   * with its `:`.
   * @throws If a plugin fails or the native loader rejects the generated CSS.
   */
  async prepare(id: string, request: EditorRequest): Promise<void> {
    if (id !== this.#id) {
      return;
    }
    if (
      this.#plugins &&
      (request.kind === "completion" ||
        (request.kind === "resolve" &&
          request.snapshot ===
            this.#completionSnapshot(request.registryDirectives, request.snippets)))
    ) {
      const input = catalogInput(
        this.#text.slice(0, request.kind === "completion" ? request.offset : request.input),
      );
      if (input && !this.#inputCatalogs.has(input)) {
        this.#inputCatalogs.set(input, await this.#plugins.catalog(input));
      }
    }
    if (
      request.kind === "resolve" &&
      request.snapshot === this.#completionSnapshot(request.registryDirectives, request.snippets) &&
      this.#plugins &&
      !this.#resolved.has(request.candidate)
    ) {
      const variant = this.#query
        .completeStylesheet(this.#text, this.#byteOffset(request.input), {
          catalog: this.#catalogAt(request.input),
        })
        .some((item) => item.label === request.candidate && item.variant);
      this.#resolved.set(
        request.candidate,
        await this.#plugins.query(
          variant && !request.candidate.endsWith(":") ? `${request.candidate}:` : request.candidate,
        ),
      );
    }
  }

  /**
   * Answers completion, hover, colors, sorting, and lint-fix requests.
   *
   * @param id - Resolver identity of the requested source.
   * @param request - Operation in the stylesheet's original UTF-16 coordinates. Cursor offsets must
   * be character boundaries within the captured text. Resolve uses the label, input, and snapshot
   * returned by completion. Completion and resolution accept `registryDirectives` and `snippets`,
   * editor-only settings whose defaults are false. Resolve requires the same settings.
   * @returns An answer with the captured source fingerprint, or undefined for another identity.
   * Completion includes original replacement ranges. Hover includes formatted CSS or documentation
   * when the input resolves. Resolve omits hover when its snapshot or label no longer matches.
   * Fixes select each report's first action when all edits address this revision and do not overlap
   * a selected action, then sort edits by offset. Sort uses captured CSS targets independently of
   * lint reporting. Colors include original utility or reference ranges; presentations replace
   * only a currently recognized range in this buffer. Link requests return an empty array;
   * the project host resolves plugin paths.
   * @throws If a matching utility definition has a malformed requirement or CSS formatting fails.
   */
  query(id: string, request: EditorRequest): EditorResult | undefined {
    if (id !== this.#id) {
      return undefined;
    }

    const result: EditorResult = {
      revision: this.#revision,
    };
    if (request.kind === "links") {
      return {
        ...result,
        links: [],
      };
    }
    if (request.kind === "colors" || request.kind === "colorPresentation") {
      const colors = this.#query.stylesheetColors(this.#text).map((color) => ({
        ...color,
        start: this.#utf16(color.start),
        end: this.#utf16(color.end),
      }));
      if (request.kind === "colors") {
        result.colors = colors.map(({ candidate: _candidate, ...color }) => color);
      } else {
        const color = colors.find(
          (color) => color.start === request.start && color.end === request.end,
        );
        if (color) {
          const newText = color.candidate
            ? colorReplacement(
                color.candidate,
                "",
                request.color,
                new CandidateQueries(this.#query),
              )
            : cssColor(request.color);
          if (newText) {
            result.edits = [
              {
                start: color.start,
                end: color.end,
                newText,
              },
            ];
          }
        }
      }
      return result;
    }
    if (request.kind === "completion") {
      result.completions = this.#query
        .completeStylesheet(this.#text, this.#byteOffset(request.offset), {
          registryDirectives: request.registryDirectives === true,
          snippets: request.snippets === true,
          catalog: this.#catalogAt(request.offset),
        })
        .map((item) => ({
          start: this.#utf16(item.start),
          end: this.#utf16(item.end),
          label: item.label,
          candidate: item.label,
          newText: item.label,
          snippet: request.snippets && item.variant ? completionSnippet(item.label) : undefined,
          retrigger: item.label.endsWith(":") || (!item.keyword && item.label.endsWith("-")),
          keyword: item.keyword,
          variant: item.variant,
          detail: item.detail ?? undefined,
          sortText: item.sortText ?? undefined,
          input: request.offset,
          snapshot: this.#completionSnapshot(request.registryDirectives, request.snippets),
        }));
    }
    if (request.kind === "hover") {
      result.hover = this.#hover(this.#text, request.offset);
    }
    if (
      request.kind === "resolve" &&
      request.snapshot === this.#completionSnapshot(request.registryDirectives, request.snippets)
    ) {
      const item = this.#query
        .completeStylesheet(this.#text, this.#byteOffset(request.input), {
          registryDirectives: request.registryDirectives === true,
          snippets: request.snippets === true,
          catalog: this.#catalogAt(request.input),
        })
        .find((item) => item.label === request.candidate);
      if (item) {
        const query = this.#resolved.get(request.candidate) ?? this.#query;
        const start = this.#utf16(item.start);
        const text =
          this.#text.slice(0, start) + item.label + this.#text.slice(this.#utf16(item.end));
        const variant =
          item.label.endsWith(":") && !item.keyword
            ? new CandidateQueries(query).variant(item.label, "utility")
            : undefined;
        const hover = variant
          ? {
              start: 0,
              end: 0,
              css: this.#query.formatCss(variant, this.#preview),
            }
          : this.#hover(text, start + item.label.length, query);
        if (hover) {
          result.hover = {
            ...hover,
            start: 0,
            end: 0,
            detail: this.#candidates.detail(hover.css),
          };
        }
      }
    }
    if (request.kind === "sort") {
      result.edits = this.#query
        .sortStylesheet(
          this.#text,
          request.protectedRanges?.map((range) => ({
            start: this.#byteOffset(range.start),
            end: this.#byteOffset(range.end),
          })),
        )
        .map((edit) => ({
          start: this.#utf16(edit.start),
          end: this.#utf16(edit.end),
          newText: edit.replacement,
        }));
    }
    if (request.kind === "fixes") {
      const edits: EditorEdit[] = [];
      for (const report of this.#reports) {
        const action = report.actions[0];
        if (!action?.edits.every((edit) => edit.location.revision === this.#revision)) {
          continue;
        }
        const proposed = action.edits.map((edit) => ({
          start: edit.location.startUtf16,
          end: edit.location.endUtf16,
          newText: edit.replacement,
        }));
        // An overlapping fix waits for the next pass, which analyzes the edited text again.
        if (
          proposed.some((edit) =>
            edits.some((other) => other.start < edit.end && edit.start < other.end),
          )
        ) {
          continue;
        }
        edits.push(...proposed);
      }
      edits.sort((a, b) => a.start - b.start || a.end - b.end);
      result.edits = edits;
    }
    return result;
  }

  /**
   * Pins completion resolution to the discovery and snippet settings used for its labels.
   * @param registryDirectives - Whether the request includes registry metadata.
   * @param snippets - Whether the client supports arbitrary variant snippets.
   * @returns A fingerprint of the captured snapshot and normalized editor settings.
   */
  #completionSnapshot(registryDirectives = false, snippets = false): string {
    return fingerprint(JSON.stringify([this.#snapshot, registryDirectives, snippets]));
  }

  /**
   * Returns the plugin suggestions for the candidate before a cursor.
   * @param offset - UTF-16 character boundary within the captured text.
   * @returns The finite catalog and the suggestions that {@link StylesheetEditor.prepare} loaded
   * for the candidate's input, without duplicates.
   */
  #catalogAt(offset: number): string[] {
    const input = catalogInput(this.#text.slice(0, offset));
    return [...new Set([...this.#catalog, ...(this.#inputCatalogs.get(input) ?? [])])];
  }

  /**
   * Converts a cursor in the original buffer to the native query's byte coordinates.
   * @param offset - UTF-16 character boundary within the captured text.
   * @returns The length in UTF-8 bytes of the text before the cursor.
   */
  #byteOffset(offset: number): number {
    return new TextEncoder().encode(this.#text.slice(0, offset)).length;
  }

  /**
   * Converts a native range boundary back to the original buffer's coordinates.
   * @param offset - UTF-8 character boundary within the captured bytes, returned by the native query.
   * @returns The UTF-16 offset at that boundary.
   */
  #utf16(offset: number): number {
    return new TextDecoder().decode(this.#bytes.subarray(0, offset)).length;
  }

  /**
   * Queries and formats a hover against original text or a completed copy of that text.
   * @param text - CSS buffer to query, including any synthetic completion replacement.
   * @param offset - UTF-16 character boundary within `text`.
   * @returns CSS and documentation at UTF-16 boundaries in `text`, or undefined for unresolved input.
   * Theme declarations retain their native spelling because the formatter expects complete rules.
   * @throws If candidate resolution or CSS formatting fails.
   */
  #hover(text: string, offset: number, query = this.#query): EditorResult["hover"] {
    const encoder = new TextEncoder();
    const bytes = encoder.encode(text);
    const hover = query.hoverStylesheet(text, encoder.encode(text.slice(0, offset)).length);
    if (!hover) {
      return undefined;
    }

    const css = hover.css.startsWith("--")
      ? hover.css
      : hover.css
        ? this.#query.formatCss(hover.css, this.#preview)
        : "";
    const decoder = new TextDecoder();
    return {
      start: decoder.decode(bytes.subarray(0, hover.start)).length,
      end: decoder.decode(bytes.subarray(0, hover.end)).length,
      css,
      documentation: hover.documentation || undefined,
    };
  }
}
