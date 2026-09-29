import "./style.css";
import {
  addPoint,
  canMove,
  connect,
  corners,
  decode,
  distance,
  edgeKey,
  exportSvg,
  faces,
  insetRegions,
  kind,
  normalize,
  preset,
  preservePaint,
  projection,
  removePoint,
  snap,
  transformDrawing,
  type Transform,
  type Drawing,
  type Edge,
  type Face,
  type Point,
  type Snap,
  type Vec,
} from "./geometry.ts";
import { decodeGrid, defaultGrid, gridSpacing, validDivisions, type GridSettings } from "./grid.ts";

type Mode = "select" | "point" | "connect" | "face";
const storageKey = "stillwind-shape-lab-v1";
const widthStorageKey = "stillwind-shape-lab-default-width-v1";
const gridStorageKey = "stillwind-shape-lab-grid-divisions-v2";
let gridSettings = defaultGrid();
try {
  const saved = localStorage.getItem(gridStorageKey);
  if (saved) {
    gridSettings = decodeGrid(saved);
  }
} catch {
  gridSettings = defaultGrid();
}
let drawing = preset(),
  restoreMessage = "共有点から、形を組み立てる。";
try {
  const saved = localStorage.getItem(storageKey);
  if (saved) {
    drawing = decode(saved);
    restoreMessage = "前回の編集を復元しました。";
  }
} catch {
  restoreMessage = "前回のデータを読み込めなかったため、サンプルを開きました。";
}
let mode: Mode = "select",
  selected: number[] = [],
  selectedEdge: string | null = null,
  selectedFace: string | null = null;
let showMesh = true,
  assist = true,
  showGrid = true,
  defaultWidth = 4.5;
try {
  const saved = localStorage.getItem(widthStorageKey);
  const value = saved === null ? NaN : Number(saved);
  if (Number.isFinite(value) && value >= 0 && value <= 32) {
    defaultWidth = value;
  } else if (
    drawing.points.length &&
    drawing.points.every((p) => p.width === drawing.points[0].width)
  ) {
    defaultWidth = drawing.points[0].width;
  }
} catch {
  defaultWidth = 4.5;
}
let zoom = 1,
  pan: Vec = { x: 0, y: 0 },
  space = false,
  lastSnap: Snap | null = null;
const undo: string[] = [],
  redo: string[] = [];
const app = document.querySelector<HTMLDivElement>("#app")!;
app.innerHTML = `
  <header class="topbar">
    <a class="brand" href="./"><span class="brand-mark">◈</span><span>Stillwind <b>Shape Lab</b></span><small>LOCAL EDITOR</small></a>
    <div class="top-actions"><span id="save-state">このブラウザに自動保存</span><button id="load">JSONを開く</button><button id="save">JSON保存</button><button class="primary" id="export">SVGを書き出す ↗</button></div>
  </header>
  <main>
    <aside class="left-panel">
      <div class="section-title">BUILD <span>01</span></div>
      <h1>点を共有する。<br>形が揃う。</h1>
      <p class="intro">点と線が設計図。<br>色のついた面は、その結果。</p>
      <nav class="tools" aria-label="編集ツール">
        <button data-mode="select"><span>↖</span>選択・移動<kbd>V</kbd></button>
        <button data-mode="point"><span>＋</span>点を追加<kbd>P</kbd></button>
        <button data-mode="connect"><span>╱</span>2点を結ぶ・解除<kbd>L</kbd></button>
        <button data-mode="face"><span>◇</span>面を選択<kbd>F</kbd></button>
      </nav>
      <div class="tool-note" id="tool-note"></div>
      <div class="section-title">GEOMETRY</div>
      <label class="switch"><span>位置アシスト</span><input id="assist" type="checkbox" checked></label>
      <p class="caption">頂点の水平・垂直、辺の平行、平行四辺形の第4頂点。Shiftで一時解除。</p>
      <label class="field"><span>グリッド設定</span><select id="grid-mode"><option value="shared">縦横共通</option><option value="independent">縦横別</option><option value="none">なし</option></select></label>
      <div id="shared-grid"><label class="field"><span>共通の分割数</span><input id="grid-shared" type="number" min="1" max="360" step="1" value="24"></label></div>
      <div id="independent-grid" hidden><label class="field"><span>横（X）の分割数</span><input id="grid-x" type="number" min="0" max="360" step="1" value="24"></label><label class="field"><span>縦（Y）の分割数</span><input id="grid-y" type="number" min="0" max="360" step="1" value="24"></label></div>
      <p class="caption" id="grid-note">中央360×360領域を分割。24分割＝15px。</p>
      <label class="field"><span>共通の太さ</span><div><input id="global-width" type="number" value="${defaultWidth}" min="0" max="32" step="0.5" aria-label="共通の太さ"><span class="unit">px</span></div></label>
      <button class="wide" id="apply-width">すべての点に適用</button>
      <p class="caption">点の太さの半分ずつを、共有する辺の両側から引きます。</p>
      <div class="section-title">TRANSFORM</div>
      <div class="transform-buttons">
        <button data-transform="counterclockwise">左に90°回転</button>
        <button data-transform="clockwise">右に90°回転</button>
        <button data-transform="horizontal">左右反転</button>
        <button data-transform="vertical">上下反転</button>
      </div>
      <p class="caption">中央を基準に図形全体を変換します。</p>
      <div class="section-title">START FROM</div>
      <div class="preset-buttons"><button id="sample">正方形・分割あり</button><button id="blank">正方形・分割なし</button></div>
      <p class="caption">四隅も面として残ります。除外したい面を選び「面を表示」をオフ。切り替えはUndoで戻せます。</p>
    </aside>
    <section class="workspace" aria-label="描画エリア">
      <div class="canvas-bar"><span class="file-name"><i></i> Untitled geometry <small>480 × 480</small></span><div><button id="undo" title="元に戻す Ctrl/Cmd+Z" aria-label="元に戻す">↶</button><button id="redo" title="やり直す Ctrl/Cmd+Shift+Z" aria-label="やり直す">↷</button><button id="fit">全体表示</button></div></div>
      <div class="canvas-wrap"><canvas id="editor" tabindex="0" aria-label="共有点エディタ。点を追加ツールでクリック、2点を結ぶツールで順に2点を選択。"></canvas><div class="canvas-badge">SHARED VERTICES <span id="counts"></span></div><div id="snap-label" hidden></div></div>
      <div class="canvas-bottom"><label><input id="mesh" type="checkbox" checked>設計線</label><label><input id="grid" type="checkbox" checked>グリッドを表示</label><span>ホイール：ズーム · Space＋ドラッグ：移動</span><b id="zoom">100%</b></div>
      <div class="preview-strip"><div><span class="section-title">LIVE PREVIEW</span><p>隙間と小さい表示を確認</p></div><div class="preview-item"><canvas data-preview="16"></canvas><small>16</small></div><div class="preview-item"><canvas data-preview="32"></canvas><small>32</small></div><div class="preview-item"><canvas data-preview="64"></canvas><small>64</small></div><div class="preview-item dark"><canvas data-preview="80"></canvas><small>ON DARK</small></div></div>
    </section>
    <aside class="right-panel"><div class="section-title">INSPECTOR <span>02</span></div><div id="inspector"></div><div class="section-title faces-title">FACES <span id="face-count"></span></div><div id="faces"></div></aside>
  </main>
  <footer><span id="status" role="status"></span><span>ダブルクリック：点を追加 / ミドルクリック：点・辺を削除</span></footer>
  <input type="file" id="import-file" accept="application/json,.json" hidden>
`;
const $ = <T extends HTMLElement = HTMLElement>(s: string) => document.querySelector<T>(s)!;
const canvas = $<HTMLCanvasElement>("#editor"),
  ctx = canvas.getContext("2d")!;
let width = 600,
  height = 600;
const baseScale = () => Math.min(width, height) / 560;
const scale = () => baseScale() * zoom;
const screen = (p: Vec): Vec => ({
  x: width / 2 + pan.x + (p.x - 240) * scale(),
  y: height / 2 + pan.y + (p.y - 240) * scale(),
});
const world = (p: Vec): Vec => ({
  x: (p.x - width / 2 - pan.x) / scale() + 240,
  y: (p.y - height / 2 - pan.y) / scale() + 240,
});
const eventPosition = (e: MouseEvent): Vec => {
  const r = canvas.getBoundingClientRect();
  return { x: e.clientX - r.left, y: e.clientY - r.top };
};
const bounded = (p: Vec): Vec => ({
  x: Math.max(0, Math.min(480, p.x)),
  y: Math.max(0, Math.min(480, p.y)),
});
const pointById = (id: number) => drawing.points.find((p) => p.id === id)!;
const paintOf = (face: Face) => drawing.paints[face.key] ?? { color: "#d9d6ee", visible: true };

function status(text: string) {
  $("#status").textContent = text;
}
function persist() {
  try {
    localStorage.setItem(storageKey, JSON.stringify(drawing));
    $("#save-state").textContent = "このブラウザに保存済み";
  } catch {
    $("#save-state").textContent = "自動保存できません — JSON保存を使用";
  }
}
function pushHistory(before: string) {
  if (before === JSON.stringify(drawing)) {
    return;
  }
  undo.push(before);
  if (undo.length > 100) {
    undo.shift();
  }
  redo.length = 0;
  persist();
}
function mutate(fn: () => void, message?: string) {
  const before = JSON.stringify(drawing),
    previousFaces = faces(drawing),
    paints = structuredClone(drawing.paints);
  try {
    fn();
  } catch (error) {
    drawing = JSON.parse(before) as Drawing;
    status((error as Error).message);
    refresh();
    return;
  }
  preservePaint(drawing, previousFaces, paints);
  pushHistory(before);
  if (message) {
    status(message);
  }
  refresh();
}
function setMode(next: Mode) {
  mode = next;
  selectedEdge = null;
  lastSnap = null;
  status(
    {
      select: "点をドラッグして移動。Shift＋クリックで複数選択。",
      point: "キャンバスをクリックして点を追加。辺上なら共有点として挿入。",
      connect: "任意の2点を順にクリック。既存の接続は同じ2点を選ぶと解除。",
      face: "面をクリックして色や表示を変更。",
    }[next],
  );
  selected = [];
  refresh();
}

function path(context: CanvasRenderingContext2D, p: Vec[], map: (p: Vec) => Vec, begin = true) {
  if (begin) {
    context.beginPath();
  }
  p.forEach((v, i) => {
    const q = map(v);
    if (i) {
      context.lineTo(q.x, q.y);
    } else {
      context.moveTo(q.x, q.y);
    }
  });
  context.closePath();
}
function render() {
  ctx.clearRect(0, 0, width, height);
  const origin = screen({ x: 0, y: 0 }),
    size = 480 * scale();
  ctx.shadowColor = "#24304912";
  ctx.shadowBlur = 24;
  ctx.shadowOffsetY = 4;
  ctx.fillStyle = "#fff";
  ctx.fillRect(origin.x, origin.y, size, size);
  ctx.shadowBlur = 0;
  ctx.shadowOffsetY = 0;
  const baseOrigin = screen({ x: 60, y: 60 });
  ctx.strokeStyle = "#aeb8c7";
  ctx.lineWidth = 1;
  ctx.setLineDash([4, 4]);
  ctx.strokeRect(baseOrigin.x, baseOrigin.y, 360 * scale(), 360 * scale());
  ctx.setLineDash([]);
  const fs = faces(drawing);
  for (const face of fs) {
    const paint = paintOf(face),
      fill = insetRegions(face.polygon);
    if (paint.visible && fill.length) {
      for (const rings of fill) {
        ctx.beginPath();
        for (const ring of rings) {
          path(ctx, ring, screen, false);
        }
        ctx.fillStyle = paint.color;
        ctx.fill("evenodd");
      }
    }
    if (paint.visible && !fill.length) {
      path(ctx, face.polygon, screen);
      ctx.fillStyle = "#df665526";
      ctx.fill();
    }
    if (selectedFace === face.key) {
      path(ctx, face.polygon, screen);
      ctx.strokeStyle = "#de8057";
      ctx.lineWidth = 2;
      ctx.setLineDash([5, 4]);
      ctx.stroke();
      ctx.setLineDash([]);
    }
  }
  if (showGrid) {
    const spacing = gridSpacing(gridSettings);
    ctx.beginPath();
    for (const axis of ["x", "y"] as const) {
      if (spacing[axis] <= 0) {
        continue;
      }
      for (let i = 0; i <= Math.round(360 / spacing[axis]); i++) {
        const value = 60 + i * spacing[axis];
        const a = screen(axis === "x" ? { x: value, y: 60 } : { x: 60, y: value });
        const b = screen(axis === "x" ? { x: value, y: 420 } : { x: 420, y: value });
        ctx.moveTo(a.x, a.y);
        ctx.lineTo(b.x, b.y);
      }
    }
    ctx.strokeStyle = "#46536b40";
    ctx.lineWidth = 0.6;
    ctx.stroke();
  }
  if (showMesh) {
    for (const e of drawing.edges) {
      const a = screen(pointById(e.a)),
        b = screen(pointById(e.b));
      ctx.beginPath();
      ctx.moveTo(a.x, a.y);
      ctx.lineTo(b.x, b.y);
      ctx.lineWidth = selectedEdge === edgeKey(e) ? 3 : 1;
      ctx.strokeStyle = selectedEdge === edgeKey(e) ? "#d36d46" : "#747b985e";
      ctx.stroke();
    }
    for (const p of drawing.points) {
      const q = screen(p),
        active = selected.includes(p.id);
      ctx.beginPath();
      ctx.arc(q.x, q.y, active ? 5.5 : 3.5, 0, Math.PI * 2);
      ctx.fillStyle = active ? "#d47650" : "#fff";
      ctx.fill();
      ctx.strokeStyle = active ? "#a64a27" : "#68728e";
      ctx.lineWidth = 1.4;
      ctx.stroke();
      if (active || drawing.points.length <= 24) {
        ctx.font = "10px system-ui";
        ctx.fillStyle = active ? "#a64a27" : "#778197";
        ctx.fillText(`P${p.id}`, q.x + 7, q.y - 7);
      }
    }
  }
  if (lastSnap) {
    for (const [a, b] of lastSnap.guides) {
      const u = screen(a),
        v = screen(b);
      ctx.beginPath();
      ctx.moveTo(u.x, u.y);
      ctx.lineTo(v.x, v.y);
      ctx.strokeStyle = "#158e88";
      ctx.lineWidth = 1;
      ctx.setLineDash([4, 3]);
      ctx.stroke();
      ctx.setLineDash([]);
    }
  }
  $("#snap-label").hidden = !lastSnap?.label;
  $("#snap-label").textContent = lastSnap?.label ?? "";
  $("#counts").textContent = `${drawing.points.length} POINTS / ${fs.length} FACES`;
  $("#zoom").textContent = `${Math.round(zoom * 100)}%`;
  for (const preview of document.querySelectorAll<HTMLCanvasElement>("[data-preview]")) {
    const s = Number(preview.dataset.preview),
      ratio = devicePixelRatio || 1;
    preview.width = s * ratio;
    preview.height = s * ratio;
    preview.style.width = `${s}px`;
    preview.style.height = `${s}px`;
    const c = preview.getContext("2d")!;
    c.scale(ratio, ratio);
    for (const f of fs) {
      const paint = paintOf(f),
        regions = insetRegions(f.polygon);
      if (paint.visible && regions.length) {
        for (const rings of regions) {
          c.beginPath();
          for (const ring of rings) {
            path(c, ring, (p) => ({ x: (p.x * s) / 480, y: (p.y * s) / 480 }), false);
          }
          c.fillStyle = paint.color;
          c.fill("evenodd");
        }
      }
    }
  }
}

function inspector() {
  const panel = $("#inspector"),
    fs = faces(drawing),
    face = fs.find((f) => f.key === selectedFace);
  if (selected.length) {
    const p = pointById(selected[0]);
    panel.innerHTML = `<h2>${selected.length > 1 ? `${selected.length}個の共有点` : `共有点 P${p.id}`}</h2><p class="caption">${drawing.edges.filter((e) => e.a === p.id || e.b === p.id).length}本の辺に接続 · ${fs.filter((f) => f.ids.includes(p.id)).length}面が共有</p><div class="coordinates"><label>X<input id="point-x" type="number" min="0" max="480" step="0.1" value="${+p.x.toFixed(3)}"></label><label>Y<input id="point-y" type="number" min="0" max="480" step="0.1" value="${+p.y.toFixed(3)}"></label></div><label class="field"><span>点の太さ</span><input id="point-width" type="number" min="0" max="32" step="0.5" value="${+p.width.toFixed(3)}"></label><p class="caption">複数選択時は、太さをまとめて変更できます。</p>${selected.length === 2 ? '<button id="join-selected" class="wide">2点を結ぶ・解除</button>' : ""}<button id="delete-selection" class="wide danger">点を削除</button>`;
    for (const axis of ["x", "y"] as const) {
      $<HTMLInputElement>(`#point-${axis}`).onchange = (e) => {
        const value = Number((e.target as HTMLInputElement).value),
          q = { x: p.x, y: p.y, [axis]: value };
        if (!Number.isFinite(value) || value < 0 || value > 480 || !canMove(drawing, p.id, q)) {
          status("この座標では辺が交差・重複します。");
          inspector();
          return;
        }
        mutate(() => Object.assign(p, q), "共有点の座標を変更しました。");
      };
    }
    $<HTMLInputElement>("#point-width").onchange = (e) => {
      const n = Number((e.target as HTMLInputElement).value);
      if (!Number.isFinite(n) || n < 0 || n > 32) {
        inspector();
        return;
      }
      mutate(
        () =>
          selected.forEach((id) => {
            pointById(id).width = n;
          }),
        "共有する辺の太さを更新しました。",
      );
    };
    if (selected.length === 2) {
      $("#join-selected").onclick = () => toggleConnection(selected[0], selected[1]);
    }
    $("#delete-selection").onclick = removeSelected;
  } else if (selectedEdge) {
    const e = drawing.edges.find((e) => edgeKey(e) === selectedEdge);
    if (!e) {
      selectedEdge = null;
      inspector();
      return;
    }
    panel.innerHTML = `<h2>分割線 P${e.a} → P${e.b}</h2><p class="caption">この線を解除すると、両側の面がつながります。点は残ります。</p><button class="wide danger" id="delete-selection">分割を解除</button>`;
    $("#delete-selection").onclick = removeSelected;
  } else if (face) {
    const paint = paintOf(face);
    panel.innerHTML = `<h2>${kind(face.polygon)}</h2><p class="caption">頂点：${corners(
      face.polygon,
    )
      .map((p) => `P${p.id}`)
      .join(
        " → ",
      )} / 辺上の共有点：${face.ids.length - corners(face.polygon).length}</p><label class="field"><span>面の色</span><input id="face-color" type="color" value="${paint.color}"></label><div class="swatches">${["#d9d6ee", "#b8b4d8", "#d9825b", "#379988", "#34445f", "#ffffff"].map((c) => `<button data-color="${c}" style="--swatch:${c}" aria-label="色 ${c}"></button>`).join("")}</div><label class="switch"><span>面を表示</span><input id="face-visible" type="checkbox" ${paint.visible ? "checked" : ""}></label><p class="caption">${!insetRegions(face.polygon).length ? "太さで面が潰れています。点の太さを小さくしてください。" : "点を動かすと、この面も自動で更新されます。"}</p>`;
    const setPaint = (patch: Partial<typeof paint>) => {
      const before = JSON.stringify(drawing);
      drawing.paints[face.key] = { ...paintOf(face), ...patch };
      pushHistory(before);
      refresh();
    };
    $<HTMLInputElement>("#face-color").onchange = (e) =>
      setPaint({ color: (e.target as HTMLInputElement).value });
    $<HTMLInputElement>("#face-visible").onchange = (e) =>
      setPaint({ visible: (e.target as HTMLInputElement).checked });
    document.querySelectorAll<HTMLButtonElement>("[data-color]").forEach((b) => {
      b.onclick = () => setPaint({ color: b.dataset.color! });
    });
  } else {
    panel.innerHTML = `<div class="empty-inspector"><span>◇</span><h2>点・線・面を選択</h2><p>点は座標と太さ、面は色を編集。<br>点は隣り合う図形で共有されます。</p></div><div class="steps"><b>QUICK START</b><p>1. 辺の上に点を追加</p><p>2. 「2点を結ぶ」で分割</p><p>3. 点をドラッグして形を調整</p></div>`;
  }
  $("#face-count").textContent = String(fs.length);
  $("#faces").innerHTML =
    fs
      .map(
        (f, i) =>
          `<button class="face-row ${f.key === selectedFace ? "active" : ""}" data-face="${f.key}"><i style="background:${paintOf(f).color};opacity:${paintOf(f).visible ? 1 : 0.2}"></i><span>面 ${String(i + 1).padStart(2, "0")}<small>${kind(f.polygon)}${!insetRegions(f.polygon).length ? " · 太さ超過" : ""}</small></span><span class="face-visible">${paintOf(f).visible ? "●" : "○"}</span></button>`,
      )
      .join("") || '<p class="caption">点を線で囲むと面ができます。</p>';
  document.querySelectorAll<HTMLButtonElement>("[data-face]").forEach((b) => {
    b.onclick = () => {
      selectedFace = b.dataset.face!;
      selected = [];
      selectedEdge = null;
      refresh();
    };
  });
}

function refresh() {
  selected = selected.filter((id) => drawing.points.some((p) => p.id === id));
  document
    .querySelectorAll<HTMLButtonElement>("[data-mode]")
    .forEach((b) => b.classList.toggle("active", b.dataset.mode === mode));
  $("#tool-note").textContent = {
    select:
      "共有点をドラッグ。Shift＋クリックで複数選択。選択点をShift＋ダブルクリックで他の選択点へ接続。",
    point: "辺の上にも追加できます。閉じた線で囲むと面になります。",
    connect: "2点を順番にクリック。同じ接続を選び直すと解除。",
    face: "面の色と表示を編集します。",
  }[mode];
  $<HTMLButtonElement>("#undo").disabled = !undo.length;
  $<HTMLButtonElement>("#redo").disabled = !redo.length;
  canvas.style.cursor = space ? "grab" : mode === "point" ? "crosshair" : "default";
  inspector();
  render();
}

function hitPoint(p: Vec): Point | undefined {
  return drawing.points
    .filter((q) => distance(p, q) * scale() < 11)
    .sort((a, b) => distance(p, a) - distance(p, b))[0];
}
function hitEdge(p: Vec): Edge | undefined {
  return drawing.edges
    .filter((e) => distance(p, projection(p, pointById(e.a), pointById(e.b)).point) * scale() < 7)
    .sort(
      (a, b) =>
        distance(p, projection(p, pointById(a.a), pointById(a.b)).point) -
        distance(p, projection(p, pointById(b.a), pointById(b.b)).point),
    )[0];
}
function toggleConnection(a: number, b: number) {
  if (a === b) {
    status("別の点を選んでください。");
    return;
  }
  const u = pointById(a),
    v = pointById(b);
  const chain = drawing.edges.filter((e) =>
    [pointById(e.a), pointById(e.b)].every((p) => distance(p, projection(p, u, v).point) < 1e-6),
  );
  const complete =
    chain.reduce((n, e) => n + distance(pointById(e.a), pointById(e.b)), 0) >=
    distance(u, v) - 1e-6;
  mutate(
    () => {
      if (complete) {
        const keys = new Set(chain.map(edgeKey));
        drawing.edges = drawing.edges.filter((e) => !keys.has(edgeKey(e)));
      } else {
        connect(drawing, a, b);
      }
    },
    complete ? "分割線を解除しました。" : "分割線を追加しました。交点は共有点になります。",
  );
  selected = [];
  refresh();
}
function removeSelected() {
  if (selected.length) {
    mutate(() => {
      for (const id of selected) {
        removePoint(drawing, id);
      }
    }, "点を削除し、周囲の辺をつなぎ直しました。");
  } else if (selectedEdge) {
    mutate(() => {
      drawing.edges = drawing.edges.filter((e) => edgeKey(e) !== selectedEdge);
      selectedEdge = null;
    }, "分割を解除しました。");
  }
  refresh();
}

function insertPoint(p: Vec, bypassAssist: boolean) {
  if (hitPoint(p)) {
    return;
  }
  if (drawing.points.length >= 300) {
    status("点数の上限は300です。");
    return;
  }
  const edge = hitEdge(p);
  let q = bounded(p),
    w = defaultWidth;
  if (edge) {
    const a = pointById(edge.a),
      b = pointById(edge.b),
      projected = projection(p, a, b);
    q = projected.point;
    w = a.width + (b.width - a.width) * projected.t;
  } else if (assist && !bypassAssist) {
    lastSnap = snap(drawing, q, null, 8 / scale(), gridSpacing(gridSettings));
    q = bounded(lastSnap.point);
  }
  mutate(
    () => {
      const added = addPoint(drawing, q, w);
      selected = [added.id];
      selectedFace = null;
      selectedEdge = null;
      normalize(drawing);
    },
    edge ? "辺に共有点を挿入しました。" : "点を追加しました。",
  );
}

let shiftClickSelection: number[] = [];
canvas.onclick = (e) => {
  if (!e.shiftKey || e.detail !== 1 || space) {
    return;
  }
  const hit = hitPoint(world(eventPosition(e)));
  if (!hit) {
    return;
  }
  shiftClickSelection = [...selected];
  selected = selected.includes(hit.id)
    ? selected.filter((id) => id !== hit.id)
    : [...selected, hit.id];
  selectedFace = null;
  selectedEdge = null;
  refresh();
};
canvas.ondblclick = (e) => {
  if (e.button !== 0 || space) {
    return;
  }
  e.preventDefault();
  const p = world(eventPosition(e)),
    hit = hitPoint(p);
  if (e.shiftKey) {
    if (hit && shiftClickSelection.length >= 2 && shiftClickSelection.includes(hit.id)) {
      selected = [...shiftClickSelection];
      mutate(() => {
        for (const id of selected) {
          if (id !== hit.id) {
            connect(drawing, hit.id, id);
          }
        }
      }, "選択点への辺を追加しました。");
    }
    return;
  }
  insertPoint(p, false);
};
canvas.onauxclick = (e) => {
  if (e.button === 1) {
    e.preventDefault();
  }
};

let drag: { id?: number; start: Vec; pan: Vec; before: string; moved: boolean } | null = null;
canvas.onpointerdown = (e) => {
  if (e.button !== 0 && e.button !== 1) {
    return;
  }
  canvas.focus();
  canvas.setPointerCapture(e.pointerId);
  const pos = eventPosition(e),
    p = world(pos);
  if (e.button === 1) {
    e.preventDefault();
    const hit = hitPoint(p),
      edge = hit ? undefined : hitEdge(p);
    if (hit || edge) {
      selected = hit ? [hit.id] : [];
      selectedEdge = edge ? edgeKey(edge) : null;
      selectedFace = null;
      removeSelected();
    } else {
      drag = { start: pos, pan: { ...pan }, before: "", moved: false };
    }
    return;
  }
  if (space) {
    drag = { start: pos, pan: { ...pan }, before: "", moved: false };
    return;
  }
  const hit = hitPoint(p);
  if (e.shiftKey && hit) {
    return;
  }
  if (mode === "connect") {
    if (!hit) {
      status("接続する点をクリックしてください。点の追加はPキー。");
      return;
    }
    selectedFace = null;
    selectedEdge = null;
    if (selected.length === 1) {
      toggleConnection(selected[0], hit.id);
    } else {
      selected = [hit.id];
      status(`P${hit.id}を選択。もう1つの点をクリック。`);
      refresh();
    }
    return;
  }
  if (mode === "point") {
    if (hit) {
      selected = [hit.id];
      refresh();
      return;
    }
    insertPoint(p, e.shiftKey);
    return;
  }
  if (mode === "select" && hit) {
    selectedFace = null;
    selectedEdge = null;
    selected = [hit.id];
    drag = {
      id: hit.id,
      start: pos,
      pan: { ...pan },
      before: JSON.stringify(drawing),
      moved: false,
    };
    refresh();
    return;
  }
  selected = [];
  selectedEdge = null;
  selectedFace = null;
  const edge = mode === "select" ? hitEdge(p) : undefined;
  if (edge) {
    selectedEdge = edgeKey(edge);
  } else {
    selectedFace = faces(drawing).find((f) => containsPoint(f, p))?.key ?? null;
  }
  refresh();
};
function containsPoint(f: Face, p: Vec) {
  let inside = false;
  for (let i = 0, j = f.polygon.length - 1; i < f.polygon.length; j = i++) {
    const a = f.polygon[i],
      b = f.polygon[j];
    if (a.y > p.y !== b.y > p.y && p.x < ((b.x - a.x) * (p.y - a.y)) / (b.y - a.y) + a.x) {
      inside = !inside;
    }
  }
  return inside;
}
canvas.onpointermove = (e) => {
  if (!drag) {
    return;
  }
  const pos = eventPosition(e);
  if (drag.id === undefined) {
    pan = { x: drag.pan.x + pos.x - drag.start.x, y: drag.pan.y + pos.y - drag.start.y };
    render();
    return;
  }
  if (distance(pos, drag.start) < 2 && !drag.moved) {
    return;
  }
  let q = bounded(world(pos));
  lastSnap = null;
  if (assist && !e.shiftKey) {
    lastSnap = snap(drawing, q, drag.id, 8 / scale(), gridSpacing(gridSettings));
    q = bounded(lastSnap.point);
  }
  if (canMove(drawing, drag.id, q)) {
    Object.assign(pointById(drag.id), q);
    drag.moved = true;
    status(lastSnap?.label || "共有点を移動中");
    render();
  } else {
    status("移動先で辺が交差・重複するため、この位置には移動できません。");
  }
};
function finishDrag(cancel = false) {
  if (drag?.id !== undefined && drag.moved) {
    if (cancel) {
      drawing = decode(drag.before);
    } else {
      pushHistory(drag.before);
    }
  }
  drag = null;
  lastSnap = null;
  refresh();
}
canvas.onpointerup = () => finishDrag();
canvas.onpointercancel = () => finishDrag(true);
canvas.addEventListener(
  "wheel",
  (e) => {
    e.preventDefault();
    const pos = eventPosition(e),
      before = world(pos);
    zoom = Math.max(0.4, Math.min(5, zoom * Math.exp(-e.deltaY * 0.001)));
    const after = screen(before);
    pan.x += pos.x - after.x;
    pan.y += pos.y - after.y;
    render();
  },
  { passive: false },
);

function travel(back: boolean) {
  const source = back ? undo : redo,
    target = back ? redo : undo;
  if (!source.length) {
    return;
  }
  target.push(JSON.stringify(drawing));
  drawing = decode(source.pop()!);
  selected = [];
  selectedEdge = null;
  selectedFace = null;
  persist();
  refresh();
  status(back ? "元に戻しました。" : "やり直しました。");
}
document.querySelectorAll<HTMLButtonElement>("[data-mode]").forEach((b) => {
  b.onclick = () => setMode(b.dataset.mode as Mode);
});
document.querySelectorAll<HTMLButtonElement>("[data-transform]").forEach((button) => {
  button.onclick = () => {
    finishDrag();
    const before = JSON.stringify(drawing);
    const faceIds = faces(drawing).find((f) => f.key === selectedFace)?.ids;
    transformDrawing(drawing, button.dataset.transform as Transform);
    if (faceIds) {
      selectedFace =
        faces(drawing).find(
          (f) => f.ids.length === faceIds.length && f.ids.every((id) => faceIds.includes(id)),
        )?.key ?? null;
    }
    pushHistory(before);
    refresh();
    status(`${button.textContent}しました。Undoで戻せます。`);
  };
});
$("#undo").onclick = () => travel(true);
$("#redo").onclick = () => travel(false);
$("#fit").onclick = () => {
  zoom = 1;
  pan = { x: 0, y: 0 };
  render();
};
$<HTMLInputElement>("#assist").onchange = (e) => {
  assist = (e.target as HTMLInputElement).checked;
};
$<HTMLInputElement>("#mesh").onchange = (e) => {
  showMesh = (e.target as HTMLInputElement).checked;
  render();
};
$<HTMLInputElement>("#grid").onchange = (e) => {
  showGrid = (e.target as HTMLInputElement).checked;
  render();
};
function syncGridControls() {
  $<HTMLSelectElement>("#grid-mode").value = gridSettings.mode;
  $("#shared-grid").hidden = gridSettings.mode !== "shared";
  $("#independent-grid").hidden = gridSettings.mode !== "independent";
  for (const key of ["shared", "x", "y"] as const) {
    $<HTMLInputElement>(`#grid-${key}`).value = String(gridSettings[key]);
  }
  $("#grid-note").textContent =
    gridSettings.mode === "none"
      ? "グリッド表示・グリッドへのスナップは無効。頂点・平行のアシストは使えます。"
      : gridSettings.mode === "independent"
        ? "中央360×360領域が基準。0にした軸はグリッドなし。"
        : "中央360×360領域を分割。24分割＝15px。";
  $<HTMLInputElement>("#grid").disabled = gridSettings.mode === "none";
}
function saveGrid() {
  try {
    localStorage.setItem(gridStorageKey, JSON.stringify(gridSettings));
  } catch {
    status("グリッド設定を保存できませんでした。");
  }
  lastSnap = null;
  syncGridControls();
  render();
}
$<HTMLSelectElement>("#grid-mode").onchange = (e) => {
  gridSettings.mode = (e.target as HTMLSelectElement).value as GridSettings["mode"];
  saveGrid();
};
for (const key of ["shared", "x", "y"] as const) {
  $<HTMLInputElement>(`#grid-${key}`).onchange = (e) => {
    const input = e.target as HTMLInputElement,
      value = Number(input.value);
    if (!input.value.trim() || !validDivisions(value, key !== "shared")) {
      status("分割数は1〜360の整数で指定してください。縦横別では0でその軸を無効にできます。");
      syncGridControls();
      return;
    }
    gridSettings[key] = value;
    saveGrid();
  };
}
syncGridControls();
$<HTMLInputElement>("#global-width").onchange = (e) => {
  const n = Number((e.target as HTMLInputElement).value);
  if ((e.target as HTMLInputElement).value.trim() && Number.isFinite(n) && n >= 0 && n <= 32) {
    defaultWidth = n;
    try {
      localStorage.setItem(widthStorageKey, String(n));
    } catch {
      status("共通の太さを保存できませんでした。");
    }
  } else {
    (e.target as HTMLInputElement).value = String(defaultWidth);
  }
};
$("#apply-width").onclick = () =>
  mutate(
    () =>
      drawing.points.forEach((p) => {
        p.width = defaultWidth;
      }),
    "すべての点の太さを変更しました。",
  );
for (const [id, empty] of [
  ["sample", false],
  ["blank", true],
] as const) {
  $("#" + id).onclick = () => {
    const before = JSON.stringify(drawing);
    drawing = preset(empty);
    selected = [];
    selectedFace = null;
    selectedEdge = null;
    pushHistory(before);
    refresh();
    status("新しい構成に切り替えました。Undoで戻せます。");
  };
}
function download(name: string, content: string, type: string) {
  const url = URL.createObjectURL(new Blob([content], { type }));
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
$("#save").onclick = () =>
  download("stillwind-shape.json", JSON.stringify(drawing, null, 2), "application/json");
$("#export").onclick = () => {
  try {
    download("stillwind-logo.svg", exportSvg(drawing), "image/svg+xml");
    status("共有点と太さからSVGを書き出しました。");
  } catch (e) {
    status((e as Error).message);
  }
};
$("#load").onclick = () => $<HTMLInputElement>("#import-file").click();
$<HTMLInputElement>("#import-file").onchange = async (e) => {
  const input = e.target as HTMLInputElement,
    file = input.files?.[0];
  if (!file) {
    return;
  }
  try {
    if (file.size > 1_000_000) {
      throw new Error("JSONは1MB以下にしてください。");
    }
    const next = decode(await file.text()),
      before = JSON.stringify(drawing);
    drawing = next;
    selected = [];
    selectedFace = null;
    selectedEdge = null;
    pushHistory(before);
    refresh();
    status("JSONを読み込みました。");
  } catch (e) {
    status((e as Error).message);
  } finally {
    input.value = "";
  }
};
window.onkeydown = (e) => {
  if ((e.target as HTMLElement).closest("input, select, textarea")) {
    return;
  }
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "z") {
    e.preventDefault();
    travel(!e.shiftKey);
    return;
  }
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "y") {
    e.preventDefault();
    travel(false);
    return;
  }
  if (e.code === "Space") {
    e.preventDefault();
    space = true;
    canvas.style.cursor = "grab";
    return;
  }
  if (e.key === "Escape") {
    if (drag) {
      finishDrag(true);
    }
    selected = [];
    selectedFace = null;
    selectedEdge = null;
    refresh();
  }
  if (e.key === "Delete" || e.key === "Backspace") {
    e.preventDefault();
    removeSelected();
  }
  const modes: Record<string, Mode> = { v: "select", p: "point", l: "connect", f: "face" };
  if (!e.ctrlKey && !e.metaKey && modes[e.key.toLowerCase()]) {
    setMode(modes[e.key.toLowerCase()]);
  }
};
window.onkeyup = (e) => {
  if (e.code === "Space") {
    space = false;
    canvas.style.cursor = "default";
  }
};
window.onblur = () => {
  space = false;
  if (drag) {
    finishDrag(true);
  }
};
new ResizeObserver((entries) => {
  width = entries[0].contentRect.width;
  height = entries[0].contentRect.height;
  const ratio = devicePixelRatio || 1;
  canvas.width = Math.round(width * ratio);
  canvas.height = Math.round(height * ratio);
  ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
  render();
}).observe(canvas.parentElement!);
status(restoreMessage);
refresh();
