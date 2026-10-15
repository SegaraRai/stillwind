import type { AnalysisWorkspace, EditorEdit, WorkspaceResult } from "@stillwind/host";
import { fingerprint } from "@stillwind/native";

/** Failed rewrite planning with the analysis results that prevented writing. */
export class RewriteFailure extends Error {
  constructor(
    message: string,
    readonly results: WorkspaceResult[],
  ) {
    super(message);
    this.name = "RewriteFailure";
  }
}

/** Original buffer and ranges excluded by a formatter adapter. Paths are absolute. */
export interface RewriteInput {
  /** Absolute target path used for workspace membership and dependency resolution. */
  file: string;
  /** Complete original text against which edits are measured. */
  source: string;
  /** Half-open UTF-16 ranges in source. Only sorting accepts protected ranges. */
  protectedRanges?: readonly {
    /** Inclusive original UTF-16 offset. */
    start: number;
    /** Exclusive original UTF-16 offset. */
    end: number;
  }[];
}

/** Replacement text and independent project findings for one original buffer. */
export interface RewriteResult {
  /** Absolute path copied from the corresponding input. */
  file: string;
  /** Final text after sorting or converged fixes. */
  code: string;
  /** Sorting edits in the input buffer. Iterated lint fixes omit these edits. */
  edits: EditorEdit[];
  /** Findings for the analyzed input of the final pass, in project order. */
  results: WorkspaceResult[];
}

/** Owns buffer versions while computing changes against one workspace. Calls must be serialized. */
export class SourceRewriter {
  /** Increasing version assigned to every buffer in each rewrite pass. */
  #version = 0;

  /**
   * Attaches rewriting to a workspace whose buffers the caller reserves for this rewriter.
   * @param workspace - Open workspace. Serialize rewrite calls and avoid concurrent updates to input paths.
   */
  constructor(private readonly workspace: AnalysisWorkspace) {}

  /**
   * Computes sorting or converged lint fixes without saving files. All input buffers are visible
   * to dependency analysis. After distinct paths are validated, captured
   * buffers are removed even when rewriting fails.
   * @param inputs - Distinct absolute paths and complete text. Only sorting uses protected ranges.
   * @param operation - One sort pass or lint-fix passes until the text stops changing.
   * @returns Final buffers in input order. Fixes return no edits because their offsets span several passes.
   * @throws If paths repeat, syntax is invalid, analysis is incomplete or fails, edits are invalid
   * or stale, projects disagree, or fixes do not converge in ten passes.
   */
  async rewrite(
    inputs: readonly RewriteInput[],
    operation: "sort" | "fixes",
  ): Promise<RewriteResult[]> {
    if (new Set(inputs.map((input) => input.file)).size !== inputs.length) {
      throw new Error("Rewrite inputs must have distinct file paths");
    }
    let buffers = inputs;
    const seen = new Set<string>();
    try {
      this.workspace.invalidate();
      for (let pass = 0; pass < 10; pass++) {
        const key = JSON.stringify(buffers.map((input) => input.source));
        if (seen.has(key)) {
          throw new Error("Stillwind fixes did not converge");
        }
        seen.add(key);
        const version = ++this.#version;
        for (const input of buffers) {
          this.workspace.update(input.file, input.source, version);
        }
        const outputs: RewriteResult[] = [];
        for (const input of buffers) {
          outputs.push(await this.#rewriteBuffer(input, operation));
        }
        if (
          operation === "sort" ||
          outputs.every((output, index) => output.code === buffers[index].source)
        ) {
          return outputs.map((output) => ({
            ...output,
            edits: operation === "sort" ? output.edits : [],
          }));
        }
        buffers = outputs.map((output) => ({
          file: output.file,
          source: output.code,
        }));
      }
      throw new Error("Stillwind fixes exceeded ten passes");
    } finally {
      for (const input of inputs) {
        this.workspace.closeDocument(input.file);
      }
    }
  }

  /**
   * Analyzes one installed buffer and requires matching project answers to produce the same text.
   * @param input - Buffer already installed in the workspace for the current pass.
   * @param operation - Sort or fixes query to apply to the original text of this pass.
   * @returns The first project's edits and all project findings, or unchanged text outside every scope.
   * @throws If analysis is incomplete, syntax is invalid, answers are missing or stale, or projects disagree.
   */
  async #rewriteBuffer(input: RewriteInput, operation: "sort" | "fixes"): Promise<RewriteResult> {
    const results = await this.workspace.analyze(input.file);
    if (results.some(({ result }) => result.status !== "complete")) {
      const reasons = results.flatMap(({ result }) =>
        result.status === "complete"
          ? []
          : [result.error ?? result.graph?.unverified[0]?.reason ?? result.status],
      );
      throw new RewriteFailure(
        `Incomplete analysis for ${input.file}: ${reasons.join("; ")}`,
        results,
      );
    }
    const syntaxErrors = results.flatMap(({ result }) =>
      result.reports.filter((report) => ["SW10001", "SW10002"].includes(report.code)),
    );
    if (syntaxErrors.length) {
      throw new RewriteFailure(
        `Cannot rewrite ${input.file}: ${syntaxErrors.map((report) => report.message).join("; ")}`,
        results,
      );
    }
    const answers = await this.workspace.query(
      input.file,
      operation === "sort"
        ? {
            kind: "sort",
            protectedRanges: input.protectedRanges,
          }
        : {
            kind: "fixes",
          },
    );
    if (answers.length !== results.length) {
      throw new Error(`Missing rewrite result for ${input.file}`);
    }
    const revision = fingerprint(input.source);
    const candidates = answers.map(({ result }) => {
      if (result.revision !== revision) {
        throw new Error(`Stale edits for ${input.file}`);
      }
      const edits = result.edits ?? [];
      return {
        code: applySourceEdits(input.source, edits),
        edits,
      };
    });
    const first = candidates[0] ?? {
      code: input.source,
      edits: [],
    };
    if (candidates.some((candidate) => candidate.code !== first.code)) {
      throw new Error(`Conflicting project rewrites for ${input.file}`);
    }
    return {
      file: input.file,
      ...first,
      results,
    };
  }
}

/**
 * Applies edits from the end of the original text after validating their ranges.
 * @param source - Original text from which all edit offsets were measured.
 * @param edits - Half-open ranges with safe integer UTF-16 offsets, in any order. The array is unchanged.
 * @returns Text with every replacement applied.
 * @throws If a range is invalid, outside the source, or overlaps another range.
 */
export function applySourceEdits(source: string, edits: readonly EditorEdit[]): string {
  const ordered = [...edits].sort((a, b) => a.start - b.start || a.end - b.end);
  let end = 0;
  for (const edit of ordered) {
    if (
      !Number.isSafeInteger(edit.start) ||
      !Number.isSafeInteger(edit.end) ||
      edit.start < end ||
      edit.end < edit.start ||
      edit.end > source.length
    ) {
      throw new Error("Invalid or overlapping source edits");
    }
    end = edit.end;
  }
  for (const edit of ordered.reverse()) {
    source = source.slice(0, edit.start) + edit.newText + source.slice(edit.end);
  }
  return source;
}
