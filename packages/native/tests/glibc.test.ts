import { expect, test } from "vite-plus/test";

import { checkGlibc, newestGlibc } from "../check-glibc.ts";

const symbol = (version: string) =>
  `0000000000000000      DF *UND*  0000000000000000 (GLIBC_${version}) memcpy\n`;

test("compares glibc versions numerically", () => {
  expect(newestGlibc(symbol("2.3.4") + symbol("2.9") + symbol("2.16"))).toBe("2.16");
});

test("accepts the glibc limit and rejects newer versions", () => {
  expect(() => checkGlibc(symbol("2.17"))).not.toThrow();
  expect(() => checkGlibc(symbol("2.2.5") + symbol("2.35"))).toThrow("requires glibc 2.35");
});

test("rejects output without glibc versions", () => {
  expect(() => checkGlibc("no symbols")).toThrow("no glibc symbol versions");
});
