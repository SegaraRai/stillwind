import { defineConfig } from "vite-plus";

export default defineConfig(({ mode }) => ({
  test: {
    include: ["source-map/*.test.ts"],
    benchmark: {
      include: ["**/*.bench.ts"],
      retainSamples: true,
    },
    environment: "node",
    setupFiles: mode === "benchmark" ? ["./setup.ts"] : [],
    testTimeout: mode === "benchmark" ? 600_000 : undefined,
    experimental: {
      viteModuleRunner: mode !== "benchmark",
    },
  },
  run: {
    tasks: {
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
    },
  },
}));
