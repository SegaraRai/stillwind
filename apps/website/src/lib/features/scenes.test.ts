import { expect, it } from "vite-plus/test";

import { featurePosition, featureScenes, featureScrollLength } from "./scenes";

it("gives every demonstration its complete local range while its source stays readable", () => {
  featureScenes.forEach((stop) => {
    [0, 0.5, 1].forEach((local) => {
      const progress = (stop.start + (stop.end - stop.start) * local) / featureScrollLength;
      const position = featurePosition(progress);
      expect(position.scene).toBe(stop.scene);
      expect(position.frame).toBe(stop.frame);
      expect(position.opacity[stop.scene]).toBeCloseTo(1);
      expect(position.local).toBeCloseTo(local);
    });
  });
});

it("holds demonstrations at their endpoints during source changes and resets the next example", () => {
  featureScenes.slice(0, -1).forEach((stop, index) => {
    const next = featureScenes[index + 1]!;
    const gap = next.start - stop.end;
    const outgoing = featurePosition((stop.end + gap * 0.25) / featureScrollLength);
    const incoming = featurePosition((stop.end + gap * 0.75) / featureScrollLength);
    expect(outgoing.active).toBe(index);
    expect(outgoing.local).toBe(1);
    expect(incoming.active).toBe(index + 1);
    expect(incoming.local).toBe(0);
    const middle = featurePosition((stop.end + gap * 0.5) / featureScrollLength);
    expect(middle.opacity[stop.scene]).toBeCloseTo(stop.scene === 0 && next.scene === 0 ? 1 : 0);
  });
});

it("clamps progress before the first example and after the last example", () => {
  expect(featurePosition(-1)).toEqual(featurePosition(0));
  expect(featurePosition(2)).toEqual(featurePosition(1));
});
