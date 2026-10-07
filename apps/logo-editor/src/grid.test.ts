import { describe, expect, test } from "vite-plus/test";

import { area, contains, exportSvg, faces, preset, snap, type Drawing } from "./geometry.ts";
import { decodeGrid, defaultGrid, gridSpacing, validDivisions } from "./grid.ts";

const empty: Drawing = { version: 1, points: [], edges: [], paints: {}, nextId: 1 };

describe("square starting geometry", () => {
  test("both presets cover the entire square, including separately selectable corners", () => {
    for (const blank of [true, false]) {
      const d = preset(blank);
      const fs = faces(d);
      expect(fs.reduce((sum, f) => sum + area(f.polygon), 0)).toBeCloseTo(360 * 360);
      expect(d.points.every((p) => p.x >= 60 && p.x <= 420 && p.y >= 60 && p.y <= 420)).toBe(true);
      for (const p of [
        { x: 419, y: 61 },
        { x: 61, y: 419 },
        { x: 61, y: 61 },
        { x: 419, y: 419 },
      ]) {
        expect(fs.some((f) => contains(f.polygon, p) && d.paints[f.key].visible)).toBe(true);
      }
      if (!blank) {
        const upper = fs.find((f) => contains(f.polygon, { x: 419, y: 61 }))!;
        const lower = fs.find((f) => contains(f.polygon, { x: 61, y: 419 }))!;
        expect(upper.key).not.toBe(lower.key);
        d.paints[upper.key].visible = false;
        expect(exportSvg(d).match(/<polygon/g)).toHaveLength(fs.length - 1);
        expect(d.paints[lower.key].visible).toBe(true);
      }
    }
  });
});

describe("division grid", () => {
  test("24 subdivisions span the centered 360-unit region", () => {
    expect(gridSpacing(defaultGrid())).toEqual({ x: 15, y: 15, origin: 60, extent: 360 });
    const d = preset();
    for (const p of d.points) {
      expect((p.x - 60) / 15).toBeCloseTo(Math.round((p.x - 60) / 15));
      expect((p.y - 60) / 15).toBeCloseTo(Math.round((p.y - 60) / 15));
    }
  });
  test("origin is the design region rather than the canvas, including non-divisor counts", () => {
    const settings = { ...defaultGrid(), shared: 7 };
    expect(snap(empty, { x: 110, y: 112 }, null, 4, gridSpacing(settings)).point.x).toBeCloseTo(
      60 + 360 / 7,
    );
    expect(snap(empty, { x: 419, y: 61 }, null, 4, gridSpacing(settings)).point).toEqual({
      x: 420,
      y: 60,
    });
  });
  test("independent axes and no grid retain drawing coordinates", () => {
    const d = preset();
    const before = JSON.stringify(d);
    const settings = { mode: "independent" as const, shared: 24, x: 12, y: 0 };
    expect(gridSpacing(settings)).toEqual({ x: 30, y: 0, origin: 60, extent: 360 });
    expect(snap(empty, { x: 91, y: 83 }, null, 4, gridSpacing(settings)).point).toEqual({
      x: 90,
      y: 83,
    });
    expect(gridSpacing({ ...settings, mode: "none" })).toEqual({
      x: 0,
      y: 0,
      origin: 60,
      extent: 360,
    });
    expect(JSON.stringify(d)).toBe(before);
    expect(decodeGrid(JSON.stringify(settings))).toEqual(settings);
  });
  test("division counts are integers, zero only disables independent axes", () => {
    expect(validDivisions(24, false)).toBe(true);
    expect(validDivisions(0, true)).toBe(true);
    for (const n of [0, 12.5, -1, 361, Infinity, NaN]) {
      expect(validDivisions(n, false)).toBe(false);
    }
  });
});

describe("snap priority", () => {
  const d: Drawing = {
    ...empty,
    points: [
      { id: 1, x: 60, y: 60, width: 4 },
      { id: 2, x: 90, y: 60, width: 4 },
      { id: 3, x: 61, y: 91, width: 4 },
    ],
    edges: [
      { a: 1, b: 2 },
      { a: 1, b: 3 },
    ],
  };
  test("both grid axes win even when the parallelogram completion is closer", () => {
    const result = snap(d, { x: 91, y: 91 }, null, 4, gridSpacing(defaultGrid()));
    expect(result.point).toEqual({ x: 90, y: 90 });
    expect(result.label).toBe("グリッドに整列");
    expect(result.guides).toHaveLength(2);
  });
  test("one-axis and disabled grids leave the parallelogram priority intact", () => {
    for (const grid of [
      { x: 15, y: 0, origin: 60, extent: 360 },
      { x: 0, y: 0 },
      { x: 15, y: 20, origin: 60, extent: 360 },
    ]) {
      const result = snap(d, { x: 91, y: 91 }, null, 4, grid);
      expect(result.point).toEqual({ x: 91, y: 91 });
      expect(result.label).toBe("平行四辺形の第4頂点");
    }
  });
});
