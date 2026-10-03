import { fixtureCache } from "@stillwind-tools/workspace/task-cache";
import { defineConfig } from "vite-plus";

export default defineConfig({
  root: import.meta.dirname,
  run: {
    tasks: {
      "generate:webref": {
        command: "node webref.ts",
        cache: fixtureCache,
      },
      "check:generated:webref": {
        command: "node webref.ts --check",
        cache: fixtureCache,
      },
      typecheck: {
        command: "tsc --noEmit",
      },
    },
  },
});
