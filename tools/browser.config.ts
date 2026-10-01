import { defineConfig } from "vite-plus";

export default defineConfig({
  test: {
    include: ["tailwind/browser.test.ts"],
    testTimeout: 60_000,
  },
});
