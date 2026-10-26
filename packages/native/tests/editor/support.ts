/**
 * Returns the labels of a native candidate completion.
 *
 * @param query - Native editor query of the binding under test.
 * @param args - Prefix, optional context, and optional snippet support.
 * @returns Labels in the order that the query returns them.
 */
export function completionLabels(
  query: {
    completeCandidate(...args: unknown[]): {
      label: string;
    }[];
  },
  ...args: unknown[]
): string[] {
  return query.completeCandidate(...args).map((item) => item.label);
}
