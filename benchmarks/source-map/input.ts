import type { CssLocations } from "@stillwind/host";

export function cssInput(rules: number, exact = true) {
  const css = Array.from(
    { length: rules },
    (_, i) => `/* 日本語 🧭 */\r\n.r${i}{background:url(ASSET_${i % 8});color:red}\n`,
  ).join("");
  const sourceMap: CssLocations = {
    sources: [{ sourceMapSource: "untitled:benchmark.css", content: css }],
    entries: [
      {
        source: 0,
        generatedStart: 0,
        generatedEnd: Buffer.byteLength(css),
        originalStart: 0,
        originalEnd: Buffer.byteLength(css),
        exact,
      },
    ],
  };
  const replacements = new Map(
    Array.from({ length: 8 }, (_, i) => [`ASSET_${i}`, `./images/${"x".repeat(i * 3)}.svg`]),
  );
  return { output: { css, sourceMap }, replacements };
}
