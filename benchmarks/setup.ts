import { execFileSync } from "node:child_process";

import { beforeEach, inject, type BenchRunOptions } from "vite-plus/test";

declare module "vite-plus/test" {
  interface ProvidedContext {
    benchmarkWasm: boolean;
  }

  interface TaskMeta {
    environment?: {
      timestamp: string;
      revision: string;
      node: string;
      platform: string;
      arch: string;
    };
    measurement?: Pick<BenchRunOptions, "iterations" | "time" | "warmupIterations" | "warmupTime">;
    canonicalBrowser?: {
      userAgent: string;
      measurements: {
        scenario: string;
        samples: number[];
      }[];
    };
    fingerprint?: string;
    notes?: string;
    rules?: number;
  }
}

if (inject("benchmarkWasm")) {
  process.env.NAPI_RS_FORCE_WASI = "error";
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
