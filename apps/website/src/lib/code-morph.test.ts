import { expect, it } from "vite-plus/test";

import { morphPosition, styleParts } from "./code-morph";

it("keeps utility names and arbitrary values intact, splitting only colons and spaces", () => {
  const parts = styleParts(
    "rounded-md hover:min-[713px]:px-[16px] bg-[#d9e2f0]",
    ["shape", "inline", "color"],
    ["inline", "color"],
  );
  expect(parts.map((part) => part.text)).toEqual([
    'style("',
    "rounded-md",
    " ",
    "hover",
    ":",
    "min-[713px]",
    ":",
    "px-[16px]",
    " ",
    "bg-[#d9e2f0]",
    '")',
  ]);
  expect(parts.map((part) => part.text).join("")).toBe(
    'style("rounded-md hover:min-[713px]:px-[16px] bg-[#d9e2f0]")',
  );
  expect(parts.filter((part) => part.finding).map((part) => part.text)).toEqual([
    "hover",
    ":",
    "min-[713px]",
    ":",
    "px-[16px]",
    "bg-[#d9e2f0]",
  ]);
  expect(
    parts
      .filter((part) => part.utility === "inline")
      .map((part) => part.text)
      .join(""),
  ).toBe("hover:min-[713px]:px-[16px]");
});

it("keeps a utility key when its variants or source order change", () => {
  const before = styleParts("rounded-md px-4", ["shape", "inline"]);
  const after = styleParts("hover:px-6 rounded-md", ["inline", "shape"]);
  expect(before.find((part) => part.key === "inline")?.text).toBe("px-4");
  expect(after.find((part) => part.key === "inline")?.text).toBe("px-6");
  expect(after.find((part) => part.key === "shape")?.text).toBe("rounded-md");
  expect(new Set(after.map((part) => part.key)).size).toBe(after.length);
  expect(() => styleParts("px-4 py-2", ["inline"])).toThrow();
});

it("maps five scene stops and holds the first three code states at the same positions", () => {
  for (let index = 0; index < 5; index++) {
    expect(morphPosition(index / 4, 5).active).toBe(index);
    expect(morphPosition(index / 4, 5).blend).toBe(index === 4 ? 1 : 0);
  }
  expect(morphPosition(-1, 5)).toMatchObject({ from: 0, blend: 0, active: 0 });
  expect(morphPosition(2, 5)).toMatchObject({ from: 3, blend: 1, active: 4 });
  for (const progress of [0.0625, 0.125, 0.1875, 0.3125, 0.375, 0.4375]) {
    expect(morphPosition(progress * 2, 3).blend).toBe(morphPosition(progress, 5).blend);
  }
  expect(morphPosition(0.625, 5)).toMatchObject({ from: 2, to: 3, blend: 0.5 });
  expect(morphPosition(0.875, 5)).toMatchObject({ from: 3, to: 4, blend: 0.5 });
  expect(() => morphPosition(0, 1)).toThrow(RangeError);
});

it("keeps complete scenes stable around navigation points and moves between them", () => {
  for (let index = 0; index < 5; index++) {
    for (const offset of [-0.07, 0, 0.07]) {
      const progress = Math.max(0, Math.min(1, index / 4 + offset));
      const position = morphPosition(progress, 5);
      expect(position.active).toBe(index);
      expect(position.blend).toBe(position.from === index ? 0 : 1);
    }
  }
  const before = morphPosition(0.1, 5);
  const middle = morphPosition(0.125, 5);
  const after = morphPosition(0.15, 5);
  expect(before.blend).toBeGreaterThan(0);
  expect(before.blend).toBeLessThan(middle.blend);
  expect(middle.blend).toBeCloseTo(0.5);
  expect(after.blend).toBeGreaterThan(middle.blend);
  expect(after.blend).toBeLessThan(1);
});
