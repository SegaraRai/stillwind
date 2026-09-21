import { defineConfig } from "vite-plus";

export default defineConfig({
  pack: {
    entry: ["src/index.ts"],
    format: ["esm"],
    fixedExtension: false,
    dts: true,
    sourcemap: true,
  },
  run: {
    tasks: {
      build: {
        command: "vp pack",
      },
      typecheck: {
        command: "tsc --noEmit",
      },
    },
  },
});
