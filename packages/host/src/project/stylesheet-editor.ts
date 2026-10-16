import { fingerprint, type BindingReport } from "@stillwind/native";

import type { EditorEdit, EditorRequest, EditorResult } from "../editor/index.ts";

/** Answers editor requests for one linted stylesheet from its captured lint reports. */
export class StylesheetEditor {
  /** Resolver identity of the stylesheet. */
  readonly #id: string;
  /** Fingerprint of the captured stylesheet text that every report range addresses. */
  readonly #revision: string;
  /** Lint reports whose primary location is this stylesheet. */
  readonly #reports: readonly BindingReport[];

  /**
   * Captures the reports of one stylesheet revision.
   * @param id - Resolver identity of the stylesheet.
   * @param text - Complete stylesheet text that the reports were computed from.
   * @param reports - Reports whose primary location is this stylesheet. The array is copied.
   */
  constructor(id: string, text: string, reports: readonly BindingReport[]) {
    this.#id = id;
    this.#revision = fingerprint(text);
    this.#reports = structuredClone(reports);
  }

  /**
   * Answers a fixes or sort request. Other requests return only the revision.
   *
   * @param id - Resolver identity of the requested source.
   * @param request - Operation in the stylesheet's original UTF-16 coordinates.
   * @returns For fixes, the first action of each report whose edits all address this revision and
   * do not overlap an earlier selected action. Edits are in ascending offset order. Sort returns no
   * edits, because utility sorting in stylesheets is a lint fix. Another identity returns undefined.
   */
  query(id: string, request: EditorRequest): EditorResult | undefined {
    if (id !== this.#id) {
      return undefined;
    }
    const result: EditorResult = { revision: this.#revision };
    if (request.kind === "sort") {
      result.edits = [];
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
}
