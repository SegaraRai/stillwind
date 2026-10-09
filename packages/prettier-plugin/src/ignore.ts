import type { Parser, ParserOptions } from "prettier";
import { parsers as typescriptParsers } from "prettier/plugins/typescript";

/** Original text that Stillwind sorting must leave untouched. */
interface SourceRange {
  /** Inclusive UTF-16 offset in the original buffer. */
  start: number;
  /** Exclusive UTF-16 offset in the original buffer. */
  end: number;
}

/**
 * Finds ignored nodes and returns half-open UTF-16 ranges in the original source.
 * Vue and Astro script bodies are parsed separately when they contain ignore directives.
 *
 * @param ast - AST parsed from `source` by `parser` before Stillwind edits are applied.
 * @param parser - Parser whose location functions return original UTF-16 offsets for that AST.
 * @param source - Original buffer, including comments and any embedded script delimiters.
 * @param options - Parser options reused for embedded TypeScript parsing with its own original text.
 * @returns Protected ranges in discovery order. Ranges can overlap, and an unmatched ignore-start
 * protects the remaining source. Neither the AST nor the options object is changed.
 * @throws If a parser location function fails or an embedded script cannot be parsed.
 */
export async function ignoredRanges(
  ast: unknown,
  parser: Parser,
  source: string,
  options: ParserOptions,
): Promise<SourceRange[]> {
  const nodes: (SourceRange & {
    /** Whether the parser's kind string identifies this node as a comment. */
    comment: boolean;
  })[] = [];
  const scripts: SourceRange[] = [];
  const seen = new Set<object>();

  /**
   * Collects valid node locations and embedded scripts while avoiding cycles and AST metadata.
   *
   * @param value - Root, child, or array from the supplied AST. Visited objects are added to `seen`,
   * and valid source ranges are appended to `nodes` and `scripts` without changing those objects.
   * @throws If the parser cannot provide a location for a node with a string kind.
   */
  const visit = (value: unknown) => {
    if (!value || typeof value !== "object" || seen.has(value)) {
      return;
    }
    seen.add(value);
    if (Array.isArray(value)) {
      for (const child of value) {
        visit(child);
      }
      return;
    }
    const node = value as Record<string, unknown>;
    const kind = node.type ?? node.kind;
    if (typeof kind === "string") {
      const start = parser.locStart(node);
      const end = parser.locEnd(node);
      if (
        Number.isInteger(start) &&
        Number.isInteger(end) &&
        start >= 0 &&
        end > start &&
        end <= source.length
      ) {
        nodes.push({ start, end, comment: /comment|^(?:Line|Block)$/i.test(kind) });
        if (kind === "AstroFrontmatter") {
          scripts.push({ start: start + 3, end: end - 3 });
        } else if (kind === "element" && node.name === "script" && Array.isArray(node.children)) {
          for (const child of node.children) {
            scripts.push({ start: parser.locStart(child), end: parser.locEnd(child) });
          }
        }
      }
    }
    for (const [key, child] of Object.entries(node)) {
      if (!["parent", "tokens", "loc", "position"].includes(key)) {
        visit(child);
      }
    }
  };
  visit(ast);

  const ranges: SourceRange[] = [];
  const comments = nodes.filter((node) => node.comment).sort((a, b) => a.start - b.start);
  let rangeStart: number | undefined;
  for (const comment of comments) {
    const text = source
      .slice(comment.start, comment.end)
      .replace(/^(?:\/\/|\/\*|<!--)|(?:\*\/|-->)$/g, "")
      .trim();
    if (text === "prettier-ignore-start") {
      rangeStart = comment.end;
    } else if (text === "prettier-ignore-end" && rangeStart !== undefined) {
      ranges.push({
        start: rangeStart,
        end: comment.start,
      });
      rangeStart = undefined;
    } else if (text === "prettier-ignore") {
      const next = nodes
        .filter(
          (node) =>
            !node.comment && node.start >= comment.end && source.slice(node.start, node.end).trim(),
        )
        .sort((a, b) => a.start - b.start || b.end - a.end)[0];
      if (next) {
        ranges.push({
          start: next.start,
          end: next.end,
        });
      }
    }
  }
  if (rangeStart !== undefined) {
    ranges.push({
      start: rangeStart,
      end: source.length,
    });
  }

  for (const script of scripts) {
    const text = source.slice(script.start, script.end);
    if (text.includes("prettier-ignore")) {
      const scriptParser = typescriptParsers.typescript;
      const scriptOptions = { ...options, originalText: text };
      const scriptAst = await scriptParser.parse(text, scriptOptions);
      const protectedRanges = await ignoredRanges(scriptAst, scriptParser, text, scriptOptions);
      ranges.push(
        ...protectedRanges.map((range) => ({
          start: script.start + range.start,
          end: script.start + range.end,
        })),
      );
    }
  }

  return ranges;
}
