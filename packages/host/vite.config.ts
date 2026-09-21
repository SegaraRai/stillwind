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
        dependsOn: ["@stillwind/native#build", "@stillwind/projection#build", "stillwind#build"],
      },
      typecheck: {
        command: "tsc --noEmit",
        dependsOn: ["@stillwind/native#build", "@stillwind/projection#build", "stillwind#build"],
      },
      test: {
        command: "node --test test/*.test.mts",
        dependsOn: ["build"],
      },
    },
  },
});
