/**
 * Returns whether Tailwind generates invalid CSS for `candidate`, which Stillwind must
 * reject. `justify-content` has no `baseline` value, and the color stops of a conic
 * gradient accept angles and percentages but not the lengths of the spacing scale.
 */
export function generatesInvalidCss(candidate: string): boolean {
  return (
    candidate === "justify-baseline" ||
    /^mask-conic-(?:from|to)-(?:\d+(?:\.\d+)?|px)$/.test(candidate)
  );
}
