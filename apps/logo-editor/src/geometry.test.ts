import { describe, expect, test } from "vite-plus/test";

import {
  addPoint,
  area,
  canMove,
  connect,
  decode,
  edgeKey,
  exportSvg,
  faces,
  inset,
  insetRegions,
  corners,
  kind,
  normalize,
  preset,
  preservePaint,
  removePoint,
  snap,
  transformDrawing,
  type Drawing,
  type Point,
} from "./geometry.ts";

const square = () => {
  const d = preset(true);
  for (const p of d.points) {
    p.x = 40 + ((p.x - 60) * 176) / 360;
    p.y = 40 + ((p.y - 60) * 176) / 360;
  }
  return d;
};
const totalArea = (d: Drawing) => faces(d).reduce((n, f) => n + area(f.polygon), 0);

describe("shared topology", () => {
  test("splitting and removing a diagonal changes faces without duplicating corner points", () => {
    const d = square();
    connect(d, 1, 3);
    expect(faces(d)).toHaveLength(2);
    expect(faces(d).map((f) => kind(f.polygon))).toEqual(["三角形", "三角形"]);
    expect(d.points).toHaveLength(4);
    expect(totalArea(d)).toBeCloseTo(176 * 176);
    d.edges = d.edges.filter((e) => edgeKey(e) !== "1:3");
    expect(faces(d)).toHaveLength(1);
    expect(kind(faces(d)[0].polygon)).toBe("平行四辺形");
  });

  test("crossing diagonals produce a single shared center, preserving total area", () => {
    const d = square();
    connect(d, 1, 3);
    connect(d, 2, 4);
    expect(d.points).toHaveLength(5);
    expect(d.points[4]).toMatchObject({ x: 128, y: 128, width: 4.5 });
    expect(faces(d)).toHaveLength(4);
    expect(faces(d).every((f) => f.ids.includes(5))).toBe(true);
    expect(totalArea(d)).toBeCloseTo(176 * 176);
  });

  test("a point inserted on an edge splits it, and a collinear connection does not duplicate it", () => {
    const d = square();
    const p = addPoint(d, { x: 128, y: 40 }, 4.5);
    normalize(d);
    connect(d, 1, 2);
    expect(d.edges).toHaveLength(5);
    expect(d.edges.some((e) => edgeKey(e) === "1:2")).toBe(false);
    expect(faces(d)).toHaveLength(1);
    expect(faces(d)[0].ids).toContain(p.id);
    expect(kind(faces(d)[0].polygon)).toBe("平行四辺形");
  });

  test("a dangling interior segment does not destroy the containing face", () => {
    const d = square(),
      p = addPoint(d, { x: 100, y: 100 }, 4.5);
    connect(d, 1, p.id);
    expect(faces(d)).toHaveLength(1);
    expect(totalArea(d)).toBeCloseTo(176 * 176);
  });

  test("moving a shared point updates every adjacent face and rejects crossing edges", () => {
    const d = square();
    connect(d, 1, 3);
    connect(d, 2, 4);
    expect(canMove(d, 5, { x: 150, y: 120 })).toBe(true);
    Object.assign(d.points[4], { x: 150, y: 120 });
    expect(faces(d).every((f) => f.polygon.some((p) => p.id === 5 && p.x === 150))).toBe(true);
    expect(canMove(d, 5, { x: 240, y: 128 })).toBe(false);
    expect(canMove(d, 5, d.points[0])).toBe(false);
  });

  test("splitting a painted face transfers its paint to the new faces", () => {
    const d = square(),
      before = faces(d);
    d.paints[before[0].key] = { color: "#123456", visible: false };
    const paint = structuredClone(d.paints);
    connect(d, 1, 3);
    preservePaint(d, before, paint);
    expect(Object.values(d.paints)).toEqual([
      { color: "#123456", visible: false },
      { color: "#123456", visible: false },
    ]);
  });
});

describe("shared width", () => {
  test("two triangular faces have exactly the specified perpendicular diagonal gap", () => {
    const d = square();
    connect(d, 1, 3);
    const offsets = faces(d).map((f) => inset(f.polygon).map((p) => (p.y - p.x) / Math.sqrt(2)));
    const gap = Math.min(...offsets[0].map(Math.abs)) + Math.min(...offsets[1].map(Math.abs));
    expect(gap).toBeCloseTo(4.5, 8);
    expect(faces(d).every((f) => inset(f.polygon).length === 3)).toBe(true);
  });

  test("constant width preserves a parallelogram, and excessive widths collapse safely", () => {
    const d = preset();
    for (const f of faces(d)) {
      expect(kind(inset(f.polygon))).toBe(kind(f.polygon));
    }
    const tiny: Point[] = [
      { id: 1, x: 0, y: 0, width: 20 },
      { id: 2, x: 10, y: 0, width: 20 },
      { id: 3, x: 0, y: 10, width: 20 },
    ];
    expect(inset(tiny)).toEqual([]);
  });

  test("varying endpoint widths are represented by the boundary offset, not arbitrary scaling", () => {
    const p: Point[] = [
      { id: 1, x: 0, y: 0, width: 4 },
      { id: 2, x: 100, y: 0, width: 12 },
      { id: 3, x: 100, y: 100, width: 12 },
      { id: 4, x: 0, y: 100, width: 4 },
    ];
    const result = inset(p);
    const top = result.filter((p) => p.y < 50);
    expect(top).toHaveLength(2);
    for (const p of top) {
      expect(p.y).toBeCloseTo(2 + 0.04 * p.x);
    }
  });
});

describe("editing and persistence", () => {
  test("assists the missing parallelogram corner exactly", () => {
    const d: Drawing = {
      version: 1,
      nextId: 4,
      points: [
        { id: 1, x: 20, y: 20, width: 4 },
        { id: 2, x: 70, y: 35, width: 4 },
        { id: 3, x: 80, y: 95, width: 4 },
      ],
      edges: [
        { a: 1, b: 2 },
        { a: 2, b: 3 },
      ],
      paints: {},
    };
    const result = snap(d, { x: 31, y: 81 }, null, 8, { x: 0, y: 0 });
    expect(result.point).toEqual({ x: 30, y: 80 });
    expect(result.label).toBe("平行四辺形の第4頂点");
  });

  test("round-trips editable topology and exports the same inset geometry", () => {
    const d = preset();
    expect(decode(JSON.stringify(d))).toEqual(d);
    const svg = exportSvg(d);
    expect(svg.match(/<polygon/g)).toHaveLength(faces(d).length);
    expect(svg).toContain('viewBox="0 0 480 480"');
    expect(svg).not.toContain("stroke");
    expect(() => decode('{"version":1,"points":[],"edges":[{"a":1,"b":2}],"paints":{}}')).toThrow();
    const broken = structuredClone(d);
    broken.points[0].width = -1;
    expect(() => decode(JSON.stringify(broken))).toThrow();
  });
});

describe("vertex removal", () => {
  test("merges an inserted boundary vertex back into one edge", () => {
    const d = square();
    const p = addPoint(d, { x: 128, y: 40 }, 4.5);
    normalize(d);
    removePoint(d, p.id);
    expect(d.points).toHaveLength(4);
    expect(d.edges).toHaveLength(4);
    expect(totalArea(d)).toBeCloseTo(176 * 176);
  });
  test("removing an interior junction preserves the outer face", () => {
    const d = square();
    connect(d, 1, 3);
    connect(d, 2, 4);
    removePoint(d, 5);
    expect(faces(d)).toHaveLength(1);
    expect(totalArea(d)).toBeCloseTo(176 * 176);
  });
  test("removing a corner closes the remaining polygon", () => {
    const d = square();
    removePoint(d, 1);
    expect(faces(d)).toHaveLength(1);
    expect(kind(faces(d)[0].polygon)).toBe("三角形");
  });
});

describe("arbitrary polygon fills", () => {
  const polygon = (coords: number[][], width = 4): Point[] =>
    coords.map(([x, y], i) => ({ id: i + 1, x, y, width }));
  const lShape = [
    [0, 0],
    [100, 0],
    [100, 40],
    [40, 40],
    [40, 100],
    [0, 100],
  ];
  test("concave polygons keep their color and inset with the same perpendicular gap", () => {
    const points = polygon(lShape),
      regions = insetRegions(points);
    expect(regions).toHaveLength(1);
    expect(area(regions[0][0])).toBeCloseTo(5616);
    const d: Drawing = {
      version: 1,
      nextId: 7,
      points,
      edges: points.map((p, i) => ({ a: p.id, b: points[(i + 1) % points.length].id })),
      paints: {},
    };
    d.paints[faces(d)[0].key] = { color: "#379988", visible: true };
    expect(exportSvg(d)).toContain('fill="#379988"');
  });
  test("collinear shared points remain editable but do not count as corners or change a uniform inset", () => {
    const points = polygon([
      [0, 0],
      [30, 0],
      [70, 0],
      [100, 0],
      [100, 40],
      [40, 40],
      [40, 70],
      [40, 100],
      [0, 100],
    ]);
    expect(corners(points)).toHaveLength(6);
    expect(kind(points)).toBe("6角形");
    expect(area(insetRegions(points)[0][0])).toBeCloseTo(5616);
    expect(points).toHaveLength(9);
  });
  test("zero width retains all concave corners", () => {
    expect(area(insetRegions(polygon(lShape, 0))[0][0])).toBeCloseTo(6400);
  });
  test("a narrow neck can collapse into separate colored components", () => {
    const points = polygon(
      [
        [0, 0],
        [40, 0],
        [40, 15],
        [60, 15],
        [60, 0],
        [100, 0],
        [100, 40],
        [60, 40],
        [60, 25],
        [40, 25],
        [40, 40],
        [0, 40],
      ],
      12,
    );
    const regions = insetRegions(points);
    expect(regions).toHaveLength(2);
    expect(regions.every((r) => area(r[0]) > 0)).toBe(true);
  });
});

describe("localized boundary widths", () => {
  test("a wider intermediate point does not extend its taper across the rest of a straight side", () => {
    const points: Point[] = [
      { id: 1, x: 0, y: 0, width: 4 },
      { id: 2, x: 100, y: 0, width: 4 },
      { id: 3, x: 100, y: 20, width: 12 },
      { id: 4, x: 100, y: 40, width: 4 },
      { id: 5, x: 100, y: 200, width: 4 },
      { id: 6, x: 0, y: 200, width: 4 },
    ];
    const regions = insetRegions(points);
    expect(regions).toHaveLength(1);
    expect(regions[0][0].some((p) => Math.abs(p.x - 98) < 1e-6 && Math.abs(p.y - 198) < 1e-6)).toBe(
      true,
    );
    expect(regions[0][0].some((p) => Math.abs(p.x - 94) < 1e-6 && Math.abs(p.y - 20) < 1e-6)).toBe(
      true,
    );
  });
});

describe("whole drawing transformations", () => {
  test("rotations and reflections retain widths, topology, colors, and hidden faces", () => {
    for (const operation of ["clockwise", "counterclockwise", "horizontal", "vertical"] as const) {
      const d = preset();
      faces(d).forEach((f, i) => {
        d.paints[f.key] = { color: i % 2 ? "#123456" : "#abcdef", visible: i % 3 !== 0 };
      });
      d.points[0].width = 13;
      const signature = (ids: number[]) => [...ids].sort((a, b) => a - b).join("-");
      const paints = new Map(faces(d).map((f) => [signature(f.ids), { ...d.paints[f.key] }]));
      const before = structuredClone(d);
      transformDrawing(d, operation);
      expect(d.edges).toEqual(before.edges);
      expect(d.points.map((p) => p.width)).toEqual(before.points.map((p) => p.width));
      for (const f of faces(d)) {
        expect(d.paints[f.key]).toEqual(paints.get(signature(f.ids)));
      }
      const original = before.points[1],
        actual = d.points[1];
      const expected =
        operation === "clockwise"
          ? { x: 480 - original.y, y: original.x }
          : operation === "counterclockwise"
            ? { x: original.y, y: 480 - original.x }
            : operation === "horizontal"
              ? { x: 480 - original.x, y: original.y }
              : { x: original.x, y: 480 - original.y };
      expect(actual).toMatchObject(expected);
      transformDrawing(
        d,
        operation === "clockwise"
          ? "counterclockwise"
          : operation === "counterclockwise"
            ? "clockwise"
            : operation,
      );
      expect(d).toEqual(before);
    }
  });
});
