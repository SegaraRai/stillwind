import { expect, test } from "vite-plus/test";

import * as react from "./class-name-object.ts";
import * as vue from "./class-object.ts";
import * as svelte from "./class-string.ts";

test("style value conversion matches merge serialization", () => {
  for (const runtime of [react, svelte, vue]) {
    for (const value of [
      undefined,
      null,
      false,
      true,
      0,
      1,
      "",
      "a b",
      {},
      { "--x": 1 },
      ["a"],
      Symbol("value"),
    ]) {
      expect(runtime.stillwindStyleValue(value)).toStrictEqual(
        runtime.stillwindMerge({
          [runtime.stillwindStyleKey]: value,
        }).style,
      );
    }
  }
});
