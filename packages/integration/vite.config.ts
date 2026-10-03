import { testCache } from "@stillwind-tools/workspace/task-cache";
import { defineConfig } from "vite-plus";

export default defineConfig({
  root: import.meta.dirname,
  pack: {
    entry: [
      "src/astro/index.ts",
      "src/astro/middleware.ts",
      "src/next/css-loader.ts",
      "src/next/index.ts",
      "src/nuxt/index.ts",
      "src/sveltekit/handle.ts",
      "src/rsbuild/index.ts",
      "src/rspack/index.ts",
      "src/turbopack/css-loader.ts",
      "src/turbopack/index.ts",
      "src/turbopack/loader.ts",
      "src/vite/index.ts",
      "src/vite/runtime-source-map.ts",
      "src/vite/runtime.ts",
      "src/webpack/css-loader.ts",
      "src/webpack/css-map-loader.ts",
      "src/webpack/fragment-loader.ts",
      "src/webpack/index.ts",
      "src/webpack/loader.ts",
      "src/webpack/source-loader.ts",
      "src/webpack/source-map-loader.ts",
      "src/webpack/stillwind-fragment.ts",
    ],
    format: ["esm"],
    fixedExtension: false,
    dts: true,
    deps: {
      neverBundle: ["webpack", "astro:middleware", "virtual:stillwind/astro-styles", "$app/paths"],
      alwaysBundle: [
        "@jridgewell/gen-mapping",
        "@jridgewell/trace-mapping",
        "@jridgewell/sourcemap-codec",
        "@jridgewell/resolve-uri",
      ],
    },
    sourcemap: true,
    copy: [
      {
        from: "src/turbopack/stillwind-turbopack.css",
        to: "dist/turbopack",
      },
      {
        from: "src/webpack/stillwind-webpack.css",
        to: "dist/webpack",
      },
    ],
  },
  test: {
    name: "integration",
    include: ["src/**/*.test.ts", "tests/**/*.test.ts"],
    fileParallelism: false,
    testTimeout: 120_000,
    server: {
      deps: {
        // Rspack loads built loaders through Node; the plugin must share their module state.
        external: [/\/packages\/integration\/dist\//],
      },
    },
  },
  run: {
    tasks: {
      "dev:frameworks": {
        command: "node tests/vite/framework-dev.ts",
        cache: false,
        dependsOn: ["typecheck"],
      },
      build: {
        command: "vp pack",
        dependsOn: ["@stillwind/host#build", "stillwind#build"],
      },
      typecheck: {
        command: "tsc --noEmit",
        dependsOn: ["build"],
      },
      test: {
        command: "vp test --run",
        cache: testCache,
        dependsOn: ["build"],
      },
      "test:vite": {
        command: "vp test --run tests/vite",
        cache: testCache,
        dependsOn: ["build"],
      },
      "test:bundlers": {
        command: "vp test --run tests/webpack",
        cache: testCache,
        dependsOn: ["build"],
      },
      "test:frameworks": {
        command: "vp test --run tests/vite/framework-builds.test.ts",
        cache: testCache,
        dependsOn: ["build"],
      },
    },
  },
});
