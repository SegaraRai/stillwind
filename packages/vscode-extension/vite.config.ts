import { defineConfig } from "vite-plus";

export default defineConfig(({ mode }) => ({
  root: import.meta.dirname,
  test: {
    name: "extension-package",
    include: mode === "package" ? ["tests/**/*.test.ts"] : ["src/**/*.test.ts"],
    testTimeout: 60_000,
  },
  pack: {
    entry: ["src/manifest.ts"],
    outDir: "dist/api",
    format: ["esm"],
    dts: true,
    fixedExtension: false,
  },
  run: {
    tasks: {
      build: {
        command: "node build.ts",
        cache: {
          input: [{ auto: true }, "!dist/**"],
          output: ["dist/extension/**"],
        },
        dependsOn: [
          "build:api",
          "@stillwind/native#collect:local",
          "@stillwind/language-server#build",
          "@stillwind/config#build",
          "stillwind#build",
        ],
      },
      "build:api": {
        command: "vp pack",
      },
      "package:desktop": {
        command: "node build.ts --package-desktop",
        cache: {
          input: [{ auto: true }, "!dist/desktop/**", "!dist/web/**", "!dist/*.vsix"],
          output: ["dist/desktop/**", "dist/stillwind-desktop.vsix"],
        },
        dependsOn: ["build"],
      },
      "package:web": {
        command: "node build.ts --package-web",
        cache: {
          input: [{ auto: true }, "!dist/desktop/**", "!dist/web/**", "!dist/*.vsix"],
          output: ["dist/stillwind-web.vsix"],
        },
        dependsOn: ["build"],
      },
      typecheck: {
        command: "tsc --noEmit",
        dependsOn: ["@stillwind/language-server#build", "@stillwind/config#build"],
      },
      "test:package": {
        command: "vp test --run --mode package",
        cache: false,
        dependsOn: ["package:desktop"],
      },
    },
  },
}));
