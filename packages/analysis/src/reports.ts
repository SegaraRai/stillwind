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
      return [
        {
          project: entry.project,
          report,
        },
      ];
    }),
  );
}

/**
 * Returns the website page that explains a report code.
 * @param code - Compiler code, such as `SW01001`, or lint rule name, such as `unknown-utility`.
 * @returns The page URL, or undefined for codes without a page, such as `SW_UNVERIFIED`.
 */
export function documentationUrl(code: string): string | undefined {
  const section = documentationSection(code);
  return section && `https://stillwind.dev/docs/${section}/${code}`;
}

/**
 * Returns the documentation section of a report code.
 * @param code - Report code of a diagnostic.
 * @returns `errors` for compiler codes, `lints` for lint rule names, and otherwise undefined.
 */
export function documentationSection(code: string): "errors" | "lints" | undefined {
  return /^SW\d+$/.test(code) ? "errors" : /^[a-z][a-z-]*$/.test(code) ? "lints" : undefined;
}
