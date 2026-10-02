import { defineConfig } from "vite-plus";

export default defineConfig({
  test: {
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
