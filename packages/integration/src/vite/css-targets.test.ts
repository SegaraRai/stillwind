import { expect, test } from "vite-plus/test";

import { inheritCssTarget, minifyTargets } from "./css-targets.ts";

test("CSS target inheritance accepts browser lists and keeps unknown constraints conservative", () => {
  const browsers = ["chrome120", "safari17.1.2", "ios17"];
  expect(inheritCssTarget(browsers)).toEqual(browsers);
  expect(inheritCssTarget(browsers)).not.toBe(browsers);
  expect(inheritCssTarget("firefox120")).toEqual(["firefox120"]);
  for (const target of [
    false,
    [],
    "esnext",
    "es2022",
    "node22",
    ["chrome120", "es2015"],
  ] as const) {
    expect(
      inheritCssTarget(target === false || typeof target === "string" ? target : [...target]),
    ).toEqual([]);
  }
});

test("minifier targets pack every version component and retain the minimum duplicate browser", () => {
  expect(
    minifyTargets(["chrome120", "chrome111.2.3", "chrome119", "ios17.1.2", "safari17"]),
  ).toEqual({
    chrome: 111 * 65536 + 2 * 256 + 3,
    ios_saf: 17 * 65536 + 256 + 2,
    safari: 17 * 65536,
  });
  expect(minifyTargets("samsung22")).toEqual({ samsung: 22 * 65536 });
  expect(minifyTargets([])).toBeUndefined();
  expect(minifyTargets(undefined)).toBeUndefined();
  for (const target of ["chrome256", "safari17.256", "ios17.1.256"]) {
    expect(() => minifyTargets(target)).toThrow("each version component must be 0..255");
  }
});
