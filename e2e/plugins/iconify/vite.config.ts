import { defineConfig } from "vite-plus";

export default defineConfig({
  run: {
    tasks: {
      typecheck: {
        command: "tsc --noEmit",
        dependsOn: [
          "@stillwind/integration#build",
          "@stillwind/plugin-iconify#build",
          "stillwind#build",
        ],
      },
      "test:e2e": {
        command: "vp test --run --config acceptance.config.ts",
        cache: false,
        dependsOn: [
          "@stillwind/integration#build",
          "@stillwind/native#collect:local",
          "@stillwind/plugin-iconify#build",
          "stillwind#build",
        ],
      },
    },
  },
});
