import { expect, test } from "vite-plus/test";

import { createFragmentRuntime } from "./fragment.ts";

test("style value conversion matches merge serialization", () => {
  for (const styleFormat of ["object", "string"] as const) {
    for (const classProperty of ["class", "className"]) {
      const runtime = createFragmentRuntime({ classProperty, styleFormat });
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
          runtime.stillwindMerge({ [runtime.stillwindStyleKey]: value }).style,
        );
      }
    }
  }
});
