import { defineConfig } from "vite-plus";

export default defineConfig({
  test: {
    include: ["tests/browser.test.ts"],
    environment: "node",
    testTimeout: 1_200_000,
    hookTimeout: 60_000,
    fileParallelism: false,
    maxWorkers: 1,
  },
});
