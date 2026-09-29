import polygonClipping from "polygon-clipping";

export interface Vec {
  x: number;
  y: number;
}
export interface Point extends Vec {
  id: number;
  width: number;
}
export interface Edge {
  a: number;
  b: number;
}
export interface Face {
  ids: number[];
  key: string;
  polygon: Point[];
}
export interface Paint {
  color: string;
  visible: boolean;
}
export interface Drawing {
  version: 1;
  nextId: number;
  points: Point[];
  edges: Edge[];
  paints: Record<string, Paint>;
}

const EPS = 1e-7;
export const distance = (a: Vec, b: Vec) => Math.hypot(a.x - b.x, a.y - b.y);
const cross = (a: Vec, b: Vec) => a.x * b.y - a.y * b.x;
const sub = (a: Vec, b: Vec): Vec => ({ x: a.x - b.x, y: a.y - b.y });
export const edgeKey = (e: Edge) => [e.a, e.b].sort((a, b) => a - b).join(":");
export const area = (p: Vec[]) => p.reduce((s, a, i) => s + cross(a, p[(i + 1) % p.length]), 0) / 2;
export const center = (p: Vec[]): Vec => ({
  x: p.reduce((s, a) => s + a.x, 0) / p.length,
  y: p.reduce((s, a) => s + a.y, 0) / p.length,
});

export function contains(p: Vec[], q: Vec): boolean {
  let inside = false;
  for (let i = 0, j = p.length - 1; i < p.length; j = i++) {
    const a = p[i],
      b = p[j];
    if (a.y > q.y !== b.y > q.y && q.x < ((b.x - a.x) * (q.y - a.y)) / (b.y - a.y) + a.x) {
      inside = !inside;
    }
  }
  return inside;
}

export function projection(q: Vec, a: Vec, b: Vec): { point: Vec; t: number } {
  const d = sub(b, a),
    length = d.x * d.x + d.y * d.y;
  const t =
    length < EPS ? 0 : Math.max(0, Math.min(1, ((q.x - a.x) * d.x + (q.y - a.y) * d.y) / length));
  return { point: { x: a.x + d.x * t, y: a.y + d.y * t }, t };
}

function intersection(a: Vec, b: Vec, c: Vec, d: Vec): { p: Vec; t: number; u: number } | null {
  const r = sub(b, a),
    s = sub(d, c),
    det = cross(r, s);
  if (Math.abs(det) < EPS) {
    return null;
  }
  const t = cross(sub(c, a), s) / det,
    u = cross(sub(c, a), r) / det;
  if (t < -EPS || t > 1 + EPS || u < -EPS || u > 1 + EPS) {
    return null;
  }
  return { p: { x: a.x + t * r.x, y: a.y + t * r.y }, t, u };
}

export function addPoint(d: Drawing, p: Vec, width: number): Point {
  const existing = d.points.find((q) => distance(p, q) < EPS);
  if (existing) {
    return existing;
  }
  if (d.points.length >= 300) {
    throw new Error("交点を含めた点数の上限は300です。");
  }
  const point = { ...p, id: d.nextId++, width };
  d.points.push(point);
  return point;
}

/** Splits crossings and points on segments into shared vertices, and deduplicates overlapping segments. */
export function normalize(d: Drawing): void {
  const byId = new Map(d.points.map((p) => [p.id, p]));
  const original = d.edges.filter((e) => e.a !== e.b && byId.has(e.a) && byId.has(e.b));
  for (let i = 0; i < original.length; i++) {
    for (let j = i + 1; j < original.length; j++) {
      const a = byId.get(original[i].a)!,
        b = byId.get(original[i].b)!;
      const c = byId.get(original[j].a)!,
        e = byId.get(original[j].b)!;
      const hit = intersection(a, b, c, e);
      if (hit) {
        addPoint(
          d,
          hit.p,
          (a.width + (b.width - a.width) * hit.t + (c.width + (e.width - c.width) * hit.u)) / 2,
        );
      }
    }
  }
  const segments = new Map<string, Edge>();
  for (const e of original) {
    const a = byId.get(e.a)!,
      b = byId.get(e.b)!;
    const chain = d.points
      .map((p) => ({ p, ...projection(p, a, b) }))
      .filter((p) => distance(p.p, p.point) < EPS)
      .sort((a, b) => a.t - b.t);
    for (let i = 1; i < chain.length; i++) {
      const edge = { a: chain[i - 1].p.id, b: chain[i].p.id };
      if (edge.a !== edge.b) {
        segments.set(edgeKey(edge), edge);
      }
    }
  }
  d.edges = [...segments.values()];
}

export function connect(d: Drawing, a: number, b: number): void {
  if (a === b) {
    return;
  }
  if (!d.edges.some((e) => edgeKey(e) === edgeKey({ a, b }))) {
    d.edges.push({ a, b });
  }
  normalize(d);
}

/** Removes a vertex and reconnects its angularly ordered neighbors to retain bounded faces. */
export function removePoint(d: Drawing, id: number): void {
  const point = d.points.find((p) => p.id === id);
  if (!point) {
    return;
  }
  const neighbors = [
    ...new Set(d.edges.flatMap((e) => (e.a === id ? [e.b] : e.b === id ? [e.a] : []))),
  ]
    .map((id) => d.points.find((p) => p.id === id)!)
    .sort(
      (a, b) => Math.atan2(a.y - point.y, a.x - point.x) - Math.atan2(b.y - point.y, b.x - point.x),
    );
  d.edges = d.edges.filter((e) => e.a !== id && e.b !== id);
  d.points = d.points.filter((p) => p.id !== id);
  const count = neighbors.length === 2 ? 1 : neighbors.length;
  if (neighbors.length >= 2) {
    for (let i = 0; i < count; i++) {
      const a = neighbors[i].id,
        b = neighbors[(i + 1) % neighbors.length].id;
      if (!d.edges.some((e) => edgeKey(e) === edgeKey({ a, b }))) {
        d.edges.push({ a, b });
      }
    }
  }
  normalize(d);
}

export function faces(d: Drawing): Face[] {
  const points = new Map(d.points.map((p) => [p.id, p]));
  const neighbors = new Map<number, number[]>();
  for (const { a, b } of d.edges) {
    neighbors.set(a, [...(neighbors.get(a) ?? []), b]);
    neighbors.set(b, [...(neighbors.get(b) ?? []), a]);
  }
  for (const [id, list] of neighbors) {
    const p = points.get(id)!;
    list.sort(
      (a, b) =>
        Math.atan2(points.get(a)!.y - p.y, points.get(a)!.x - p.x) -
        Math.atan2(points.get(b)!.y - p.y, points.get(b)!.x - p.x),
    );
  }
  const visited = new Set<string>(),
    result: Face[] = [];
  for (const edge of d.edges) {
    for (const start of [
      [edge.a, edge.b],
      [edge.b, edge.a],
    ]) {
      if (visited.has(start.join(":"))) {
        continue;
      }
      let [a, b] = start;
      let ids: number[] = [];
      for (let step = 0; step <= d.edges.length * 2; step++) {
        const key = `${a}:${b}`;
        if (visited.has(key)) {
          break;
        }
        visited.add(key);
        ids.push(a);
        const list = neighbors.get(b)!;
        const next = list[(list.indexOf(a) + list.length - 1) % list.length];
        a = b;
        b = next;
      }
      // A dangling edge traverses out and back without bounding an additional face.
      let changed = true;
      while (changed && ids.length >= 3) {
        changed = false;
        for (let i = 0; i < ids.length; i++) {
          if (ids[i] === ids[(i + 2) % ids.length]) {
            ids = ids.filter((_, j) => j !== i && j !== (i + 1) % ids.length);
            changed = true;
            break;
          }
        }
      }
      if (ids.length < 3 || new Set(ids).size !== ids.length) {
        continue;
      }
      const polygon = ids.map((id) => points.get(id)!);
      if (area(polygon) < EPS) {
        continue;
      }
      const min = ids.indexOf(Math.min(...ids));
      const canonical = [...ids.slice(min), ...ids.slice(0, min)];
      result.push({ ids, polygon, key: canonical.join("-") });
    }
  }
  return result;
}

export function corners<T extends Vec>(p: T[]): T[] {
  return p.filter((b, i) => {
    const a = p[(i + p.length - 1) % p.length],
      c = p[(i + 1) % p.length];
    return distance(b, projection(b, a, c).point) > 1e-6;
  });
}

export function kind(p: Vec[]): string {
  const q = corners(p);
  if (q.length === 3) {
    return "三角形";
  }
  if (
    q.length === 4 &&
    distance(
      { x: q[0].x + q[2].x, y: q[0].y + q[2].y },
      { x: q[1].x + q[3].x, y: q[1].y + q[3].y },
    ) < 0.01
  ) {
    return "平行四辺形";
  }
  return `${q.length}角形`;
}

export function convex(p: Vec[]): boolean {
  return p.every(
    (a, i) =>
      cross(sub(p[(i + 1) % p.length], a), sub(p[(i + 2) % p.length], p[(i + 1) % p.length])) >=
      -EPS,
  );
}

/** Returns the exterior of a single hole-free inset; split or collapsed fills return no ring. */
export function inset(p: Point[]): Vec[] {
  const regions = insetRegions(p);
  return regions.length === 1 && regions[0].length === 1 ? regions[0][0] : [];
}

/** Returns inset polygon components, each containing an exterior ring followed by hole rings. */
export function insetRegions(p: Point[]): Vec[][][] {
  if (p.length < 3 || p.some((a, i) => distance(a, p[(i + 1) % p.length]) < EPS)) {
    return [];
  }
  const strips: [number, number][][][] = [];
  const offsets = p.map((a, i) => {
    const b = p[(i + 1) % p.length],
      length = distance(a, b);
    const nx = -(b.y - a.y) / length,
      ny = (b.x - a.x) / length;
    return {
      u: { x: a.x + (nx * a.width) / 2, y: a.y + (ny * a.width) / 2 },
      v: { x: b.x + (nx * b.width) / 2, y: b.y + (ny * b.width) / 2 },
    };
  });
  const add = (ring: Vec[]) => {
    if (Math.abs(area(ring)) > EPS) {
      strips.push([ring.map((q) => [q.x, q.y])]);
    }
  };
  for (let i = 0; i < p.length; i++) {
    const a = p[i],
      b = p[(i + 1) % p.length],
      current = offsets[i],
      previous = offsets[(i + p.length - 1) % p.length];
    add([a, b, current.v, current.u]);
    if (cross(sub(a, p[(i + p.length - 1) % p.length]), sub(b, a)) < -EPS) {
      const r = sub(previous.v, previous.u),
        t = sub(current.v, current.u),
        det = cross(r, t);
      if (Math.abs(det) > EPS) {
        const k = cross(sub(current.u, previous.u), t) / det;
        const m = { x: previous.u.x + r.x * k, y: previous.u.y + r.y * k };
        add([a, previous.v, m, current.u]);
      }
    }
  }
  const source: [number, number][][] = [p.map((q) => [q.x, q.y])];
  if (!strips.length) {
    return [[p.map(({ x, y }) => ({ x, y }))]];
  }
  const result = polygonClipping.difference(source, ...strips);
  return result.map((polygon) =>
    polygon.map((ring) => ring.slice(0, -1).map(([x, y]) => ({ x, y }))),
  );
}

export function canMove(d: Drawing, id: number, p: Vec): boolean {
  if (d.points.some((q) => q.id !== id && distance(p, q) < 0.5)) {
    return false;
  }
  const get = (n: number) => (n === id ? p : d.points.find((q) => q.id === n)!);
  for (let i = 0; i < d.edges.length; i++) {
    for (let j = i + 1; j < d.edges.length; j++) {
      const a = d.edges[i],
        b = d.edges[j];
      const shared = [a.a, a.b].some((n) => n === b.a || n === b.b);
      const hit = intersection(get(a.a), get(a.b), get(b.a), get(b.b));
      if (!shared && hit) {
        return false;
      }
      // Collinear overlaps would merge faces without an explicit split operation.
      for (const n of [a.a, a.b]) {
        if (
          n !== b.a &&
          n !== b.b &&
          distance(get(n), projection(get(n), get(b.a), get(b.b)).point) < EPS
        ) {
          return false;
        }
      }
      for (const n of [b.a, b.b]) {
        if (
          n !== a.a &&
          n !== a.b &&
          distance(get(n), projection(get(n), get(a.a), get(a.b)).point) < EPS
        ) {
          return false;
        }
      }
    }
  }
  return true;
}

export interface Snap {
  point: Vec;
  label: string;
  guides: [Vec, Vec][];
}
export interface GridSpacing {
  /** Horizontal distance between vertical guides; zero disables this axis. */
  x: number;
  /** Vertical distance between horizontal guides; zero disables this axis. */
  y: number;
  origin?: number;
  extent?: number;
}
export function snap(
  d: Drawing,
  p: Vec,
  moving: number | null,
  threshold: number,
  grid: GridSpacing,
): Snap {
  const gridCoordinate = (value: number, step: number) => {
    if (!Number.isFinite(step) || step <= 0) {
      return null;
    }
    const candidate =
      (grid.origin ?? 0) +
      Math.max(
        0,
        Math.min(
          Math.floor((grid.extent ?? 480) / step) * step,
          Math.round((value - (grid.origin ?? 0)) / step) * step,
        ),
      );
    return Math.abs(candidate - value) < threshold ? candidate : null;
  };
  const gx = gridCoordinate(p.x, grid.x),
    gy = gridCoordinate(p.y, grid.y);
  if (gx !== null && gy !== null) {
    return {
      point: { x: gx, y: gy },
      label: "グリッドに整列",
      guides: [
        [
          { x: gx, y: 0 },
          { x: gx, y: 480 },
        ],
        [
          { x: 0, y: gy },
          { x: 480, y: gy },
        ],
      ],
    };
  }
  const others = d.points.filter((q) => q.id !== moving);
  let best: Snap = { point: p, label: "", guides: [] },
    score = threshold;
  const offer = (point: Vec, label: string, guides: [Vec, Vec][], priority = 1) => {
    const dist = distance(point, p) * priority;
    if (dist < score) {
      best = { point, label, guides };
      score = dist;
    }
  };
  // Two connected sides define the fourth vertex exactly.
  for (const b of others) {
    const ns = d.edges
      .filter((e) => e.a === b.id || e.b === b.id)
      .map((e) => others.find((p) => p.id === (e.a === b.id ? e.b : e.a)))
      .filter((p): p is Point => !!p);
    for (let i = 0; i < ns.length; i++) {
      for (let j = i + 1; j < ns.length; j++) {
        const a = ns[i],
          c = ns[j],
          q = { x: a.x + c.x - b.x, y: a.y + c.y - b.y };
        offer(
          q,
          "平行四辺形の第4頂点",
          [
            [a, q],
            [c, q],
          ],
          0.65,
        );
      }
    }
  }
  if (best.label) {
    return best;
  }
  const x = others.reduce<Point | null>(
    (v, q) =>
      Math.abs(q.x - p.x) < threshold && (!v || Math.abs(q.x - p.x) < Math.abs(v.x - p.x)) ? q : v,
    null,
  );
  const y = others.reduce<Point | null>(
    (v, q) =>
      Math.abs(q.y - p.y) < threshold && (!v || Math.abs(q.y - p.y) < Math.abs(v.y - p.y)) ? q : v,
    null,
  );
  const useGridX = gx !== null && (!x || Math.abs(gx - p.x) <= Math.abs(x.x - p.x));
  const useGridY = gy !== null && (!y || Math.abs(gy - p.y) <= Math.abs(y.y - p.y));
  const sx = useGridX ? gx! : x?.x,
    sy = useGridY ? gy! : y?.y;
  if (sx !== undefined || sy !== undefined) {
    return {
      point: { x: sx ?? p.x, y: sy ?? p.y },
      label:
        useGridX || useGridY
          ? "グリッドに整列"
          : x && y
            ? "水平・垂直に整列"
            : x
              ? "垂直に整列"
              : "水平に整列",
      guides: [
        ...(sx !== undefined
          ? [
              [
                { x: sx, y: 0 },
                { x: sx, y: 480 },
              ] as [Vec, Vec],
            ]
          : []),
        ...(sy !== undefined
          ? [
              [
                { x: 0, y: sy },
                { x: 480, y: sy },
              ] as [Vec, Vec],
            ]
          : []),
      ],
    };
  }
  const connected =
    moving === null
      ? []
      : d.edges
          .filter((e) => e.a === moving || e.b === moving)
          .map((e) => others.find((p) => p.id === (e.a === moving ? e.b : e.a))!);
  for (const origin of connected) {
    const directions = [
      0,
      Math.PI / 6,
      Math.PI / 4,
      Math.PI / 3,
      Math.PI / 2,
      (2 * Math.PI) / 3,
      (3 * Math.PI) / 4,
      (5 * Math.PI) / 6,
    ];
    for (const e of d.edges) {
      if (e.a === moving || e.b === moving) {
        continue;
      }
      const a = others.find((p) => p.id === e.a)!,
        b = others.find((p) => p.id === e.b)!;
      directions.push(Math.atan2(b.y - a.y, b.x - a.x));
    }
    for (const angle of directions) {
      const dx = Math.cos(angle),
        dy = Math.sin(angle),
        t = (p.x - origin.x) * dx + (p.y - origin.y) * dy;
      const q = { x: origin.x + t * dx, y: origin.y + t * dy };
      offer(q, "辺と平行・角度に整列", [[origin, q]]);
    }
  }
  return best;
}

export function preservePaint(d: Drawing, before: Face[], paints: Drawing["paints"]): void {
  const next: Drawing["paints"] = {};
  for (const face of faces(d)) {
    const old =
      before.find((f) => f.key === face.key) ??
      before.find((f) => contains(f.polygon, center(face.polygon)));
    next[face.key] =
      old && paints[old.key] ? { ...paints[old.key] } : { color: "#d9d6ee", visible: true };
  }
  d.paints = next;
}

export type Transform = "clockwise" | "counterclockwise" | "horizontal" | "vertical";

/** Transforms the entire graph about (240, 240), retaining point widths and face paints. */
export function transformDrawing(d: Drawing, operation: Transform): void {
  const signature = (f: Face) => [...f.ids].sort((a, b) => a - b).join("-");
  const paints = new Map(faces(d).map((f) => [signature(f), d.paints[f.key]]));
  for (const p of d.points) {
    const { x, y } = p;
    switch (operation) {
      case "clockwise":
        p.x = 480 - y;
        p.y = x;
        break;
      case "counterclockwise":
        p.x = y;
        p.y = 480 - x;
        break;
      case "horizontal":
        p.x = 480 - x;
        break;
      case "vertical":
        p.y = 480 - y;
        break;
    }
  }
  d.paints = Object.fromEntries(
    faces(d).map((f) => [f.key, paints.get(signature(f)) ?? { color: "#d9d6ee", visible: true }]),
  );
}

export function preset(empty = false): Drawing {
  const d: Drawing = { version: 1, nextId: 1, points: [], edges: [], paints: {} };
  const ids = [
    [60, 60],
    [420, 60],
    [420, 420],
    [60, 420],
  ].map(([x, y]) => addPoint(d, { x, y }, 4.5).id);
  d.edges = ids.map((a, i) => ({ a, b: ids[(i + 1) % 4] }));
  if (!empty) {
    const cut = (a: Vec, b: Vec) => connect(d, addPoint(d, a, 4.5).id, addPoint(d, b, 4.5).id);
    for (const x of [195, 285]) {
      cut({ x, y: 60 }, { x, y: 420 });
    }
    for (const intercept of [-20, 70, 160, 250]) {
      const left = Math.max(60, (60 - intercept) * 3);
      const right = Math.min(420, (420 - intercept) * 3);
      cut({ x: left, y: left / 3 + intercept }, { x: right, y: right / 3 + intercept });
    }
  }
  for (const f of faces(d)) {
    d.paints[f.key] = {
      visible: true,
      color: center(f.polygon).x > 195 && center(f.polygon).x < 285 ? "#d9825b" : "#d9d6ee",
    };
  }
  return d;
}

export function decode(text: string): Drawing {
  const d = JSON.parse(text) as Drawing;
  if (
    d?.version !== 1 ||
    !Array.isArray(d.points) ||
    !Array.isArray(d.edges) ||
    d.points.length > 300 ||
    d.edges.length > 900 ||
    !d.paints ||
    typeof d.paints !== "object"
  ) {
    throw new Error("対応するShape LabのJSONではありません（点は最大300個）。");
  }
  const ids = new Set<number>();
  for (const p of d.points) {
    if (
      !p ||
      !Number.isSafeInteger(p.id) ||
      p.id < 1 ||
      ids.has(p.id) ||
      !Number.isFinite(p.x) ||
      !Number.isFinite(p.y) ||
      p.x < 0 ||
      p.x > 480 ||
      p.y < 0 ||
      p.y > 480 ||
      !Number.isFinite(p.width) ||
      p.width < 0 ||
      p.width > 32
    ) {
      throw new Error("点の座標・ID・太さが不正です。");
    }
    ids.add(p.id);
  }
  for (const e of d.edges) {
    if (!e || !ids.has(e.a) || !ids.has(e.b) || e.a === e.b) {
      throw new Error("分割線の端点が不正です。");
    }
  }
  for (const paint of Object.values(d.paints)) {
    if (!paint || !/^#[0-9a-f]{6}$/i.test(paint.color) || typeof paint.visible !== "boolean") {
      throw new Error("面の色が不正です。");
    }
  }
  d.nextId = Math.max(0, ...ids) + 1;
  normalize(d);
  if (d.points.length > 300) {
    throw new Error("交点を含む点数が上限を超えています。");
  }
  return d;
}

export function exportSvg(d: Drawing): string {
  const polygons = faces(d).flatMap((f) => {
    const paint = d.paints[f.key] ?? { color: "#d9d6ee", visible: true },
      regions = insetRegions(f.polygon);
    if (!paint.visible) {
      return [];
    }
    if (!regions.length) {
      throw new Error("太さで潰れた面があります。太さを調整してください。");
    }
    return regions.map((rings) =>
      rings.length === 1
        ? `  <polygon points="${rings[0].map((p) => `${+p.x.toFixed(5)},${+p.y.toFixed(5)}`).join(" ")}" fill="${paint.color}"/>`
        : `  <path d="${rings.map((ring) => `M${ring.map((p) => `${+p.x.toFixed(5)},${+p.y.toFixed(5)}`).join(" L")} Z`).join(" ")}" fill="${paint.color}" fill-rule="evenodd"/>`,
    );
  });
  return `<svg xmlns="http://www.w3.org/2000/svg" width="480" height="480" viewBox="0 0 480 480">\n${polygons.join("\n")}\n</svg>\n`;
}
