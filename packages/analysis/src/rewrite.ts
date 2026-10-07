import type { AnalysisWorkspace, EditorEdit, WorkspaceResult } from "@stillwind/host";
import { fingerprint } from "@stillwind/native";

/** Original buffer and ranges excluded by a formatter adapter. Paths are absolute. */
export interface RewriteInput {
  file: string;
  source: string;
  /** Half-open UTF-16 ranges in source. Only sorting accepts protected ranges. */
  protectedRanges?: readonly {
    start: number;
    end: number;
  }[];
}

/** Replacement text and independent project findings for one original buffer. */
export interface RewriteResult {
  file: string;
  code: string;
  /** Sorting edits in the input buffer. Iterated lint fixes omit these edits. */
  edits: EditorEdit[];
  results: WorkspaceResult[];
}

/** Owns buffer versions while computing changes against one workspace. Calls must be serialized. */
export class SourceRewriter {
  #version = 0;

  constructor(private readonly workspace: AnalysisWorkspace) {}

  /**
   * Computes sorting or converged lint fixes without saving files. All input buffers are visible
   * to dependency analysis. Throws on syntax errors, incomplete analysis, conflicting project outputs, stale
   * edits, or fixes that do not converge in ten passes. Removes captured buffers before returning.
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

  async #rewriteBuffer(input: RewriteInput, operation: "sort" | "fixes"): Promise<RewriteResult> {
    const results = await this.workspace.analyze(input.file);
    if (results.some(({ result }) => result.status !== "complete")) {
      const reasons = results.flatMap(({ result }) =>
        result.status === "complete"
          ? []
          : [result.error ?? result.graph?.unverified[0]?.reason ?? result.status],
      );
      throw new Error(`Incomplete analysis for ${input.file}: ${reasons.join("; ")}`);
    }
    const syntaxErrors = results.flatMap(({ result }) =>
      result.reports.filter((report) => ["SW01012", "SW01013"].includes(report.code)),
    );
    if (syntaxErrors.length) {
      throw new Error(
        `Cannot rewrite ${input.file}: ${syntaxErrors.map((report) => report.message).join("; ")}`,
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

/** Applies edits in original UTF-16 coordinates. Throws on out-of-bounds or overlapping edits. */
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
