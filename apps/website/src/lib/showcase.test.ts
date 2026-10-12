import { expect, it } from "vite-plus/test";

import { showcasePosition, showcaseScenes } from "./showcase";

it("keeps normalization and sorting in one scene with three distinct source labels", () => {
  const normalize = showcasePosition(2.5 / 5);
  expect(normalize).toMatchObject({ from: 2, to: 3, scene: 2, diagnostic: 0, refine: 1, sort: 0 });
  expect(normalize.opacity[2]).toBe(1);
  const format = showcasePosition(3.5 / 5);
  expect(format).toMatchObject({ from: 3, to: 4, scene: 2, refine: 1, sort: 0.5, compile: 0 });
  expect(format.opacity[2]).toBe(1);
  expect([2, 3, 4].map((state) => showcasePosition(state / 5).labels)).toEqual([
    [1, 0, 0, 0],
    [0, 1, 0, 0],
    [0, 0, 1, 0],
  ]);
});

it("navigates to each scene's first complete state and compiles the sorted result", () => {
  const destinations = Array.from(
    { length: 4 },
    (_, scene) =>
      showcaseScenes.findIndex((stateScene) => stateScene === scene) / (showcaseScenes.length - 1),
  );
  expect(destinations.map((progress) => showcasePosition(progress).scene)).toEqual([0, 1, 2, 3]);
  expect(showcasePosition(destinations[2]!)).toMatchObject({ active: 2, diagnostic: 1, sort: 0 });
  expect(showcasePosition(4 / 5)).toMatchObject({ active: 4, sort: 1, compile: 0 });
  expect(showcasePosition(1)).toMatchObject({
    active: 5,
    scene: 3,
    sort: 1,
    compile: 1,
    labels: [0, 0, 0, 1],
  });
});
