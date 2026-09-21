import { defineConfig } from "vite-plus";

export default defineConfig({
  pack: {
    entry: [
      "src/vite/index.ts",
      "src/astro/index.ts",
      "src/webpack/index.ts",
      "src/rspack/index.ts",
      "src/rsbuild/index.ts",
      "src/next/index.ts",
      "src/webpack/loader.ts",
      "src/webpack/css-loader.ts",
      "src/webpack/css-map-loader.ts",
      "src/webpack/source-map-loader.ts",
      "src/webpack/fragment-loader.ts",
      "src/webpack/stillwind-fragment.ts",
      "src/turbopack/index.ts",
      "src/turbopack/loader.ts",
      "src/turbopack/css-loader.ts",
      "src/next/css-loader.ts",
    ],
    format: ["esm"],
    fixedExtension: false,
    dts: true,
    deps: { neverBundle: ["webpack"] },
    sourcemap: true,
    copy: [
      { from: "src/turbopack/stillwind-turbopack.css", to: "dist/turbopack" },
      { from: "src/webpack/stillwind-webpack.css", to: "dist/webpack" },
    ],
  },
  run: {
    tasks: {
      build: {
        command: "vp pack",
        dependsOn: ["@stillwind/host#build", "stillwind#build"],
      },
      typecheck: { command: "tsc --noEmit", dependsOn: ["build"] },
      test: {
        command: "node --test test/vite/*.test.mts test/webpack/*.test.mts test/*.test.mts",
        dependsOn: ["typecheck"],
        cache: false,
      },
      "test:vite": {
        command: "node --test test/vite/*.test.mts",
        dependsOn: ["typecheck"],
        cache: false,
      },
      "test:webpack": {
        command: "node --test test/webpack/*.test.mts",
        dependsOn: ["typecheck"],
        cache: false,
      },
      "test:frameworks": {
        command: "node --test test/vite/framework-builds.mts",
        dependsOn: ["typecheck"],
        cache: false,
      },
      "dev:frameworks": {
        command: "node test/vite/framework-dev.mts",
        dependsOn: ["typecheck"],
        cache: false,
      },
    },
  },
});
