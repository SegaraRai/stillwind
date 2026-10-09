import type { UtilitySortOptions } from "@stillwind/config";
import type { PreparedSource } from "@stillwind/projection";
import ts from "typescript-6";

import type { CandidateQueries } from "./candidates.ts";
import { orderInputs, type InputOrderItem } from "./input-order.ts";
import type { EditorEdit } from "./snapshot.ts";
import type { SourceIndex, UtilityLiteral } from "./source-index.ts";

/**
 * Composes existing edits with utility sorting and grouping of direct style inputs.
 * Literal sorting applies only to ordinary utilities. Containers are processed from the
 * smallest to the largest so an outer replacement includes edits to nested inputs.
 * Unknown syntax, ambiguous comments, and text that cannot map exactly delimit grouping.
 *
 * @param source - The same prepared source object stored in `index`, with an unchanged buffer.
 * @param index - Index built from that source graph before any edits are applied.
 * @param candidates - Candidate query for the design system that compiled the source.
 * @param options - Sorting targets. False returns only the seed edits in source order.
 * @param seed - Nonoverlapping replacements in original UTF-16 coordinates. Inside a sortable
 * literal, each edit must replace a complete utility token. Edits inside grouped containers must
 * stay within one input. Replacement text must contain any JS escapes needed at its range.
 * @param allows - Optional predicate that approves a whole literal's content range or the
 * range covering an adjacent input exchange. Every range uses original UTF-16 coordinates.
 * @returns Edits in ascending original offset order. The source and seed array remain unchanged.
 * @throws If a matching utility definition has a malformed requirement or native ranking fails.
 */
export function sortSource(
  source: PreparedSource,
  index: SourceIndex,
  candidates: CandidateQueries,
  options: UtilitySortOptions | false,
  seed: readonly EditorEdit[] = [],
  allows?: (start: number, end: number) => boolean,
): EditorEdit[] {
  if (options === false) {
    return seed.toSorted((a, b) => a.start - b.start);
  }

  let edits = [...seed];
  const permits = allows ?? (() => true);

  if (options.stringLiterals !== false) {
    const literals = index.literals
      .filter(
        (literal) =>
          literal.source === source &&
          literal.context === "utility" &&
          permits(literal.boundaries[0], literal.boundaries.at(-1)!),
      )
      .map((literal) => {
        const matches = [...literal.text.matchAll(/\S+/gu)];
        const original = matches.map((match) => match[0]);
        const spellings = matches.map((match) => {
          const start = literal.boundaries[match.index];
          const end = literal.boundaries[match.index + match[0].length];
          const edit = seed.find((edit) => edit.start === start && edit.end === end);
          return {
            token: edit ? decodeReplacement(edit.newText, literal) : match[0],
            text: edit?.newText ?? source.document.code.slice(start, end),
          };
        });
        const tokens = spellings.map((spelling) => spelling.token);
        return {
          literal,
          original,
          tokens,
          matches,
          spellings,
        };
      });
    const compare = candidates.order(
      literals.flatMap(({ literal, tokens }) => tokens.map((token) => literal.prefix + token)),
    );
    for (const { literal, original, tokens, matches, spellings } of literals) {
      const start = literal.boundaries[0];
      const end = literal.boundaries.at(-1)!;
      const sorted = sortRuns(tokens, (a, b) => compare(literal.prefix + a, literal.prefix + b));
      if (sorted.every((token, position) => token === original[position])) {
        continue;
      }

      let cursor = start;
      let newText = "";
      const queues = new Map<
        string,
        {
          /** Authored or seeded spellings of duplicate candidates, in occurrence order. */
          texts: string[];
          /** Zero-based index of the next spelling to consume. */
          position: number;
        }
      >();
      for (const spelling of spellings) {
        let queue = queues.get(spelling.token);
        if (!queue) {
          queue = {
            texts: [],
            position: 0,
          };
          queues.set(spelling.token, queue);
        }
        queue.texts.push(spelling.text);
      }
      for (const [position, match] of matches.entries()) {
        const tokenStart = literal.boundaries[match.index];
        const tokenEnd = literal.boundaries[match.index + match[0].length];
        const queue = queues.get(sorted[position])!;
        newText += source.document.code.slice(cursor, tokenStart) + queue.texts[queue.position++];
        cursor = tokenEnd;
      }
      newText += source.document.code.slice(cursor, end);
      edits = absorb(edits, {
        start,
        end,
        newText,
      });
    }
  }

  if (options.arrays !== false || options.attributes === true) {
    const containers = index.containers
      .filter((container) => container.source === source)
      .filter(
        (container) => options.arrays !== false || ts.isObjectLiteralExpression(container.node),
      )
      .sort((a, b) => a.node.end - a.node.pos - (b.node.end - b.node.pos));
    for (const container of containers) {
      const elements = ts.isArrayLiteralExpression(container.node)
        ? container.node.elements
        : ts.isCallExpression(container.node)
          ? container.node.arguments
          : container.node.properties;
      const items = [...elements].map((node) => {
        const range = index.range(source, node);
        const offset = range ? range[0] - node.getStart() : 0;
        const start = node.getFullStart() + offset;
        const end = range?.[1] ?? 0;
        const text = range
          ? applyEdits(
              source.document.code.slice(start, end),
              edits
                .filter((edit) => edit.start >= start && edit.end <= end)
                .map((edit) => ({
                  ...edit,
                  start: edit.start - start,
                  end: edit.end - start,
                })),
            )
          : "";
        const order: InputOrderItem["order"] = index.inputOrder(source, node);
        const enabledOrder =
          options.arrays !== false || order.kind === "attributes"
            ? order
            : {
                kind: "barrier" as const,
              };
        const ambiguousComment = /^[^\r\n]*(?:\/\/|\/\*)/.test(text) && node.pos > elements.pos;
        return {
          node,
          start,
          end,
          text,
          order:
            range &&
            !ambiguousComment &&
            source.document.code.slice(start, end) ===
              node.getSourceFile().text.slice(node.getFullStart(), node.end)
              ? enabledOrder
              : {
                  kind: "barrier" as const,
                },
        };
      });

      for (let position = 0; position < items.length; position++) {
        const item = items[position];
        const next = items[position + 1];
        const tail = source.document.code.slice(
          item.end,
          next?.start ?? index.range(source, container.node)?.[1] ?? item.end,
        );
        const attachedToPrevious =
          /^[^\r\n]*(?:\/\/|\/\*)/.test(tail) ||
          (next &&
            /^[^\r\n]*(?:\/\/|\/\*)/.test(
              next.text.slice(0, next.text.length - (next.node.end - next.node.getStart())),
            ));
        if (attachedToPrevious) {
          item.order = {
            kind: "barrier",
          };
        }
      }

      const sorted = orderInputs(items, options.attributes === true, allows);
      if (sorted.every((item, position) => item === items[position])) {
        continue;
      }

      // Replace changed runs independently so unmapped nodes, holes, and spreads remain untouched.
      for (let position = 0; position < items.length;) {
        if (items[position] === sorted[position]) {
          position++;
          continue;
        }

        const start = position++;
        while (position < items.length && items[position] !== sorted[position]) {
          position++;
        }
        if (items.slice(start, position).some((item) => item.order.kind === "barrier")) {
          continue;
        }

        const edit = {
          start: items[start].start,
          end: items[position - 1].end,
          newText: sorted
            .slice(start, position)
            .map((item) => item.text)
            .join(","),
        };
        edits = absorb(edits, edit);
      }
    }
  }

  return edits.sort((a, b) => a.start - b.start);
}

/**
 * Applies replacements from the end of a document so all ranges keep their original meaning.
 *
 * @param source - Original document text, or a slice whose coordinates match `edits`.
 * @param edits - Nonoverlapping half-open UTF-16 ranges within `source`, in any order.
 * The caller must provide valid boundaries because this function does not validate them.
 * @returns The replaced text. The edit array and its objects remain unchanged.
 */
export function applyEdits(source: string, edits: readonly EditorEdit[]): string {
  for (const edit of [...edits].sort((a, b) => b.start - a.start)) {
    source = source.slice(0, edit.start) + edit.newText + source.slice(edit.end);
  }
  return source;
}

/**
 * Escapes replacement content for an existing JS string or template token.
 *
 * @param text - Decoded replacement text without surrounding delimiters.
 * @param literal - Indexed literal whose AST token supplies the quote or backtick delimiter.
 * @returns Content without delimiters, with backslashes, the delimiter, tabs, CR, and LF escaped.
 * Template content also escapes `${` so it cannot introduce an interpolation.
 */
export function escapeContent(text: string, literal: UtilityLiteral): string {
  const quote = ts.isTemplateLiteralToken(literal.node) ? "`" : literal.node.getText()[0];
  let escaped = text
    .replaceAll("\\", "\\\\")
    .replaceAll(quote, `\\${quote}`)
    .replaceAll("\t", "\\t")
    .replaceAll("\r", "\\r")
    .replaceAll("\n", "\\n");
  if (quote === "`") {
    escaped = escaped.replaceAll("${", "\\${");
  }
  return escaped;
}

/**
 * Decodes a seeded replacement using the original literal's delimiter.
 *
 * @param text - Source spelling of replacement content that can be wrapped in that delimiter.
 * @param literal - Indexed literal that supplies the quote or backtick delimiter.
 * @returns TypeScript's decoded string value, or the supplied text when parsing produces a
 * different initializer kind. Parser diagnostics are not checked.
 */
function decodeReplacement(text: string, literal: UtilityLiteral): string {
  const quote = ts.isTemplateLiteralToken(literal.node) ? "`" : literal.node.getText()[0];
  const file = ts.createSourceFile(
    "literal.ts",
    `const value=${quote}${text}${quote};`,
    ts.ScriptTarget.Latest,
    true,
  );
  const node = (file.statements[0] as ts.VariableStatement).declarationList.declarations[0]
    .initializer;
  return node && ts.isStringLiteralLike(node) ? node.text : text;
}

/**
 * Replaces edits covered by a larger, already composed replacement.
 *
 * @param edits - Existing edits in original coordinates. Every overlapping edit must be fully
 * contained in `replacement` and already reflected in its text.
 * @param replacement - Replacement whose text includes all contained edits.
 * @returns A new array with overlapping edits removed and the replacement appended.
 * The result is not sorted, and the input edit objects remain unchanged.
 */
function absorb(edits: readonly EditorEdit[], replacement: EditorEdit): EditorEdit[] {
  return [
    ...edits.filter((edit) => edit.end <= replacement.start || edit.start >= replacement.end),
    replacement,
  ];
}

/**
 * Sorts contiguous runs of resolved utilities while keeping unresolved spellings in place.
 *
 * @param values - Decoded candidates in their original order.
 * @param compare - Compiler comparator prepared for these spellings. A null self-comparison
 * marks a boundary, and a null comparison within a run treats that pair as equal.
 * @returns A new array with stable ordering inside each run. `values` remains unchanged.
 */
function sortRuns(
  values: string[],
  compare: (left: string, right: string) => number | null,
): string[] {
  const result = [...values];
  let start = 0;
  for (let position = 0; position <= values.length; position++) {
    if (position === values.length || compare(values[position], values[position]) === null) {
      result.splice(
        start,
        position - start,
        ...values.slice(start, position).sort((a, b) => compare(a, b) ?? 0),
      );
      start = position + 1;
    }
  }
  return result;
}
