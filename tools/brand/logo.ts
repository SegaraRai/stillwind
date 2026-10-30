/**
 * SVG files of the logo: the layered icons used by the sites, the favicon, and the files for distribution.
 */

import { COLORS, fmt, type Lockup, RADIUS } from "./geometry.ts";

const STROKE = `stroke-width="${2 * RADIUS}" stroke-linejoin="round"`;

const lockupRoot = (logo: Lockup) =>
  `<svg xmlns="http://www.w3.org/2000/svg" width="${logo.viewBox[2] * 4}" height="${logo.viewBox[3] * 4}" viewBox="${logo.viewBox.join(" ")}">`;

const paths = (list: string[], attributes = "") =>
  list.map((d) => `    <path${attributes} d="${d}" />`).join("\n");

/**
 * The `stillwind-logo` utility draws the lockup in three icon layers with the same viewBox.
 *
 * The element itself uses `t` as its mask. Its background is transparent on the left and the text color from
 * {@link LAYER_SPLIT} of the width, so the text gets the text color. The rectangle in `t` lets the `e` and `o`
 * pseudo-elements show through the mask over the mark.
 */
export const LAYER_SPLIT = 0.25;
/** Right edge of the rectangle in `t`. It stays left of the split so that its antialiased edge is not visible. */
const RECT_END = 34;

/** Returns the `t`, `e`, and `o` icon layers of the lockup. */
export function logoLayers(logo: Lockup): Record<"t" | "e" | "o", string> {
  const [x, y, width, height] = logo.viewBox;
  const split = x + LAYER_SPLIT * width;
  if (RECT_END <= 32 || split <= RECT_END + 1 || split >= logo.wordmark.box.x1) {
    throw new Error(
      `The layer split at x = ${split} does not fall between the mark and the wordmark.`,
    );
  }
  const part = (name: "e" | "o") =>
    `${lockupRoot(logo)}
  <g fill="currentColor" stroke="currentColor" ${STROKE}>
${paths(logo.tiles.filter((tile) => tile.part === name).map((tile) => tile.d))}
  </g>
</svg>
`;
  return {
    t: `${lockupRoot(logo)}
  <rect x="${x}" y="${y}" width="${fmt(RECT_END - x)}" height="${height}" fill="currentColor" />
  <g fill="currentColor" aria-label="Stillwind">
${paths(logo.wordmark.paths)}
  </g>
</svg>
`,
    e: part("e"),
    o: part("o"),
  };
}

/** Returns the mark with colors for both color schemes, for favicons and file icons. */
export function markIcon(logo: Lockup): string {
  const rule = (scheme: keyof typeof COLORS, part: "o" | "e") =>
    `.${part} { fill: ${COLORS[scheme][part]}; stroke: ${COLORS[scheme][part]}; }`;
  return `<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 32 32">
  <style>
    ${rule("light", "o")}
    ${rule("light", "e")}
    @media (prefers-color-scheme: dark) {
      ${rule("dark", "o")}
      ${rule("dark", "e")}
    }
  </style>
  <g ${STROKE}>
${logo.tiles.map((tile) => `    <path class="${tile.part}" d="${tile.d}" />`).join("\n")}
  </g>
</svg>
`;
}

const coloredTiles = (logo: Lockup) => {
  const c = COLORS.light;
  return `  <g ${STROKE}>
${logo.tiles.map((tile) => `    <path d="${tile.d}" fill="${c[tile.part]}" stroke="${c[tile.part]}" />`).join("\n")}
  </g>`;
};

/** Returns the mark alone with the light colors. */
export const markOnly = (logo: Lockup): string =>
  `<svg xmlns="http://www.w3.org/2000/svg" width="128" height="128" viewBox="0 0 32 32">
${coloredTiles(logo)}
</svg>
`;

/** Returns the lockup with the light colors and an outlined wordmark. */
export const lockupOutlined = (logo: Lockup): string =>
  `${lockupRoot(logo)}
  <title>Stillwind</title>
${coloredTiles(logo)}
  <g fill="${COLORS.light.text}" aria-label="Stillwind">
${paths(logo.wordmark.paths)}
  </g>
</svg>
`;

/** Returns the lockup with the light colors and an editable Rubik 400 `<text>` wordmark. */
export const lockupEditable = (logo: Lockup, tracking: number): string =>
  `${lockupRoot(logo)}
  <title>Stillwind</title>
${coloredTiles(logo)}
  <text x="${fmt(logo.wordmark.x)}" y="${fmt(logo.baseline)}" fill="${COLORS.light.text}" font-family="Rubik" font-weight="400" font-size="${fmt(logo.wordmark.size)}" letter-spacing="${fmt(tracking * logo.wordmark.size)}">Stillwind</text>
</svg>
`;
