import { defineConfig } from "vite-plus";

export default defineConfig({
  root: import.meta.dirname,
  test: {
    name: "e2e-harness",
    include: ["src/**/*.test.ts"],
    exclude: ["src/wasm-browser.test.ts"],
  },
  run: {
    tasks: {
      typecheck: {
        command: "tsc --noEmit",
      },
      check: {
        command: "node src/matrix.ts --typecheck",
        dependsOn: ["typecheck", "@stillwind/integration#build"],
        cache: false,
      },
      test: {
        command: "vp test --run",
        cache: false,
      },
      "test:wasm": {
        command: "vp test --run --config vite.browser.config.ts",
        dependsOn: ["@stillwind/native#collect:local"],
        cache: false,
      },
      "e2e:primary": {
        command: "node src/matrix.ts --support primary",
        dependsOn: ["@stillwind/integration#build", "@stillwind/native#collect:local"],
        cache: false,
      },
      "e2e:compatibility": {
        command: "node src/matrix.ts --support compatibility",
        dependsOn: ["@stillwind/integration#build", "@stillwind/native#collect:local"],
        cache: false,
      },
      e2e: {
        command: "node src/matrix.ts",
        dependsOn: ["@stillwind/integration#build", "@stillwind/native#collect:local"],
        cache: false,
      },
    },
  },
});
