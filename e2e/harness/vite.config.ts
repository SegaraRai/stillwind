import { testCache } from "@stillwind-internal/tools-workspace/task-cache";
import { defineConfig } from "vite-plus";

import { fixtureTypechecks } from "./src/config.ts";

export default defineConfig({
  root: import.meta.dirname,
  test: {
    name: "e2e-harness",
    include: ["src/**/*.test.ts"],
    exclude: ["src/wasm-browser.test.ts", "src/language-server-browser.test.ts"],
  },
  run: {
    tasks: {
      typecheck: {
        command: "tsc --noEmit",
        dependsOn: ["@stillwind/native#build", "@stillwind/language-server#build"],
      },
      "typecheck:fixtures": {
        command: "",
        dependsOn: fixtureTypechecks,
      },
      test: {
        command: "vp test --run",
        cache: testCache,
      },
      "test:wasm": {
        command: "vp test --run --config vite.browser.config.ts",
        cache: false,
        dependsOn: ["@stillwind/native#collect:local", "@stillwind/language-server#build"],
      },
      "test:e2e": {
        command: "node src/matrix.ts",
        cache: false,
        dependsOn: [
          "@stillwind-internal/e2e-sveltekit3-svelte5#generate:types",
          "@stillwind/integration#build",
          "@stillwind/native#collect:local",
          "@stillwind/plugin-iconify#build",
          "stillwind#build",
        ],
      },
      "test:e2e:compatibility": {
        command: "node src/matrix.ts --support compatibility",
        cache: false,
        dependsOn: [
          "@stillwind-internal/e2e-sveltekit3-svelte5#generate:types",
          "@stillwind/integration#build",
          "@stillwind/native#collect:local",
          "@stillwind/plugin-iconify#build",
          "stillwind#build",
        ],
      },
      "test:e2e:primary": {
        command: "node src/matrix.ts --support primary",
        cache: false,
        dependsOn: [
          "@stillwind-internal/e2e-sveltekit3-svelte5#generate:types",
          "@stillwind/integration#build",
          "@stillwind/native#collect:local",
          "@stillwind/plugin-iconify#build",
          "stillwind#build",
        ],
      },
    },
  },
});
