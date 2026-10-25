import { execFileSync } from "node:child_process";

import { beforeEach, inject, type BenchRunOptions } from "vite-plus/test";

declare module "vite-plus/test" {
  interface ProvidedContext {
    benchmarkWasm: boolean;
  }

  interface TaskMeta {
    /** Browser analysis timings. Samples use milliseconds and exclude transport and UI work. */
    analysisBrowser?: {
      /** Browser version used for the measurements. */
      userAgent: string;
      /** Whether the workload queries ref unions using the published Stillwind declarations. */
      typeFacts: boolean;
      /** Timings grouped by operation, in measurement order. */
      measurements: {
        operation: string;
        /** Elapsed milliseconds for each run. */
        samples: number[];
      }[];
    };
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
