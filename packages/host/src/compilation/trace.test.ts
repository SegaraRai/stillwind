import { subscribe, unsubscribe } from "node:diagnostics_channel";

import { expect, test } from "vite-plus/test";

import { tracePhase, type PhaseMessage } from "./trace.ts";

test("phases are published after synchronous, asynchronous, and failed operations", async () => {
  const messages: PhaseMessage[] = [];
  const listener = (message: unknown) => messages.push(message as PhaseMessage);
  subscribe("stillwind:host:phase", listener);
  try {
    expect(tracePhase("sync", 2, () => "value")).toBe("value");
    await expect(tracePhase("async", 3, async () => "value")).resolves.toBe("value");
    expect(() =>
      tracePhase("failed", 1, () => {
        throw new Error("failure");
      }),
    ).toThrow("failure");
  } finally {
    unsubscribe("stillwind:host:phase", listener);
  }
  expect(messages.map(({ phase, items }) => [phase, items])).toEqual([
    ["sync", 2],
    ["async", 3],
    ["failed", 1],
  ]);
  expect(messages.every(({ duration }) => duration >= 0)).toBe(true);
});
