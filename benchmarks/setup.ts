import { execFileSync } from "node:child_process";

import { beforeEach, type BenchRunOptions } from "vite-plus/test";

declare module "vite-plus/test" {
  interface TaskMeta {
    environment?: {
      timestamp: string;
      revision: string;
      node: string;
      platform: string;
      arch: string;
    };
    measurement?: Pick<BenchRunOptions, "iterations" | "time" | "warmupIterations" | "warmupTime">;
    fingerprint?: string;
    notes?: string;
    rules?: number;
  }
}

const revision = execFileSync("git", ["rev-parse", "HEAD"], { encoding: "utf8" }).trim();

beforeEach(({ task }) => {
  task.meta.environment = {
    timestamp: new Date().toISOString(),
    revision,
    node: process.version,
    platform: process.platform,
    arch: process.arch,
  };
});
