import type { GridSpacing } from "./geometry.ts";

export interface GridSettings {
  mode: "shared" | "independent" | "none";
  shared: number;
  x: number;
  y: number;
}

export function defaultGrid(): GridSettings {
  return { mode: "shared", shared: 24, x: 24, y: 24 };
}

export function gridSpacing(settings: GridSettings): GridSpacing {
  if (settings.mode === "none") {
    return { x: 0, y: 0, origin: 60, extent: 360 };
  }
  if (settings.mode === "shared") {
    return { x: 360 / settings.shared, y: 360 / settings.shared, origin: 60, extent: 360 };
  }
  return {
    x: settings.x ? 360 / settings.x : 0,
    y: settings.y ? 360 / settings.y : 0,
    origin: 60,
    extent: 360,
  };
}

export function validDivisions(value: number, allowZero: boolean): boolean {
  return Number.isInteger(value) && ((allowZero && value === 0) || (value >= 1 && value <= 360));
}

export function decodeGrid(text: string): GridSettings {
  const value = JSON.parse(text) as GridSettings;
  if (
    !value ||
    !["shared", "independent", "none"].includes(value.mode) ||
    !validDivisions(value.shared, false) ||
    !validDivisions(value.x, true) ||
    !validDivisions(value.y, true)
  ) {
    throw new Error("グリッド設定が不正です。");
  }
  return { mode: value.mode, shared: value.shared, x: value.x, y: value.y };
}
