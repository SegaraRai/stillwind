import type { NativeEditorQuery } from "@stillwind/native";

/**
 * Query methods of one design system that the lookup uses.
 */
export type UtilityQuery = Pick<
  NativeEditorQuery,
  | "previewCandidate"
  | "canonicalCandidate"
  | "canonicalCandidates"
  | "completeCandidate"
  | "formatCss"
>;

/**
 * One utility that matches the input.
 */
export interface UtilityMatch {
  /** Canonical spelling of the utility, or several utilities separated by spaces. */
  utility: string;
  /** Formatted CSS of `utility`. Empty for a marker class, which adds no declarations. */
  css: string;
  /**
   * How the input reached `utility`: `utility` for a utility in the input, `css` for CSS
   * declarations in the input, and `prefix` for a utility that starts with the input.
   */
  source: "utility" | "css" | "prefix";
  /** Text of the input that produced `utility`, when it differs from `utility`. */
  from?: string;
  /**
   * Relation of a utility in the input to its canonical spelling. `same` marks a canonical
   * spelling whose declarations equal those of `from`. `written` marks the spelling as written,
   * which follows a canonical spelling with different declarations.
   */
  relation?: "same" | "written";
}

/**
 * Returns a short description of where `match` came from, or undefined when it needs none.
 */
export function matchCaption(match: UtilityMatch): string | undefined {
  if (match.relation === "written") {
    return "As written";
  }
  if (match.from === undefined) {
    return undefined;
  }
  if (match.source === "css") {
    return `From ${match.from}`;
  }
  return match.relation === "same"
    ? `Same as ${match.from}`
    : `Canonical spelling of ${match.from}`;
}

/**
 * Matches for one input.
 */
export interface UtilityLookup {
  matches: UtilityMatch[];
  /** Number of utilities that start with the input, including those beyond the limit. */
  prefixTotal: number;
}

/**
 * Finds utilities for `input` in both directions. Utilities in the input resolve to their CSS
 * and canonical spelling. CSS declarations, with or without a surrounding rule, resolve to the
 * canonical utility of each declaration. A single word also lists the utilities that start with it.
 *
 * @param limit - Maximum number of matches. Matches from the input come before prefix matches.
 */
export function lookupUtilities(query: UtilityQuery, input: string, limit = 10): UtilityLookup {
  const text = input.trim();
  if (!text) {
    return {
      matches: [],
      prefixTotal: 0,
    };
  }

  const css = (utility: string) => {
    const preview = query.previewCandidate(utility);
    return preview === null
      ? undefined
      : query.formatCss(preview, {
          themeValues: true,
          pixelEquivalents: true,
          colorEquivalents: false,
        });
  };
  const matches: UtilityMatch[] = [];
  const add = (
    utilities: string[],
    source: UtilityMatch["source"],
    from: string,
    relation?: UtilityMatch["relation"],
  ) => {
    const previews = utilities.map(css);
    if (previews.every((preview) => preview !== undefined)) {
      const utility = utilities.join(" ");
      matches.push({
        utility,
        css: previews.join("\n"),
        source,
        ...(utility === from ? {} : { from }),
        ...(relation ? { relation } : {}),
      });
    }
  };

  const declarations = parseDeclarations(text);
  const words = text.split(/\s+/);
  if (declarations) {
    const utilities = declarations.map(([property, value]) => {
      const arbitrary = `[${property}:${value.replaceAll("_", "\\_").replace(/\s+/g, "_")}]`;
      return query.canonicalCandidate(arbitrary) ?? arbitrary;
    });
    // An input such as `hover:underline` also reads as a declaration. The utility reading wins
    // unless a declaration maps to a named utility.
    const named = utilities.some((utility) => !utility.startsWith("["));
    if (named || !words.every((word) => query.previewCandidate(word) !== null)) {
      if (declarations.length > 1) {
        add(query.canonicalCandidates(utilities), "css", text);
      }
      declarations.forEach(([property, value], index) =>
        add([utilities[index]!], "css", `${property}: ${value}`),
      );
    }
  }
  if (words.every((word) => query.previewCandidate(word) !== null)) {
    const canonical = query.canonicalCandidates(words);
    if (words.length > 1 && canonical.join(" ") !== text) {
      add(canonical, "utility", text);
    }
    for (const word of words) {
      const canonical = query.canonicalCandidate(word) ?? word;
      if (canonical === word) {
        add([word], "utility", word);
        continue;
      }
      // Previews compare the generated declarations before formatting adds annotations.
      const same = query.previewCandidate(canonical) === query.previewCandidate(word);
      add([canonical], "utility", word, same ? "same" : undefined);
      if (!same) {
        add([word], "utility", word, "written");
      }
    }
  }

  const seen = new Set([...words, ...matches.map((match) => match.utility)]);
  const prefixes =
    words.length === 1
      ? query
          .completeCandidate(text)
          .map(({ label }) => label)
          .filter((label) => !label.endsWith(":") && !seen.has(label))
      : [];
  for (const label of prefixes) {
    if (matches.length >= limit) {
      break;
    }
    matches.push({
      utility: label,
      css: css(label) ?? "",
      source: "prefix",
    });
  }
  return {
    matches: unique(matches).slice(0, limit),
    prefixTotal: prefixes.length,
  };
}

/**
 * Removes later matches that repeat the utility and source of an earlier match.
 */
function unique(matches: UtilityMatch[]): UtilityMatch[] {
  const seen = new Set<string>();
  return matches.filter((match) => {
    const key = `${match.source}\n${match.utility}\n${match.from ?? ""}\n${match.relation ?? ""}`;
    return !seen.has(key) && seen.add(key);
  });
}

/**
 * Reads `text` as CSS declarations separated by semicolons or line breaks. A surrounding rule,
 * such as `& { ... }`, is ignored.
 *
 * @returns Property and value pairs, or undefined when any part is not a declaration.
 */
export function parseDeclarations(text: string): [property: string, value: string][] | undefined {
  const body = text.replace(/^[^{]*\{([\s\S]*)\}\s*$/, "$1");
  const parts = body
    .split(/;|\n/)
    .map((part) => part.trim())
    .filter(Boolean);
  const declarations = parts.map((part) =>
    /^(--[\w-]+|-?[a-z][a-z0-9-]*)\s*:\s*(\S[\s\S]*)$/i.exec(part),
  );
  return parts.length > 0 && declarations.every(Boolean)
    ? declarations.map((match) => {
        const [, property, value] = match as unknown as [string, string, string];
        return [property.startsWith("--") ? property : property.toLowerCase(), value.trim()];
      })
    : undefined;
}
