import { defineConfig } from "vite-plus";

export default defineConfig({
  root: import.meta.dirname,
  server: {
    host: "127.0.0.1",
    port: 5198,
    strictPort: true,
  },
  test: {
    name: "logo-editor",
    include: ["src/**/*.test.ts"],
  },
  run: {
    tasks: {
      dev: {
        command: "vp dev",
        cache: false,
      },
      build: {
        command: "vp build",
      },
      check: {
        command: "tsc --noEmit",
      },
      test: {
        command: "vp test --run",
        cache: false,
      },
    },
  },
});
