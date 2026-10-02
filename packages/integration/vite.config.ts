import { defineConfig } from "vite-plus";

export default defineConfig({
  test: {
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
  run: {
    tasks: {
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
        dependsOn: ["build"],
        cache: false,
      },
      "test:vite": {
        command: "vp test --run tests/vite",
        dependsOn: ["build"],
        cache: false,
      },
      "test:webpack": {
        command: "vp test --run tests/webpack",
        dependsOn: ["build"],
        cache: false,
      },
      "test:frameworks": {
        command: "vp test --run tests/vite/framework-builds.test.ts",
        dependsOn: ["build"],
        cache: false,
      },
      "dev:frameworks": {
        command: "node tests/vite/framework-dev.ts",
        dependsOn: ["typecheck"],
        cache: false,
      },
    },
  },
});
