import type { NativeEditorQuery } from "@stillwind/native";

/**
 * Query methods of one design system that the lookup uses.
 */
export type CandidateQuery = Pick<
  NativeEditorQuery,
  | "previewCandidate"
  | "canonicalCandidate"
  | "canonicalCandidates"
  | "completeCandidate"
  | "formatCss"
  | "previewVariant"
>;

/**
 * One candidate that matches the input: a utility, or a variant.
 */
export interface CandidateMatch {
  /**
   * Canonical spelling of the utility, or several utilities separated by spaces. A variant keeps
   * its trailing colon.
   */
  candidate: string;
  /** Formatted CSS of `candidate`. Empty for a marker class, which adds no declarations. */
  css: string;
  /**
   * How the input reached `candidate`: `utility` for a utility in the input, `css` for CSS
   * declarations in the input, `variant` for a variant whose name or CSS contains the input, and
   * `prefix` for a utility that starts with the input.
   */
  source: "utility" | "css" | "variant" | "prefix";
  /** Text of the input that produced `candidate`, when it differs from `candidate`. */
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
export function matchCaption(match: CandidateMatch): string | undefined {
  if (match.relation === "written") {
    return "As written";
  }
  if (match.from === undefined) {
    return match.source === "variant" ? "Variant" : undefined;
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
export interface CandidateLookup {
  matches: CandidateMatch[];
  /** Number of utilities that start with the input, including those beyond the limit. */
  prefixTotal: number;
  /** Number of variants that match the input, including those beyond the limit. */
  variantTotal: number;
}

const annotations = {
  themeValues: true,
  pixelEquivalents: true,
  colorEquivalents: false,
} as const;

/**
 * Finds utilities and variants for `input`. Utilities in the input resolve to their CSS and
 * canonical spelling. CSS declarations, with or without a surrounding rule, resolve to the
 * canonical utility of each declaration. Variants match by name or by the CSS that they add, so
 * `:hover` and `@media (hover: hover)` find `hover:`. A variant in the input also resolves to its
 * canonical spelling. A single word also lists the utilities that start with it.
 *
 * @param limit - Maximum number of matches. Matches from the input come before prefix matches.
 * @param variantLimit - Maximum number of variants within `limit`.
 */
export function lookupCandidates(
  query: CandidateQuery,
  input: string,
  limit = 10,
  variantLimit = 5,
): CandidateLookup {
  const text = input.trim();
  if (!text) {
    return {
      matches: [],
      prefixTotal: 0,
      variantTotal: 0,
    };
  }

  const css = (utility: string) => {
    const preview = query.previewCandidate(utility);
    return preview === null ? undefined : formatPreview(query, preview);
  };
  const matches: CandidateMatch[] = [];
  const add = (
    utilities: string[],
    source: CandidateMatch["source"],
    from: string,
    relation?: CandidateMatch["relation"],
  ) => {
    const previews = utilities.map(css);
    if (previews.every((preview) => preview !== undefined)) {
      const candidate = utilities.join(" ");
      matches.push({
        candidate,
        css: previews.join("\n"),
        source,
        ...(candidate === from ? {} : { from }),
        ...(relation ? { relation } : {}),
      });
    }
  };

  const declarations = parseDeclarations(text);
  const words = text.split(/\s+/);
  const utilityInput = words.every((word) => css(word) !== undefined);
  if (declarations) {
    const utilities = declarations.map(([property, value]) => {
      const arbitrary = `[${property}:${value.replaceAll("_", "\\_").replace(/\s+/g, "_")}]`;
      return query.canonicalCandidate(arbitrary) ?? arbitrary;
    });
    // An input such as `hover:underline` also reads as a declaration. The utility reading wins
    // unless a declaration maps to a named utility.
    const named = utilities.some((utility) => !utility.startsWith("["));
    if (named || !utilityInput) {
      if (declarations.length > 1) {
        add(query.canonicalCandidates(utilities), "css", text);
      }
      declarations.forEach(([property, value], index) =>
        add([utilities[index]!], "css", `${property}: ${value}`),
      );
    }
  }
  if (utilityInput) {
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

  // Text that reads as a utility skips this reading, because `[color:red]` would otherwise
  // resolve to the selector `&:is(color:red)`.
  const exactMatches = utilityInput || words.length > 1 ? [] : exactVariants(query, text);
  const exactNames = new Set(exactMatches.map(({ candidate }) => candidate));
  const variants = findVariants(query, text).filter(({ name }) => !exactNames.has(`${name}:`));
  const shownVariants = [
    ...exactMatches,
    ...variants
      .slice(0, Math.max(variantLimit - exactMatches.length, 0))
      .map(({ name, preview }): CandidateMatch => ({
        candidate: `${name}:`,
        css: variantCss(query, preview) ?? "",
        source: "variant",
      })),
  ];
  // A declaration without a named utility, such as `hover: hover`, is less likely than a variant.
  const ordered = utilityInput ? [...matches, ...shownVariants] : [...shownVariants, ...matches];

  const seen = new Set([...words, ...matches.map((match) => match.candidate)]);
  const prefixes =
    words.length === 1
      ? query
          .completeCandidate(text)
          .map(({ label }) => label)
          .filter((label) => !label.endsWith(":") && !seen.has(label))
      : [];
  for (const label of prefixes) {
    if (ordered.length >= limit) {
      break;
    }
    ordered.push({
      candidate: label,
      css: css(label) ?? "",
      source: "prefix",
    });
  }
  return {
    matches: unique(ordered).slice(0, limit),
    prefixTotal: prefixes.length,
    variantTotal: exactMatches.length + variants.length,
  };
}

/**
 * Formats a preview, or returns undefined when it is not valid CSS. The compiler accepts any text
 * before the first colon as an arbitrary property, so `[&:focus]` has the preview `&{&:focus;}`.
 */
function formatPreview(query: CandidateQuery, preview: string): string | undefined {
  try {
    return query.formatCss(preview, annotations);
  } catch {
    return undefined;
  }
}

/**
 * One listed variant with the text that the search compares.
 */
interface VariantEntry {
  name: string;
  /** Unformatted CSS of the variant. */
  preview: string;
  /** Lowercase name without whitespace. */
  nameKey: string;
  /** Lowercase CSS without whitespace, so spacing in the input does not affect a match. */
  previewKey: string;
}

const searchKey = (text: string) => text.toLowerCase().replace(/\s+/g, "");

const variantCatalogs = new WeakMap<CandidateQuery, VariantEntry[]>();

/**
 * Returns the variants that completion lists for `query`. The list contains no functional variant
 * with an arbitrary name, such as `data-active`.
 */
function variantCatalog(query: CandidateQuery): VariantEntry[] {
  let catalog = variantCatalogs.get(query);
  if (!catalog) {
    catalog = query.completeCandidate("", "elementVariants").flatMap(({ label }) => {
      const name = label.slice(0, -1);
      const preview = query.previewVariant(name, "utility");
      return preview === null
        ? []
        : [
            {
              name,
              preview,
              nameKey: searchKey(name),
              previewKey: searchKey(preview),
            },
          ];
    });
    variantCatalogs.set(query, catalog);
  }
  return catalog;
}

/**
 * Finds variants whose name or CSS contains every whitespace-separated term of `text`, ignoring
 * case and whitespace. A term of one character matches names only, because nearly every preview
 * contains it. Exact names come first, then names that start with or contain the terms, then
 * matches in CSS. Shorter CSS comes first within each group, so `hover` precedes `not-hover`.
 *
 * @returns Every match in order.
 */
function findVariants(query: CandidateQuery, text: string): VariantEntry[] {
  const terms = text.replace(/:$/, "").toLowerCase().split(/\s+/);
  const inCss = terms.some((term) => term.length > 1);
  const rank = ({ nameKey }: VariantEntry) =>
    nameKey === terms[0] && terms.length === 1
      ? 0
      : nameKey.startsWith(terms[0]!) && terms.length === 1
        ? 1
        : terms.every((term) => nameKey.includes(term))
          ? 2
          : 3;
  return variantCatalog(query)
    .filter(({ nameKey, previewKey }) =>
      terms.every((term) => nameKey.includes(term) || (inCss && previewKey.includes(term))),
    )
    .map((entry) => ({
      entry,
      rank: rank(entry),
    }))
    .sort(
      (a, b) =>
        a.rank - b.rank ||
        a.entry.preview.length - b.entry.preview.length ||
        a.entry.name.localeCompare(b.entry.name),
    )
    .map(({ entry }) => entry);
}

/**
 * Resolves `input` as one variant chain, with or without a trailing colon. This accepts variants
 * that completion does not list, such as `data-active` and `[&:focus]`.
 *
 * @returns The canonical spelling, followed by the spelling as written when their CSS differs.
 * Without a different canonical spelling, returns the variant alone. Returns an empty array when
 * `input` is not a variant.
 */
function exactVariants(query: CandidateQuery, input: string): CandidateMatch[] {
  const name = input.replace(/:$/, "");
  const from = `${name}:`;
  const preview = query.previewVariant(name, "utility");
  const css = preview === null ? undefined : variantCss(query, preview);
  if (css === undefined) {
    return [];
  }
  // A variant resolves only with a utility, which the canonical spelling must not keep.
  const canonical = query.canonicalCandidate(`${name}:flex`)?.replace(/:flex$/, "");
  const canonicalPreview = canonical ? query.previewVariant(canonical, "utility") : null;
  const canonicalCss = canonicalPreview === null ? undefined : variantCss(query, canonicalPreview);
  if (canonicalCss === undefined) {
    return [
      {
        candidate: from,
        css,
        source: "variant",
      },
    ];
  }
  const match: CandidateMatch = {
    candidate: `${canonical}:`,
    css: canonicalCss,
    source: "variant",
    from,
  };
  return canonicalPreview === preview
    ? [
        {
          ...match,
          relation: "same",
        },
      ]
    : [
        match,
        {
          candidate: from,
          css,
          source: "variant",
          relation: "written",
        },
      ];
}

/**
 * Formats the CSS of a variant and marks the empty rule where the utility declarations belong.
 * The comment is added after formatting, because `formatCss` removes comments from its input.
 * The comment replaces an empty `&` rule inside a selector, which generated CSS does not contain.
 * The rule stays when only at-rules enclose it, because a utility needs it there.
 *
 * @returns The CSS, or undefined when `preview` is not valid CSS.
 */
function variantCss(query: CandidateQuery, preview: string): string | undefined {
  const comment = "/* utility declarations */";
  const lines = formatPreview(query, preview)?.split("\n");
  if (!lines) {
    return undefined;
  }
  const output: string[] = [];
  // Indentation of the open rules that have a selector.
  const selectors: number[] = [];
  for (let index = 0; index < lines.length; index++) {
    const line = lines[index]!;
    const indent = line.search(/\S/);
    while (indent >= 0 && selectors.length > 0 && selectors.at(-1)! >= indent) {
      selectors.pop();
    }
    if (line.trim() !== "& {" || lines[index + 1]?.trim() !== "}") {
      output.push(line);
      if (line.endsWith("{") && !line.trimStart().startsWith("@")) {
        selectors.push(indent);
      }
    } else if (selectors.length > 0) {
      output.push(`${" ".repeat(indent)}${comment}`);
      index++;
    } else {
      output.push(line, `${" ".repeat(indent + 2)}${comment}`);
    }
  }
  return output.join("\n");
}

/**
 * Removes later matches that repeat the candidate and source of an earlier match.
 */
function unique(matches: CandidateMatch[]): CandidateMatch[] {
  const seen = new Set<string>();
  return matches.filter((match) => {
    const key = `${match.source}\n${match.candidate}\n${match.from ?? ""}\n${match.relation ?? ""}`;
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
