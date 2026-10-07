import type { UtilitySortOptions } from "@stillwind/config";
import type { PreparedSource } from "@stillwind/projection";
import ts from "typescript-6";

import type { CandidateQueries } from "./candidates.ts";
import { orderInputs, type InputOrderItem } from "./input-order.ts";
import type { EditorEdit } from "./snapshot.ts";
import type { SourceIndex, UtilityLiteral } from "./source-index.ts";

/**
 * Sorts recognized literals and arrays in `source` using `options` and the active
 * design system in `candidates`. `index` must describe the unchanged source graph.
 * `seed` contains nonoverlapping edits in original UTF-16 coordinates. The result
 * composes these edits with sorting edits, also in original coordinates.
 * `allows` can reject exchanged ranges, for example to preserve lint suppression.
 */
export function sortSource(
  source: PreparedSource,
  index: SourceIndex,
  candidates: CandidateQueries,
  options: UtilitySortOptions,
  seed: EditorEdit[] = [],
  allows?: (start: number, end: number) => boolean,
): EditorEdit[] {
  let edits = [...seed];
  const permits = allows ?? (() => true);

  if (options.stringLiterals !== false) {
    const literals = index.literals
      .filter(
        (literal) =>
          literal.source === source && permits(literal.boundaries[0], literal.boundaries.at(-1)!),
      )
      .map((literal) => {
        const matches = [...literal.text.matchAll(/\S+/gu)];
        const original = matches.map((match) => match[0]);
        const tokens = matches.map((match) => {
          const edit = seed.find(
            (edit) =>
              edit.start === literal.boundaries[match.index] &&
              edit.end === literal.boundaries[match.index + match[0].length],
          );
          return edit ? decodeReplacement(edit.newText, literal) : match[0];
        });
        return { literal, original, tokens };
      });
    const compare = candidates.order(
      literals.flatMap(({ literal, tokens }) => tokens.map((token) => literal.prefix + token)),
    );
    for (const { literal, original, tokens } of literals) {
      const start = literal.boundaries[0];
      const end = literal.boundaries.at(-1)!;
      const sorted = sortRuns(tokens, (a, b) => compare(literal.prefix + a, literal.prefix + b));
      if (sorted.every((token, position) => token === original[position])) {
        continue;
      }

      let position = 0;
      const newText = escapeContent(
        literal.text.replace(/\S+/gu, () => sorted[position++]),
        literal,
      );
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

/** Applies nonoverlapping replacements in descending original offset order. */
export function applyEdits(source: string, edits: readonly EditorEdit[]): string {
  for (const edit of [...edits].sort((a, b) => b.start - a.start)) {
    source = source.slice(0, edit.start) + edit.newText + source.slice(edit.end);
  }
  return source;
}

export function escapeContent(text: string, literal: UtilityLiteral): string {
  const quote = ts.isTemplateLiteralToken(literal.node) ? "`" : literal.node.getText()[0];
  let escaped = text
    .replaceAll("\\", "\\\\")
    .replaceAll(quote, `\\${quote}`)
    .replaceAll("\r", "\\r")
    .replaceAll("\n", "\\n");
  if (quote === "`") {
    escaped = escaped.replaceAll("${", "\\${");
  }
  return escaped;
}

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

function absorb(edits: EditorEdit[], replacement: EditorEdit): EditorEdit[] {
  return [
    ...edits.filter((edit) => edit.end <= replacement.start || edit.start >= replacement.end),
    replacement,
  ];
}

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
