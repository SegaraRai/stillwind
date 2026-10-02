import { defineConfig } from "vite-plus";

export default defineConfig({
  test: {
    include: ["src/**/*.test.ts"],
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
      "e2e:primary": {
        command: "node src/matrix.ts --support primary",
        dependsOn: ["@stillwind/integration#build"],
        cache: false,
      },
      "e2e:compatibility": {
        command: "node src/matrix.ts --support compatibility",
        dependsOn: ["@stillwind/integration#build"],
        cache: false,
      },
      e2e: {
        command: "node src/matrix.ts",
        dependsOn: ["@stillwind/integration#build"],
        cache: false,
      },
    },
  },
});
