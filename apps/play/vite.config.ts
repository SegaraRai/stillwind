import { astroCache, testCache } from "@stillwind-internal/tools-workspace/task-cache";
import { version } from "stillwind/package.json" with { type: "json" };
import { defineConfig } from "vite-plus";

export default defineConfig({
  root: import.meta.dirname,
  define: {
    __STILLWIND_VERSION__: JSON.stringify(version),
  },
  test: {
    name: "play",
    include: ["src/**/*.test.ts"],
    testTimeout: 60_000,
  },
  run: {
    tasks: {
      dev: {
        command: "astro dev",
        cache: false,
        dependsOn: [
          "build:assets",
          "@stillwind/host#build",
          "@stillwind/integration#build",
          "prettier-plugin-stillwind#build",
        ],
      },
      build: {
        command: "astro build",
        cache: {
          ...astroCache,
          input: [
            ...astroCache.input,
            "!.astro",
            "!.astro/**",
            "!.nimbus",
            "!.nimbus/**",
            "!dist/site",
            "!dist/site/**",
            "!dist/acceptance",
            "!dist/acceptance/**",
          ],
          output: ["dist/site/**"],
        },
        dependsOn: [
          "build:assets",
          "generate:types",
          "@stillwind/host#build",
          "@stillwind/integration#build",
          "prettier-plugin-stillwind#build",
        ],
      },
      "build:acceptance": {
        command: "astro build --base /play/ --outDir dist/acceptance",
        cache: {
          ...astroCache,
          input: [
            ...astroCache.input,
            "!.astro",
            "!.astro/**",
            "!.nimbus",
            "!.nimbus/**",
            "!dist/site",
            "!dist/site/**",
            "!dist/acceptance",
            "!dist/acceptance/**",
          ],
          output: ["dist/acceptance/**"],
        },
        // Both Astro builds write .astro, so the production build must finish first.
        dependsOn: ["build"],
      },
      "build:assets": {
        command: "node build/assets.ts",
        cache: {
          input: [{ auto: true }, "!dist/**"],
          output: ["dist/assets-static/**"],
        },
        dependsOn: [
          "@stillwind/plugin-iconify#build",
          "@stillwind/vscode-extension#build",
          "stillwind#build",
        ],
      },
      "fmt:examples": {
        command: "node build/examples.ts fix",
        cache: false,
        dependsOn: ["@stillwind/cli#build", "@stillwind/plugin-iconify#build", "stillwind#build"],
      },
      "check:examples": {
        command: "node build/examples.ts check",
        dependsOn: ["@stillwind/cli#build", "@stillwind/plugin-iconify#build", "stillwind#build"],
      },
      "generate:types": {
        command: "astro sync",
        dependsOn: ["@stillwind/integration#build", "@stillwind/plugin-iconify#build"],
        cache: {
          ...astroCache,
          input: [
            ...astroCache.input,
            "!.astro",
            "!.astro/**",
            "!dist",
            "!dist/**",
            "!node_modules/.astro",
            "!node_modules/.astro/**",
          ],
          output: [".astro/**", "node_modules/.astro/**"],
        },
      },
      typecheck: {
        command: "astro check --noSync",
        cache: astroCache,
        dependsOn: [
          "@stillwind/host#build",
          "@stillwind/integration#build",
          "prettier-plugin-stillwind#build",
          "generate:types",
        ],
      },
      test: {
        command: "vp test --run",
        cache: testCache,
        dependsOn: ["@stillwind/host#build", "prettier-plugin-stillwind#build"],
      },
    },
  },
});
