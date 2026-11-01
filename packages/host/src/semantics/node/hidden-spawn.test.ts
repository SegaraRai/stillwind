import childProcess, { spawn } from "node:child_process";
import { syncBuiltinESMExports } from "node:module";

import { expect, onTestFinished, test, vi } from "vite-plus/test";

import { withHiddenSpawnWindows } from "./hidden-spawn.ts";

/**
 * Replaces `spawn` for ESM and default-export callers and restores it after the test.
 *
 * @returns The mock that records calls.
 */
function mockSpawn() {
  const original = childProcess.spawn;
  const mock = vi.fn();
  childProcess.spawn = mock as unknown as typeof spawn;
  syncBuiltinESMExports();
  onTestFinished(() => {
    childProcess.spawn = original;
    syncBuiltinESMExports();
  });
  return mock;
}

test("hides windows for named ESM imports of spawn and keeps other options", () => {
  const mock = mockSpawn();
  withHiddenSpawnWindows(() =>
    spawn("tsc", ["--api"], {
      stdio: "ignore",
    }),
  );
  expect(mock).toHaveBeenCalledWith("tsc", ["--api"], {
    stdio: "ignore",
    windowsHide: true,
  });
});

test("restores spawn after the function throws", () => {
  const mock = mockSpawn();
  expect(() =>
    withHiddenSpawnWindows(() => {
      throw new Error("start failed");
    }),
  ).toThrow("start failed");
  expect(spawn).toBe(mock);
});
