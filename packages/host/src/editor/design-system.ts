import type { NativeEditorQuery } from "@stillwind/native";

import type { PluginRuntime } from "../css/plugins/runtime.ts";
import { CandidateQueries, segments } from "./candidates.ts";

/**
 * A question about one project's design system, independent of any source file.
 */
export type DesignSystemRequest =
  | {
      /** Requests theme tokens. */
      kind: "theme";
      /** Token names without or with the leading `--`, where `*` matches any text. */
      targets: readonly string[];
      /** Keeps tokens with this origin. Omission keeps every token. */
      source?: DesignTokenSource;
    }
  | {
      /** Requests utilities, or variants, by exact candidate or pattern. */
      kind: "utilities" | "variants";
      /** Decoded candidates. A `*` outside brackets and parentheses makes a pattern. */
      targets: readonly string[];
    };

/**
 * Origin of a token: the project's `@theme`, or a default that `@theme` can override.
 */
export type DesignTokenSource = "project" | "default";

/**
 * An active theme token.
 */
export interface DesignToken {
  /** Complete custom property name, including the leading `--`. */
  name: string;
  /** CSS value as written, including references to other tokens. */
  value: string;
  /** Origin of the active definition. */
  source: DesignTokenSource;
  /** Whether utilities write the value of the token instead of a reference to it. */
  inline: boolean;
}

/**
 * One utility, variant, or marker class, with its CSS.
 */
export interface DesignCandidate {
  /** Decoded candidate. Variant names have no trailing colon. */
  name: string;
  /** Kind of input that the name selects. */
  kind: "utility" | "variant" | "marker";
  /** Whether the name resolves in this design system. */
  valid: boolean;
  /** Formatted CSS. It is empty for an invalid name and for a marker without CSS. */
  css: string;
  /** Closest valid spelling of an invalid name. Absent when no spelling is close enough. */
  suggestion?: string;
  /**
   * Canonical spelling of a valid exact candidate. Absent for a canonical name, for a name without
   * a replacement that produces the same CSS, and for a pattern match.
   */
  canonical?: string;
}

/**
 * The answer to one design-system request.
 */
export interface DesignSystemResult {
  /** Matching tokens in name order. */
  tokens: DesignToken[];
  /** Exact candidates in target order, then pattern matches in name order. */
  candidates: DesignCandidate[];
  /** Targets without a match. Exact candidates stay in candidates as invalid entries. */
  unmatched: string[];
}

/**
 * Everything a request reads from one loaded design system.
 */
export interface DesignSystemContext {
  /** Query over the loaded design system. */
  query: NativeEditorQuery;
  /** Plugins that add dynamic candidates. */
  plugins?: PluginRuntime;
  /** Preview comment settings. */
  annotations: Parameters<NativeEditorQuery["formatCss"]>[1];
}

/**
 * Tests whether a target contains a wildcard outside brackets and parentheses.
 *
 * @param target - Decoded target text.
 * @returns Whether the target is a pattern.
 */
function isPattern(target: string): boolean {
  return segments(target, "*").length > 1;
}

/**
 * Compiles a target whose `*` matches any text, including `/` and `:`.
 *
 * @param target - Decoded pattern.
 * @returns An anchored expression.
 */
function patternExpression(target: string): RegExp {
  const parts = segments(target, "*").map((part) => part.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));
  return new RegExp(`^${parts.join(".*")}$`, "s");
}

/**
 * Answers a theme request.
 *
 * @param context - Loaded design system.
 * @param request - Theme request.
 * @returns Tokens that match, or every token for an empty target list, and unmatched targets.
 */
function theme(
  context: DesignSystemContext,
  request: Extract<DesignSystemRequest, { kind: "theme" }>,
): DesignSystemResult {
  const tokens = context.query
    .themeTokens()
    .map(({ name, value, fallback, inline }): DesignToken => ({
      name,
      value,
      source: fallback ? "default" : "project",
      inline,
    }))
    .filter(({ source }) => !request.source || source === request.source);
  const targets = request.targets.map((target) => ({
    target,
    expression: patternExpression(`--${target.replace(/^--/, "")}`),
  }));
  return {
    tokens: targets.length
      ? tokens.filter(({ name }) => targets.some(({ expression }) => expression.test(name)))
      : tokens,
    candidates: [],
    unmatched: targets
      .filter(({ expression }) => !tokens.some(({ name }) => expression.test(name)))
      .map(({ target }) => target),
  };
}

/**
 * Describes one candidate.
 *
 * @param query - Query that contains the definitions of the plugin candidates in the request.
 * @param candidates - Preview cache over that query.
 * @param annotations - Preview comment settings.
 * @param name - Decoded candidate, or a variant name without its trailing colon.
 * @param kind - Selects utility or variant resolution.
 * @returns The candidate, with a suggestion when it is invalid.
 * @throws If a matching utility definition is malformed.
 */
function describe(
  query: NativeEditorQuery,
  candidates: CandidateQueries,
  annotations: DesignSystemContext["annotations"],
  name: string,
  kind: "utilities" | "variants",
): DesignCandidate {
  const css = kind === "variants" ? candidates.variant(name, "utility") : candidates.preview(name);
  const marker = kind === "utilities" ? candidates.marker(name) : null;
  if (css || marker) {
    return {
      name,
      kind: kind === "variants" ? "variant" : marker ? "marker" : "utility",
      valid: true,
      css: css ? query.formatCss(css, annotations) : "",
    };
  }
  // A variant resolves only with a utility, which a suggestion must not keep.
  const suggestion =
    kind === "variants"
      ? query.suggestCandidate(`${name}:flex`)?.replace(/:flex$/, "")
      : query.suggestCandidate(name);
  return {
    name,
    kind: kind === "variants" ? "variant" : "utility",
    valid: false,
    css: "",
    ...(suggestion ? { suggestion } : {}),
  };
}

/**
 * Returns the canonical spelling of a valid candidate under the lint options of the project.
 *
 * @param query - Query that contains the definitions of the plugin candidates in the request.
 * @param name - Decoded candidate, or a variant name without its trailing colon.
 * @param kind - Selects utility or variant spelling.
 * @returns The spelling, or undefined when `name` has no different canonical spelling.
 * @throws If a matching utility definition is malformed.
 */
function canonicalSpelling(
  query: NativeEditorQuery,
  name: string,
  kind: "utilities" | "variants",
): string | undefined {
  // A variant resolves only with a utility, which the spelling must not keep.
  return kind === "variants"
    ? query.canonicalCandidate(`${name}:flex`)?.replace(/:flex$/, "")
    : (query.canonicalCandidate(name) ?? undefined);
}

/**
 * Answers a utility or variant request.
 *
 * @param context - Loaded design system.
 * @param request - Candidate request. An empty target list means every listed name.
 * @returns Exact candidates, then the names that match patterns. Patterns read the completion
 * catalog, so they omit numeric and arbitrary values that exact candidates accept.
 * @throws If a plugin fails or a matching utility definition is malformed.
 */
async function candidates(
  context: DesignSystemContext,
  request: Extract<DesignSystemRequest, { kind: "utilities" | "variants" }>,
): Promise<DesignSystemResult> {
  const { kind } = request;
  const catalog = new CandidateQueries(context.query);
  const targets = request.targets.length ? request.targets : ["*"];
  const exact = targets.filter((target) => !isPattern(target));
  const patterns: {
    target: string;
    names: string[];
  }[] = [];
  for (const target of targets.filter(isPattern)) {
    const expression = patternExpression(target);
    const prefix = segments(target, "*")[0];
    const labels = [
      ...catalog.complete(prefix).map(({ label }) => label),
      ...((await context.plugins?.catalog(prefix)) ?? []),
    ];
    const names = labels.flatMap((label) => {
      const variant = label.endsWith(":");
      const name = variant ? label.slice(0, -1) : label;
      return variant === (kind === "variants") && !name.endsWith("-") && expression.test(name)
        ? [name]
        : [];
    });
    patterns.push({
      target,
      names: [...new Set(names)].sort(),
    });
  }

  // Each plugin load rebuilds the design system, so one load covers every name of the request.
  const dynamic = [...exact, ...patterns.flatMap(({ names }) => names)]
    .map((name) => (kind === "variants" ? `${name}:` : name))
    .filter((text) => context.query.dynamicCandidates(text).length > 0);
  const query =
    context.plugins && dynamic.length
      ? await context.plugins.query(dynamic.join(" "))
      : context.query;
  const queries = query === context.query ? catalog : new CandidateQueries(query);

  const seen = new Set(exact);
  const listed: DesignCandidate[] = [];
  const unmatched: string[] = [];
  for (const { target, names } of patterns) {
    let matched = false;
    for (const name of names) {
      if (seen.has(name)) {
        matched = true;
        continue;
      }
      const candidate = describe(query, queries, context.annotations, name, kind);
      if (candidate.valid) {
        matched = true;
        seen.add(name);
        listed.push(candidate);
      }
    }
    if (!matched) {
      unmatched.push(target);
    }
  }
  const named = exact.map((name) => {
    const candidate = describe(query, queries, context.annotations, name, kind);
    const canonical = candidate.valid ? canonicalSpelling(query, name, kind) : undefined;
    return canonical ? { ...candidate, canonical } : candidate;
  });
  return {
    tokens: [],
    candidates: [...named, ...listed],
    unmatched,
  };
}

/**
 * Answers a question about a loaded design system.
 *
 * @param context - Loaded design system.
 * @param request - Theme, utility, or variant request.
 * @returns The answer. Invalid names and unmatched targets are data.
 * @throws If a plugin fails or a matching utility definition is malformed.
 */
export function queryDesignSystem(
  context: DesignSystemContext,
  request: DesignSystemRequest,
): Promise<DesignSystemResult> {
  return request.kind === "theme"
    ? Promise.resolve(theme(context, request))
    : candidates(context, request);
}
