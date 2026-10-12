import type { WorkspaceResult } from "./workspace.ts";

/**
 * Collects distinct reports per project while preserving their analysis order.
 * @param results - Analysis results with their project membership.
 * @returns Reports in first-occurrence order. Equal reports from different projects remain separate.
 */
export function lintReports(results: readonly WorkspaceResult[]) {
  const seen = new Set<string>();
  return results.flatMap((entry) =>
    entry.result.reports.flatMap((report) => {
      const key = JSON.stringify([entry.project, report]);
      if (seen.has(key)) {
        return [];
      }
      seen.add(key);
      return [{ project: entry.project, report }];
    }),
  );
}
