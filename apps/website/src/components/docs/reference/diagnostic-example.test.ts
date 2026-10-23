import { expect, test } from "vite-plus/test";

import { diagnosticExample } from "./diagnostic-example.ts";

test("circular utility applications produce the documented diagnostic", async () => {
  const example = await diagnosticExample("SW14008");

  expect(example.diagnostics).toEqual([
    expect.objectContaining({
      code: "SW14008",
      message: expect.stringContaining("circular @apply dependency"),
    }),
  ]);
});
