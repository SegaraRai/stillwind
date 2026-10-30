/**
 * Social preview images. Each image has the lockup and a message on the left and a field of mark tiles on the right.
 * The whole image is one SVG with outlined text, so rendering does not depend on installed fonts.
 */

import {
  COLORS,
  fmt,
  GAP,
  loadFont,
  outlineText,
  RADIUS,
  ROW_HEIGHT,
  SLOPE,
  tilePath,
  type Lockup,
} from "./geometry.ts";

/** Scale from mark units to image pixels for the tile field. */
const UNIT = 4;
/** Width of the narrowest field tile in pixels: the short tile A of the mark. */
const SHORT = 4.75 * UNIT;
const ROW = ROW_HEIGHT * UNIT;
const ROW_GAP = GAP * UNIT;

/** Content of one social preview image. */
export interface OgImage {
  width: number;
  height: number;
  /** Headline lines. */
  title: string[];
  /** Description lines below the headline. */
  description: string[];
  /** Pattern of the flagged tiles in the field. */
  field: "scattered" | "brackets";
}

const BACKGROUND = "#f4f7fc";
const MUTED = "#596a78";

/**
 * Rows of `</>` drawn with one cell per short tile. `#` marks a flagged cell.
 *
 * The slash moves one column per row. The centers of `<`, `/`, and `>` are 5 columns apart, so the narrowest
 * space between neighboring glyphs is one empty column on both sides of the slash.
 */
const BRACKETS = [
  "..#.....#.#..",
  ".#.....#...#.",
  "#.....#.....#",
  ".#...#.....#.",
  "..#.#.....#..",
];
/** Empty cells before the first column of {@link BRACKETS}. */
const BRACKETS_START = 4;

/** Returns a deterministic pseudo-random generator. */
const random = (seed: number) => () => {
  seed = (seed * 16807) % 2147483647;
  return seed / 2147483647;
};

const tile = (x: number, y: number, width: number, fill: string, opacity: number) => {
  const r = RADIUS * UNIT;
  const op = opacity < 1 ? ` opacity="${fmt(opacity)}"` : "";
  return `<path d="${tilePath(x, y, width, ROW, r)}" fill="${fill}" stroke="${fill}" stroke-width="${2 * r}"${op} />`;
};

/**
 * Returns the tile field. Row starts lie on one line with the mark's slope, and the tiles fade toward the right edge.
 * `left` is the field's left edge at the bottom of the image.
 */
function field(image: OgImage, left: number): string {
  const rnd = random(image.field === "brackets" ? 23 : 11);
  const { o, e } = COLORS.light;
  const rows = Math.ceil(image.height / (ROW + ROW_GAP)) + 1;
  const offset = (image.height - rows * (ROW + ROW_GAP) + ROW_GAP) / 2;
  const band = image.field === "brackets" ? Math.floor((rows - BRACKETS.length) / 2) : -1;
  const out: string[] = [];
  for (let row = 0; row < rows; row++) {
    const y = offset + row * (ROW + ROW_GAP);
    let x = left + (image.height - y) * SLOPE;
    const fade = (at: number) =>
      Math.max(0.16, 1 - Math.max(0, at - left - 160) / (image.width - left));
    const pattern = row >= band && band >= 0 ? BRACKETS[row - band] : undefined;
    if (pattern) {
      // The band uses one short tile per cell, so the flagged cells line up into the bracket shapes.
      x += SHORT + ROW_GAP;
      for (let cell = 0; x < image.width + 40; cell++, x += SHORT + ROW_GAP) {
        if (pattern[cell - BRACKETS_START] === "#") {
          out.push(tile(x, y, SHORT, e, 1));
        } else {
          out.push(tile(x, y, SHORT, o, 0.18 + rnd() * 0.1));
        }
      }
      continue;
    }
    while (x < image.width + 40) {
      const n = [1, 1, 2, 3, 4][Math.floor(rnd() * 5)]!;
      const width = n * SHORT + (n - 1) * ROW_GAP;
      const flagged = image.field === "scattered" && rnd() < 0.05 && x < image.width - 220;
      out.push(
        flagged ? tile(x, y, width, e, 1) : tile(x, y, width, o, fade(x) * (0.6 + rnd() * 0.4)),
      );
      x += width + ROW_GAP;
    }
  }
  return out.join("\n  ");
}

/** Returns the social preview image as SVG. */
export function ogSvg(image: OgImage, logo: Lockup): string {
  const regular = loadFont(
    "@fontsource/instrument-sans/files/instrument-sans-latin-400-normal.woff",
  );
  const medium = loadFont(
    "@fontsource/instrument-sans/files/instrument-sans-latin-500-normal.woff",
  );
  const margin = 54;
  const scale = 2.75; // mark units to pixels for the lockup: the mark is 77px tall
  const titleSize = 34,
    titleLine = 44,
    descriptionSize = 21,
    descriptionLine = 30;
  const blockHeight =
    32 * scale +
    24 +
    image.title.length * titleLine +
    14 +
    image.description.length * descriptionLine;
  const top = (image.height - blockHeight) / 2;
  const lockupY = top - 2 * scale;
  let y = top + 28 * scale + 24 + titleSize * 0.95;
  const lines: string[] = [];
  for (const line of image.title) {
    lines.push(
      ...outlineText(medium, line, margin, y, titleSize, -0.01).paths.map(
        (d) => `<path d="${d}" />`,
      ),
    );
    y += titleLine;
  }
  y += 14 - titleLine + descriptionLine;
  const description: string[] = [];
  for (const line of image.description) {
    description.push(
      ...outlineText(regular, line, margin, y, descriptionSize).paths.map(
        (d) => `<path d="${d}" />`,
      ),
    );
    y += descriptionLine;
  }
  const { o, e, text } = COLORS.light;
  const tiles = logo.tiles
    .map(
      (t) =>
        `<path d="${t.d}" fill="${t.part === "e" ? e : o}" stroke="${t.part === "e" ? e : o}" />`,
    )
    .join("");
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${image.width}" height="${image.height}" viewBox="0 0 ${image.width} ${image.height}">
  <rect width="${image.width}" height="${image.height}" fill="${BACKGROUND}" />
  ${field(image, image.width * 0.45)}
  <g transform="translate(${fmt(margin)} ${fmt(lockupY)}) scale(${scale})">
    <g stroke-width="${2 * RADIUS}" stroke-linejoin="round">${tiles}</g>
    <g fill="${text}">${logo.wordmark.paths.map((d) => `<path d="${d}" />`).join("")}</g>
  </g>
  <g fill="${text}">${lines.join("")}</g>
  <g fill="${MUTED}">${description.join("")}</g>
</svg>
`;
}
