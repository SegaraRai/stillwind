import { expect, test } from "vite-plus/test";

import { startNode } from "./process.ts";

test("stop resolves after the subprocess has closed", async () => {
  const runner = startNode(
    ["-e", "console.log('ready'); setInterval(() => {}, 1000)"],
    import.meta.dirname,
  );
  let closed = false;
  void runner.done.then(() => {
    closed = true;
  });
  try {
    await expect
      .poll(() => runner.logs(), {
        timeout: 5_000,
      })
      .toContain("ready");
    await runner.stop();
    expect(closed).toBe(true);
  } finally {
    await runner.stop();
  }
}, 20_000);
