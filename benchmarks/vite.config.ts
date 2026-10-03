import { defineConfig } from "vite-plus";

export default defineConfig(({ mode }) => ({
  test: {
    include: ["source-map/*.test.ts", "research/*.test.ts", "performance/*.test.ts"],
    benchmark: {
      include: ["**/*.bench.ts"],
      retainSamples: true,
    },
    environment: "node",
    provide: {
      performanceFull: mode === "performance-full",
    },
    setupFiles: mode === "benchmark" || mode.startsWith("performance") ? ["./setup.ts"] : [],
    testTimeout: (
      {
        benchmark: 600_000,
        "performance-full": 120_000,
        performance: 30_000,
      } as Record<string, number | undefined>
    )[mode],
    experimental: {
      viteModuleRunner: mode !== "benchmark" && !mode.startsWith("performance"),
    },
  },
  run: {
    tasks: {
      performance: {
        command:
          "vp test bench performance/performance.bench.ts --run --mode performance --reporter default --reporter json --outputFile results/performance.json",
        dependsOn: ["@stillwind/host#build"],
        cache: false,
      },
      "performance:full": {
        command:
          "vp test bench performance/performance.bench.ts --run --mode performance-full --reporter default --reporter json --outputFile results/performance-full.json",
        dependsOn: ["@stillwind/host#build"],
        cache: false,
      },
      test: {
        command: "vp test --run",
        cache: false,
      },
      typecheck: {
        command: "tsc --noEmit",
        dependsOn: ["@stillwind/host#build"],
      },
      compiler: {
        command:
          "vp test bench compiler/compiler.bench.ts --run --mode benchmark --reporter default --reporter json --outputFile results/compiler.json",
        dependsOn: ["@stillwind/native#build"],
        cache: false,
      },
      host: {
        command:
          "vp test bench compiler/host.bench.ts --run --mode benchmark --reporter default --reporter json --outputFile results/host.json",
        dependsOn: ["@stillwind/host#build"],
        cache: false,
      },
      "source-map": {
        command:
          "vp test bench source-map/source-map.bench.ts --run --mode benchmark --reporter default --reporter json --outputFile results/source-map.json",
        dependsOn: ["@stillwind/host#build"],
        cache: false,
      },
      research: {
        command:
          "vp test bench research/investigation.bench.ts --run --mode benchmark --reporter default --reporter json --outputFile results/research.json",
        dependsOn: ["@stillwind/host#build"],
        cache: false,
      },
      architecture: {
        command:
          "vp test bench research/architecture.bench.ts --run --mode benchmark --reporter default --reporter json --outputFile results/architecture.json",
        dependsOn: ["@stillwind/host#build"],
        cache: false,
      },
    },
  },
}));
