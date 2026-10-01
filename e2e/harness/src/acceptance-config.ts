import { defineConfig } from "vite-plus";

export default defineConfig({
  test: {
    include: ["tests/**/*.test.ts"],
    environment: "node",
    testTimeout: 600_000,
    hookTimeout: 30_000,
    fileParallelism: false,
    maxWorkers: 1,
  },
});
