/**
 * Converts the legacy `theme.container` configuration into the `container` utility that
 * Tailwind's compatibility layer adds next to its built-in `container`.
 */

/**
 * Compares two breakpoint values in ascending order, as Tailwind sorts its `--breakpoint-*`
 * values. Lengths in `px`, `rem`, and `em` compare by size, with `1rem` and `1em` as `16px`.
 * Other values compare as text after every length.
 *
 * @param left - CSS length, such as `40rem`.
 * @param right - CSS length.
 * @returns A negative number when `left` comes first, a positive number when `right` comes
 * first, and zero for equal values.
 */
function compareBreakpoints(left: string, right: string): number {
  const a = pixels(left);
  const b = pixels(right);
  if (a !== null && b !== null) {
    return a - b;
  }
  if (a !== null || b !== null) {
    return a === null ? 1 : -1;
  }
  return left < right ? -1 : left > right ? 1 : 0;
}

/**
 * Converts a length to pixels.
 * @param value - CSS length.
 * @returns The size in pixels, or null for other values.
 */
const pixels = (value: string) => {
  const match = /^(-?\d*\.?\d+)(px|r?em)$/.exec(value.trim());
  return match ? Number(match[1]) * (match[2] === "px" ? 1 : 16) : null;
};

/**
 * Builds the body of the `container` utility for a `theme.container` configuration, as
 * Tailwind's compatibility layer does.
 *
 * `center` adds `margin-inline: auto`. A string `padding`, or the `DEFAULT` of an object, adds
 * `padding-inline`, and the other keys of an object add `padding-inline` from the breakpoint of
 * the same name. `screens` replaces the built-in maximum widths: the smallest breakpoint resets
 * `max-width`, and each screen sets `max-width` to its minimum width. A screen without a
 * minimum width is skipped.
 *
 * @param container - The `theme.container` value.
 * @param breakpoints - Breakpoint values by name, such as `{ sm: "40rem" }`.
 * @returns CSS declarations and rules for an `@utility container` body, or an empty string when
 * the configuration adds nothing.
 */
export function containerUtility(
  container: unknown,
  breakpoints: Readonly<Record<string, string>>,
): string {
  if (!container || typeof container !== "object") {
    return "";
  }
  const { center, padding, screens } = container as Record<string, unknown>;
  const output: string[] = [];
  if (center) {
    output.push("margin-inline:auto;");
  }
  if (typeof padding === "string") {
    output.push(`padding-inline:${padding};`);
  } else if (padding && typeof padding === "object" && "DEFAULT" in padding) {
    output.push(`padding-inline:${String((padding as Record<string, unknown>).DEFAULT)};`);
  }
  const sorted = Object.entries(breakpoints).sort(([, a], [, b]) => compareBreakpoints(a, b));
  let widths: Map<string, string[]> | undefined;
  if (screens && typeof screens === "object") {
    widths = new Map();
    if (sorted.length) {
      output.push(`@media (width >= ${sorted[0][1]}){max-width:none;}`);
    }
    for (const [name, screen] of Object.entries(screens)) {
      const width =
        typeof screen === "string"
          ? screen
          : screen && typeof screen === "object" && "min" in screen
            ? String((screen as Record<string, unknown>).min)
            : undefined;
      if (width !== undefined) {
        widths.set(name, [width, `max-width:${width};`]);
      }
    }
  }
  if (padding && typeof padding === "object") {
    const paddings = Object.entries(padding as Record<string, unknown>)
      .filter(([name]) => name !== "DEFAULT" && name in breakpoints)
      .sort(([a], [b]) => compareBreakpoints(breakpoints[a], breakpoints[b]));
    for (const [name, value] of paddings) {
      const screen = widths?.get(name);
      if (screen) {
        screen.push(`padding-inline:${String(value)};`);
      } else if (!widths) {
        output.push(`@media (width >= ${breakpoints[name]}){padding-inline:${String(value)};}`);
      }
    }
  }
  for (const [width, ...declarations] of widths?.values() ?? []) {
    output.push(`@media (width >= ${width}){${declarations.join("")}}`);
  }
  return output.join("");
}
