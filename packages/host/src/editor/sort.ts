import type { UtilitySortOptions } from "@stillwind/config";
import type { PreparedSource } from "@stillwind/projection";
import ts from "typescript-6";

import type { CandidateQueries } from "./candidates.ts";
import type { EditorEdit } from "./snapshot.ts";
import type { SourceIndex, UtilityLiteral } from "./source-index.ts";

/** Sorts safe source containers, composing supplied lint fixes before ordering their contents. */
export function sortSource(
  source: PreparedSource,
  index: SourceIndex,
  candidates: CandidateQueries,
  options: UtilitySortOptions,
  seed: EditorEdit[] = [],
  allows: (start: number, end: number) => boolean = () => true,
  baseCss: (asset: NonNullable<ReturnType<SourceIndex["asset"]>>) => string = () => "",
): EditorEdit[] {
  let edits = [...seed];

  if (options.stringLiterals !== false) {
    for (const literal of index.literals.filter((literal) => literal.source === source)) {
      const start = literal.boundaries[0],
        end = literal.boundaries.at(-1)!;
      if (!allows(start, end)) {
        continue;
      }

      const tokens = [...literal.text.matchAll(/\S+/gu)].map((match) => {
        const edit = seed.find(
          (edit) =>
            edit.start === literal.boundaries[match.index] &&
            edit.end === literal.boundaries[match.index + match[0].length],
        );
        return edit ? decodeReplacement(edit.newText, literal) : match[0];
      });
      const sorted = sortRuns(tokens, (a, b) =>
        candidates.compare(literal.prefix + a, literal.prefix + b),
      );
      if (sorted.every((token, position) => token === literal.text.match(/\S+/gu)?.[position])) {
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

  if (options.arrays !== false) {
    const arrays = index.arrays
      .filter((array) => array.source === source)
      .sort((a, b) => a.node.end - a.node.pos - (b.node.end - b.node.pos));
    for (const array of arrays) {
      const items = [...array.node.elements].map((node) => {
        const range = index.range(source, node);
        const offset = range ? range[0] - node.getStart() : 0;
        const start = node.getFullStart() + offset,
          end = range?.[1] ?? 0;
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
        let rank = -1,
          name = "",
          css = "";
        if (ts.isStringLiteralLike(node)) {
          const parsed = ts.createSourceFile(
            "literal.ts",
            `const value=${text};`,
            ts.ScriptTarget.Latest,
            true,
          );
          const declaration = (parsed.statements[0] as ts.VariableStatement | undefined)
            ?.declarationList?.declarations[0];
          const literal = declaration?.initializer;
          if (
            literal &&
            ts.isStringLiteralLike(literal) &&
            literal.text.trim() &&
            literal.text
              .trim()
              .split(/\s+/)
              .every((token) => candidates.preview(array.prefix + token))
          ) {
            rank = 2;
            name = literal.text.trim();
          }
        } else {
          const asset = index.asset(source, node);
          if (asset?.identity.kind === "ref" && ts.isIdentifier(node)) {
            rank = 0;
            name = node.getText();
          }
          if (asset?.identity.kind === "style" && ts.isIdentifier(node) && index.pureBase(asset)) {
            rank = 1;
            name = node.getText();
            css = baseCss(asset);
          }
        }
        const ambiguousComment =
          /^[^\r\n]*(?:\/\/|\/\*)/.test(text) && node.pos > array.node.getStart() + 1;
        return {
          node,
          start,
          end,
          text,
          rank:
            range &&
            !ambiguousComment &&
            source.document.code.slice(start, end) ===
              node.getSourceFile().text.slice(node.getFullStart(), node.end)
              ? rank
              : -1,
          name,
          css,
        };
      });

      for (let position = 0; position < items.length; position++) {
        const item = items[position],
          next = items[position + 1];
        const tail = source.document.code.slice(
          item.end,
          next?.start ?? index.range(source, array.node)?.[1] ?? item.end,
        );
        const attachedToPrevious =
          /^[^\r\n]*(?:\/\/|\/\*)/.test(tail) ||
          (next &&
            /^[^\r\n]*(?:\/\/|\/\*)/.test(
              next.text.slice(0, next.text.length - (next.node.end - next.node.getStart())),
            ));
        if (attachedToPrevious) {
          item.rank = -1;
        }
      }

      const sorted = [...items];
      for (let position = 1; position < sorted.length; position++) {
        for (let cursor = position; cursor > 0; cursor--) {
          const left = sorted[cursor - 1],
            right = sorted[cursor];
          if (
            left.rank < 0 ||
            right.rank < 0 ||
            (left.rank === 1 &&
              right.rank === 1 &&
              !candidates.canReorderStyles(left.css, right.css)) ||
            !allows(Math.min(left.start, right.start), Math.max(left.end, right.end))
          ) {
            break;
          }
          const order =
            left.rank - right.rank ||
            (left.rank === 2
              ? compareLists(left.name, right.name, candidates)
              : left.name < right.name
                ? -1
                : left.name > right.name
                  ? 1
                  : 0);
          if (order <= 0) {
            break;
          }
          [sorted[cursor - 1], sorted[cursor]] = [right, left];
        }
      }

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
        if (items.slice(start, position).some((item) => item.rank < 0)) {
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

function compareLists(left: string, right: string, candidates: CandidateQueries): number {
  const a = left.split(/\s+/),
    b = right.split(/\s+/);
  for (let position = 0; position < Math.min(a.length, b.length); position++) {
    const comparison = candidates.compare(a[position], b[position]);
    if (comparison) {
      return comparison;
    }
  }
  return a.length - b.length;
}
