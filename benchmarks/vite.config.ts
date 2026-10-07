import { testCache } from "@stillwind-internal/tools-workspace/task-cache";
import { defineConfig } from "vite-plus";

export default defineConfig(({ mode }) => ({
  root: import.meta.dirname,
  test: {
    name: "benchmarks",
    include: ["source-map/*.test.ts", "research/*.test.ts", "performance/*.test.ts"],
    benchmark: {
      include: ["**/*.bench.ts"],
      retainSamples: true,
    },
    environment: "node",
    execArgv:
      mode.startsWith("benchmark") || mode.startsWith("performance")
        ? ["--experimental-transform-types"]
        : [],
    provide: {
      performanceFull: mode === "performance-full",
      benchmarkWasm: mode === "benchmark-wasm",
    },
    setupFiles:
      mode.startsWith("benchmark") || mode.startsWith("performance") ? ["./setup.ts"] : [],
    testTimeout: (
      {
        benchmark: 600_000,
        "benchmark-wasm": 600_000,
        "performance-full": 120_000,
        performance: 30_000,
      } as Record<string, number | undefined>
    )[mode],
    experimental: {
      viteModuleRunner: !mode.startsWith("benchmark") && !mode.startsWith("performance"),
    },
  },
  run: {
    tasks: {
      "bench:architecture": {
        command:
          "vp test bench research/architecture.bench.ts --run --mode benchmark --reporter default --reporter json --outputFile results/architecture.json",
        cache: false,
        dependsOn: ["@stillwind/host#build", "@stillwind/native#build"],
      },
      "bench:compiler": {
        command:
          "vp test bench compiler/canonical.bench.ts compiler/compiler.bench.ts --run --mode benchmark --reporter default --reporter json --outputFile results/compiler.json",
        cache: false,
        dependsOn: ["@stillwind/native#build"],
      },
      "bench:host": {
        command:
          "vp test bench compiler/host.bench.ts --run --mode benchmark --reporter default --reporter json --outputFile results/host.json",
        cache: false,
        dependsOn: ["@stillwind/host#build", "@stillwind/native#build", "stillwind#build"],
      },
      "bench:compiler:browser": {
        command:
          "vp test --run --config vite.browser.config.ts --reporter default --reporter json --outputFile results/canonical-browser.json",
        cache: false,
        dependsOn: ["@stillwind/native#collect:local"],
      },
      "bench:compiler:wasm": {
        command:
          "vp test bench compiler/canonical.bench.ts --run --mode benchmark-wasm --reporter default --reporter json --outputFile results/canonical-wasm.json",
        cache: false,
        dependsOn: ["@stillwind/native#collect:local"],
      },
      "bench:sorting": {
        command:
          "vp test bench compiler/sorting.bench.ts --run --mode benchmark --reporter default --reporter json --outputFile results/sorting-native.json",
        cache: false,
        dependsOn: ["@stillwind/host#build", "@stillwind/native#build"],
      },
      "bench:sorting:wasm": {
        command:
          "vp test bench compiler/sorting.bench.ts --run --mode benchmark-wasm --reporter default --reporter json --outputFile results/sorting-wasm.json",
        cache: false,
        dependsOn: ["@stillwind/host#build", "@stillwind/native#collect:local"],
      },
      "bench:sorting:profile": {
        command:
          "vp test bench compiler/sorting-profile.bench.ts --run --mode benchmark --reporter default --reporter json --outputFile results/sorting-profile.json",
        cache: false,
        dependsOn: ["@stillwind/host#build", "@stillwind/native#build"],
      },
      "bench:research": {
        command:
          "vp test bench research/investigation.bench.ts --run --mode benchmark --reporter default --reporter json --outputFile results/research.json",
        cache: false,
        dependsOn: ["@stillwind/host#build", "@stillwind/native#build", "stillwind#build"],
      },
      "bench:source-map": {
        command:
          "vp test bench source-map/source-map.bench.ts --run --mode benchmark --reporter default --reporter json --outputFile results/source-map.json",
        cache: false,
        dependsOn: ["@stillwind/host#build"],
      },
      typecheck: {
        command: "tsc --noEmit",
        dependsOn: ["@stillwind/host#build", "@stillwind/native#build", "stillwind#build"],
      },
      test: {
        command: "vp test --run",
        cache: testCache,
        dependsOn: ["@stillwind/host#build", "@stillwind/native#build"],
      },
      "test:performance": {
        command:
          "vp test bench performance/performance.bench.ts --run --mode performance --reporter default --reporter json --outputFile results/performance.json",
        cache: false,
        dependsOn: ["@stillwind/host#build", "@stillwind/native#build"],
      },
      "test:performance:full": {
        command:
          "vp test bench performance/performance.bench.ts --run --mode performance-full --reporter default --reporter json --outputFile results/performance-full.json",
        cache: false,
        dependsOn: ["@stillwind/host#build", "@stillwind/native#build"],
      },
    },
  },
}));
