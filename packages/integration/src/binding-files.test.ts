import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { __napiBindingTarget } from "@stillwind/native";
import { expect, test } from "vite-plus/test";

import { nativeBindingFiles } from "./binding-files.ts";

test("binding watch files include the generated entry and local or packaged binaries", () => {
  const files = nativeBindingFiles();
  expect(files).toContain(fileURLToPath(import.meta.resolve("@stillwind/native")));
  const extension = __napiBindingTarget === "native" ? ".node" : ".wasm";
  expect(files.some((file) => file.endsWith(extension))).toBe(true);
  expect(files.every(existsSync)).toBe(true);
  expect(new Set(files).size).toBe(files.length);
});
