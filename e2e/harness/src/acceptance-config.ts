import { join } from "node:path";

import { defineConfig } from "vite-plus";

export default defineConfig({
  test: {
    fsModuleCachePath: join(process.cwd(), "node_modules/.vitest-acceptance"),
    include: ["tests/**/*.test.ts"],
    environment: "node",
    reporters: ["verbose"],
    sequence: { concurrent: false },
    testTimeout: 600_000,
    hookTimeout: 600_000,
    fileParallelism: false,
    maxWorkers: 1,
  },
});
