import { CandidateQueries, segments } from "./candidates.ts";
import type { EditorColor } from "./request.ts";

/**
 * Finds a spelling with an arbitrary color that preserves the candidate's variants, importance,
 * and modifier.
 * Existing opacity modifiers remain present, so their maximum opacity can prevent a replacement.
 * The emitted color must agree with every requested component within 2/255.
 *
 * @param text - Decoded candidate without enclosing group variants. Its current color must resolve.
 * @param outer - Enclosing decoded variants, including trailing colons, used when probing colors.
 * @param color - Finite sRGB components and effective alpha. Values are clamped to 0..1.
 * @param candidates - Query for the snapshot's design system, used to validate each proposed spelling.
 * @returns Decoded replacement text without `outer`, or undefined when the original color cannot
 * resolve or no supported spelling with an arbitrary color reproduces the requested components.
 * @throws If a matching utility definition has a malformed requirement.
 */
export function colorReplacement(
  text: string,
  outer: string,
  color: Pick<EditorColor, "red" | "green" | "blue" | "alpha">,
  candidates: CandidateQueries,
): string | undefined {
  if (
    ![color.red, color.green, color.blue, color.alpha].every(Number.isFinite) ||
    !candidates.color(outer + text)
  ) {
    return undefined;
  }

  const parts = segments(text, ":");
  const utility = parts.pop()!;
  const prefix = parts.length ? `${parts.join(":")}:` : "";
  const importance = utility.startsWith("!") || utility.endsWith("!") ? "!" : "";
  const [base, modifier] = segments(utility.replace(/^!|!$/g, ""), "/");
  const spellings: ((value: string) => string)[] = [];
  if (base.startsWith("[") && base.endsWith("]")) {
    const property = base.slice(1).split(":", 1)[0];
    spellings.push((value) => `[${property}:${value}]`);
  } else {
    for (let index = base.length - 1; index > 0; index--) {
      if (base[index] === "-") {
        const root = base.slice(0, index);
        spellings.push((value) => `${root}-[${value}]`);
      }
    }
  }

  const rgba = [color.red, color.green, color.blue, color.alpha].map((value) =>
    Math.min(1, Math.max(0, value)),
  );
  const rgb = rgba.slice(0, 3).map((value) => Math.round(value * 255));
  for (const spelling of spellings) {
    /**
     * Rebuilds a color candidate with its variants, importance, and opacity modifier preserved.
     *
     * @param value - CSS color encoded for an arbitrary utility value.
     * @returns A candidate using the current spelling, without the separately supplied outer variants.
     */
    const build = (value: string) =>
      `${prefix}${importance}${spelling(value)}${modifier ? `/${modifier}` : ""}`;
    const probe = candidates.color(outer + build("#ffffff"));
    if (!probe || rgba[3] > probe[3] + 1 / 255) {
      continue;
    }

    const alpha = probe[3] ? rgba[3] / probe[3] : 1;
    const value =
      alpha >= 1
        ? `#${rgb.map((value) => value.toString(16).padStart(2, "0")).join("")}`
        : `rgb(${rgb.join("_")}_/_${Number(alpha.toFixed(4))})`;
    const candidate = build(value);
    const rendered = candidates.color(outer + candidate);
    if (rendered?.every((component, index) => Math.abs(component - rgba[index]) <= 2 / 255)) {
      return candidate;
    }
  }

  return undefined;
}

/**
 * Serializes a picker color as a CSS literal for one local reference replacement.
 *
 * @param color - Finite sRGB components and effective alpha, clamped to 0..1.
 * @returns A hex or rgb() literal, or undefined for nonfinite components.
 */
export function cssColor(
  color: Pick<EditorColor, "red" | "green" | "blue" | "alpha">,
): string | undefined {
  const components = [color.red, color.green, color.blue, color.alpha];
  if (!components.every(Number.isFinite)) {
    return undefined;
  }

  const rgba = components.map((value) => Math.min(1, Math.max(0, value)));
  const rgb = rgba.slice(0, 3).map((value) => Math.round(value * 255));
  return rgba[3] === 1
    ? `#${rgb.map((value) => value.toString(16).padStart(2, "0")).join("")}`
    : `rgb(${rgb.join(" ")} / ${Number(rgba[3].toFixed(4))})`;
}
