// Loaded through `NODE_OPTIONS=--import` into every Node process that a measured command starts.
// It aggregates `stillwind:host:phase` messages and resource usage, and writes them to
// `$STILLWIND_BENCH_TRACE_DIR/<pid>-<thread>.json`. Development servers are stopped
// without running exit handlers, so the file is also rewritten periodically.

import { subscribe } from "node:diagnostics_channel";
import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { threadId } from "node:worker_threads";

const directory = process.env.STILLWIND_BENCH_TRACE_DIR;

if (directory) {
  /** @type {Map<string, { calls: number, items: number, total: number, max: number }>} */
  const phases = new Map();
  subscribe("stillwind:host:phase", (message) => {
    const { phase, duration, items } =
      /** @type {import("../../packages/host/src/compilation/trace.ts").PhaseMessage} */ (message);
    let entry = phases.get(phase);
    if (!entry) {
      phases.set(phase, (entry = { calls: 0, items: 0, total: 0, max: 0 }));
    }
    entry.calls += 1;
    entry.items += items;
    entry.total += duration;
    entry.max = Math.max(entry.max, duration);
  });
  const file = join(directory, `${process.pid}-${threadId}.json`);
  const write = () => {
    const usage = process.resourceUsage();
    writeFileSync(
      file,
      JSON.stringify({
        pid: process.pid,
        threadId,
        argv: process.argv.slice(1),
        maxRssKb: usage.maxRSS,
        cpuUserMs: usage.userCPUTime / 1000,
        cpuSystemMs: usage.systemCPUTime / 1000,
        phases: Object.fromEntries(phases),
      }),
    );
  };
  setInterval(write, 500).unref();
  process.on("exit", write);
}
