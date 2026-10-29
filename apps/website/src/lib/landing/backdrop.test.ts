import { describe, expect, it } from "vite-plus/test";

import { STREAMLINE_COUNT, backdropState, streamlinePath } from "./backdrop";

function points(path: string): [number, number][] {
  const numbers = path.match(/-?[\d.]+(?:e-?\d+)?/g)!.map(Number);
  return Array.from({ length: numbers.length / 2 }, (_, i) => [
    numbers[2 * i]!,
    numbers[2 * i + 1]!,
  ]);
}

describe("backdropState", () => {
  it("keeps full wind at the top and calms it after the hero", () => {
    expect(backdropState(0, 900, 5000)).toMatchObject({
      wind: 1,
      gustOpacity: 1,
      opacity: 1,
      columnAlpha: 1,
      straightness: 0,
    });
    expect(backdropState(1200, 900, 5000)).toMatchObject({
      wind: 0,
      gustOpacity: 0,
      opacity: 0.5,
    });
  });

  it("straightens the lines at the end of the page", () => {
    expect(backdropState(5000, 900, 5000).straightness).toBe(1);
  });

  it("treats a page without scroll range as unscrolled", () => {
    expect(backdropState(0, 900, 0).straightness).toBe(0);
  });
});

describe("streamlinePath", () => {
  const frame = {
    width: 1440,
    height: 900,
    heroEnd: 820,
    spacing: 64,
    tilt: (-6 * Math.PI) / 180,
  };

  it("places each straight line on a distinct lattice row", () => {
    const rows = new Set<number>();
    for (let index = 0; index < STREAMLINE_COUNT; index++) {
      const distances = points(
        streamlinePath(index, {
          ...frame,
          straightness: 1,
        }),
      ).map(
        ([x, y]) =>
          (y - frame.height / 2) * Math.cos(frame.tilt) -
          (x - frame.width / 2) * Math.sin(frame.tilt),
      );
      // A row covers the pixel above its multiple of spacing from the top of the lattice layer.
      const row = (distances[0]! + 0.5 + frame.height) / frame.spacing;
      expect(row).toBeCloseTo(Math.round(row), 6);
      for (const distance of distances) {
        expect(distance).toBeCloseTo(distances[0]!, 6);
      }
      rows.add(Math.round(row));
    }
    expect(rows.size).toBe(STREAMLINE_COUNT);
  });

  it("draws curves that do not depend on the lattice", () => {
    expect(
      streamlinePath(4, {
        ...frame,
        straightness: 0,
      }),
    ).toBe(
      streamlinePath(4, {
        ...frame,
        tilt: 0,
        spacing: 48,
        straightness: 0,
      }),
    );
  });
});
