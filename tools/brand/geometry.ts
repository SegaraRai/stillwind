/**
 * Geometry of the Stillwind mark and the logo lockup.
 *
 * All lengths use the mark grid: the mark's square is 32 units wide, and its rows span y = 2–30. At 16px one unit
 * is 0.5px, so the row height and the gaps stay on whole pixels in a favicon.
 */

import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { inflateSync } from "node:zlib";

import * as hb from "harfbuzzjs";
import opentype from "opentype.js";

/** Row height. */
export const ROW_HEIGHT = 8;
/** Gap between rows and between the tiles of one row. */
export const GAP = 2;
/** Horizontal slant per unit of height. */
export const SLOPE = 1 / 4;
/** Corner radius of each tile. */
export const RADIUS = 0.75;

/** Colors of the mark parts. `o` paints the ordinary tiles, and `e` paints the flagged middle row. */
export const COLORS = {
  light: { o: "#354fa2", e: "#d9725b", text: "#20222c" },
  dark: { o: "#b6cdff", e: "#f1a297", text: "#e9f2fd" },
} as const;

/** Rounds a coordinate for SVG output. */
export const fmt = (n: number): number => +n.toFixed(3);

/**
 * Returns the path of one parallelogram tile whose top-left corner is (x, y).
 *
 * The polygon is inset by `radius` so that a round-joined stroke of width `2 * radius` puts the outer edge on the
 * construction lines. Callers must stroke the path with that width.
 */
export function tilePath(
  x: number,
  y: number,
  width: number,
  height = ROW_HEIGHT,
  radius = RADIUS,
): string {
  const k = radius * Math.hypot(1, SLOPE);
  const top = y + radius;
  const bottom = y + height - radius;
  const left = (yy: number) => x - (yy - y) * SLOPE;
  const points = [
    [left(top) + k, top],
    [left(top) + width - k, top],
    [left(bottom) + width - k, bottom],
    [left(bottom) + k, bottom],
  ];
  return `M${points.map((p) => p.map(fmt).join(" ")).join("L")}Z`;
}

/** A tile of the mark. `part` selects the color from {@link COLORS}. */
export interface MarkTile {
  part: "o" | "e";
  d: string;
}

/**
 * Returns the five tiles of the mark in the 32-unit square.
 *
 * Every row has the same length. The middle row is one flagged tile. The top and bottom rows hold a short tile A
 * and a long tile 3A in point-symmetric order. The top row moves 2 units right and the bottom row 2 units left of
 * the aligned layout, so the mark reads as an S and spans x = 0–32.
 */
export function markTiles(): MarkTile[] {
  const slant = ROW_HEIGHT * SLOPE;
  const step = (ROW_HEIGHT + GAP) * SLOPE;
  const length = 32 - 4 - slant - 2 * step;
  const short = (length - GAP) / 4;
  const rows: { y: number; shift: number; tiles: [MarkTile["part"], number][] }[] = [
    {
      y: 2,
      shift: 2 * step + 2,
      tiles: [
        ["o", 3 * short],
        ["o", short],
      ],
    },
    { y: 2 + ROW_HEIGHT + GAP, shift: step, tiles: [["e", length]] },
    {
      y: 2 + 2 * (ROW_HEIGHT + GAP),
      shift: -2,
      tiles: [
        ["o", short],
        ["o", 3 * short],
      ],
    },
  ];
  const lowest = Math.min(...rows.map((row) => row.shift));
  return rows.flatMap((row) => {
    let x = slant + row.shift - lowest;
    return row.tiles.map(([part, width]) => {
      const tile = { part, d: tilePath(x, row.y, width) };
      x += width + GAP;
      return tile;
    });
  });
}

const require = createRequire(import.meta.url);

/**
 * Converts a WOFF 1 font to SFNT data. HarfBuzz reads only SFNT data, and Fontsource ships WOFF files.
 */
function sfntFromWoff(woff: Buffer): Uint8Array {
  const count = woff.readUInt16BE(12);
  const tables = Array.from({ length: count }, (_, i) => {
    const at = 44 + i * 20;
    const offset = woff.readUInt32BE(at + 4);
    const compressed = woff.readUInt32BE(at + 8);
    const length = woff.readUInt32BE(at + 12);
    const raw = woff.subarray(offset, offset + compressed);
    return {
      tag: woff.subarray(at, at + 4),
      checksum: woff.readUInt32BE(at + 16),
      data: compressed < length ? inflateSync(raw) : raw,
    };
  });
  const directory = 12 + 16 * count;
  const size = tables.reduce((sum, table) => sum + Math.ceil(table.data.length / 4) * 4, directory);
  const out = Buffer.alloc(size);
  const power = 2 ** Math.floor(Math.log2(count));
  out.writeUInt32BE(woff.readUInt32BE(4), 0);
  out.writeUInt16BE(count, 4);
  out.writeUInt16BE(power * 16, 6);
  out.writeUInt16BE(Math.log2(power), 8);
  out.writeUInt16BE(count * 16 - power * 16, 10);
  let offset = directory;
  tables.forEach((table, i) => {
    const at = 12 + i * 16;
    table.tag.copy(out, at);
    out.writeUInt32BE(table.checksum, at + 4);
    out.writeUInt32BE(offset, at + 8);
    out.writeUInt32BE(table.data.length, at + 12);
    table.data.copy(out, offset);
    offset += Math.ceil(table.data.length / 4) * 4;
  });
  return out;
}

/**
 * A font loaded for both libraries. HarfBuzz positions the glyphs with the GPOS kerning, which opentype.js 1
 * ignores, and opentype.js provides the glyph outlines.
 */
export interface LoadedFont {
  outlines: opentype.Font;
  shaping: hb.Font;
  unitsPerEm: number;
  capHeight: number;
}

/** Loads a WOFF file from an installed Fontsource package, such as `@fontsource/rubik/files/rubik-latin-400-normal.woff`. */
export function loadFont(specifier: string): LoadedFont {
  const buffer = readFileSync(require.resolve(specifier));
  const outlines = opentype.parse(
    buffer.buffer.slice(buffer.byteOffset, buffer.byteOffset + buffer.byteLength),
  );
  const shaping = new hb.Font(new hb.Face(new hb.Blob(sfntFromWoff(buffer))));
  return {
    outlines,
    shaping,
    unitsPerEm: outlines.unitsPerEm,
    capHeight: outlines.tables.os2.sCapHeight,
  };
}

/** Text converted to outlines. Coordinates use the caller's units. */
export interface OutlinedText {
  /** One path per glyph with ink, in text order. */
  paths: string[];
  /** Ink bounds of all glyphs. */
  box: { x1: number; y1: number; x2: number; y2: number };
  /** Font size. */
  size: number;
  /** Pen position of the first glyph. */
  x: number;
}

/**
 * Converts one line of text to glyph outlines. HarfBuzz applies the font's kerning and other default features, as
 * a browser does.
 *
 * @param inkLeft - x of the first glyph's left ink edge.
 * @param baseline - y of the baseline.
 * @param size - Font size.
 * @param tracking - Letter spacing in em, added after every glyph like CSS `letter-spacing`.
 */
export function outlineText(
  font: LoadedFont,
  text: string,
  inkLeft: number,
  baseline: number,
  size: number,
  tracking = 0,
): OutlinedText {
  const scale = size / font.unitsPerEm;
  const buffer = new hb.Buffer();
  buffer.addText(text);
  buffer.guessSegmentProperties();
  hb.shape(font.shaping, buffer);
  const shaped = buffer.getGlyphInfosAndPositions();
  const glyph = (index: number) => font.outlines.glyphs.get(index);
  const start = inkLeft - (shaped[0] ? glyph(shaped[0].codepoint).getBoundingBox().x1 : 0) * scale;
  const box = { x1: Infinity, y1: Infinity, x2: -Infinity, y2: -Infinity };
  const paths: string[] = [];
  let x = start;
  for (const item of shaped) {
    const path = glyph(item.codepoint).getPath(
      x + (item.xOffset ?? 0) * scale,
      baseline - (item.yOffset ?? 0) * scale,
      size,
    );
    const bounds = path.getBoundingBox();
    if (Number.isFinite(bounds.x1)) {
      box.x1 = Math.min(box.x1, bounds.x1);
      box.y1 = Math.min(box.y1, bounds.y1);
      box.x2 = Math.max(box.x2, bounds.x2);
      box.y2 = Math.max(box.y2, bounds.y2);
      paths.push(path.toPathData(3));
    }
    x += (item.xAdvance ?? 0) * scale + tracking * size;
  }
  return { paths, box, size, x: start };
}

/** Font size whose cap height equals `cap`. */
export const sizeForCap = (font: LoadedFont, cap: number): number =>
  (cap * font.unitsPerEm) / font.capHeight;

/** Settings of the logo lockup in mark units. */
export const LOCKUP = {
  /** Cap height of the wordmark. */
  cap: 21,
  /** Distance from the mark's right edge to the left ink edge of the S. */
  gap: 9,
  /** The wordmark's cap band is centered on the mark and then moved down by this distance. */
  drop: 1,
  /** Letter spacing in em. */
  tracking: -0.03,
  /** Space around the visible outlines before the width is rounded up. */
  padding: 2,
} as const;

/** The logo lockup: mark tiles, wordmark outlines, and a viewBox whose width and height are whole units. */
export interface Lockup {
  tiles: MarkTile[];
  wordmark: OutlinedText;
  baseline: number;
  /** `[x, y, width, height]`. Width and height are integers, and the extra space is split between both sides. */
  viewBox: [number, number, number, number];
}

/** Lays out the logo lockup with the Rubik 400 wordmark. */
export function lockup(): Lockup {
  const font = loadFont("@fontsource/rubik/files/rubik-latin-400-normal.woff");
  const size = sizeForCap(font, LOCKUP.cap);
  const baseline = 16 + LOCKUP.cap / 2 + LOCKUP.drop;
  const wordmark = outlineText(font, "Stillwind", 32 + LOCKUP.gap, baseline, size, LOCKUP.tracking);
  const top = Math.min(2, wordmark.box.y1);
  const bottom = Math.max(30, wordmark.box.y2);
  const pad = LOCKUP.padding;
  const width = Math.ceil(wordmark.box.x2 + 2 * pad);
  const height = Math.ceil(bottom - top + 2 * pad);
  const viewBox: Lockup["viewBox"] = [
    fmt(-pad - (width - wordmark.box.x2 - 2 * pad) / 2),
    fmt(top - pad - (height - (bottom - top) - 2 * pad) / 2),
    width,
    height,
  ];
  return { tiles: markTiles(), wordmark, baseline, viewBox };
}
