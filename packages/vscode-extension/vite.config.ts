import { createRequire, isBuiltin } from "node:module";
import path from "node:path";

import { defineConfig } from "vite-plus";
import type { PackUserConfig } from "vite-plus/pack";

const require = createRequire(import.meta.url);
const rolldownBrowser = path.dirname(require.resolve("@rolldown/browser"));

const runtime: PackUserConfig = {
  clean: ["dist/extension"],
  dts: false,
  fixedExtension: false,
  sourcemap: true,
  target: "esnext",
  deps: {
    neverBundle: ["vscode"],
    alwaysBundle: (id) => id !== "vscode" && !isBuiltin(id),
  },
  outputOptions: {
    chunkFileNames: "[name]-[hash].js",
  },
};

const pack: PackUserConfig[] = [
  {
    name: "api",
    entry: ["src/manifest.ts"],
    outDir: "dist/api",
    format: ["esm"],
    dts: true,
    fixedExtension: false,
  },
  {
    ...runtime,
    name: "runtime-web-extension",
    entry: {
      extension: "src/browser.ts",
    },
    outDir: "dist/extension/dist/browser",
    format: "cjs",
    platform: "browser",
    inputOptions: {
      platform: "browser",
    },
    outputOptions: {
      entryFileNames: "extension.js",
    },
  },
  {
    ...runtime,
    name: "runtime-web-server",
    entry: {
      bundle: "src/server/bundle.ts",
      "server-module": "src/server/browser.ts",
    },
    outDir: "dist/extension/dist/browser",
    format: "esm",
    platform: "browser",
  },
  {
    ...runtime,
    name: "runtime-native-worker",
    entry: {
      "wasi-worker-browser": "../native/npm/wasm32-wasi/wasi-worker-browser.mjs",
    },
    outDir: "dist/extension/dist/browser/@stillwind/native-wasm32-wasi",
    format: "esm",
    platform: "browser",
    sourcemap: false,
    outputOptions: {
      entryFileNames: "wasi-worker-browser.mjs",
    },
  },
  {
    ...runtime,
    name: "runtime-rolldown-worker",
    entry: {
      "wasi-worker-browser": path.join(rolldownBrowser, "wasi-worker-browser.mjs"),
    },
    outDir: "dist/extension/dist/browser",
    format: "esm",
    platform: "browser",
    sourcemap: false,
    outputOptions: {
      entryFileNames: "wasi-worker-browser.mjs",
    },
  },
  {
    ...runtime,
    name: "runtime-node-extension",
    entry: {
      extension: "src/node.ts",
    },
    outDir: "dist/extension/dist/node",
    format: "cjs",
    platform: "node",
    outputOptions: {
      entryFileNames: "extension.cjs",
    },
  },
  {
    ...runtime,
    name: "runtime-node-server",
    entry: {
      server: "src/server/node.ts",
    },
    outDir: "dist/extension/dist/node",
    format: "esm",
    platform: "node",
    deps: {
      neverBundle: true,
    },
    sourcemap: false,
  },
];

export default defineConfig(({ mode }) => ({
  root: import.meta.dirname,
  test: {
    name: "extension-package",
    include:
      mode === "package"
        ? ["tests/package.test.ts"]
        : mode === "browser"
          ? ["tests/browser.test.ts"]
          : ["src/**/*.test.ts"],
    testTimeout: 60_000,
  },
  pack,
  run: {
    tasks: {
      build: {
        command: "node build.ts",
        cache: {
          input: [{ auto: true }, "!dist/**"],
          output: [
            "dist/extension/package.json",
            "dist/extension/README.md",
            "dist/extension/LICENSE",
            "dist/extension/dist/node/package.json",
            "dist/extension/dist/browser/server.js",
            "dist/extension/dist/browser/stillwind_native.wasm32-wasi.wasm",
            "dist/extension/dist/browser/rolldown-binding.wasm32-wasi.wasm",
            "dist/extension/dist/browser/libraries.json",
          ],
        },
        dependsOn: ["build:runtime"],
      },
      "build:runtime": {
        command: "vp pack",
        cache: {
          input: [{ auto: true }, "!dist/**"],
          output: ["dist/api/**", "dist/extension/**"],
        },
        dependsOn: [
          "@stillwind/native#collect:local",
          "@stillwind/language-server#build",
          "@stillwind/config#build",
          "stillwind#build",
        ],
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
      "test:browser": {
        command: "vp test --run --mode browser",
        cache: false,
        dependsOn: ["build"],
      },
      "test:package": {
        command: "vp test --run --mode package",
        cache: false,
        dependsOn: ["package:desktop"],
      },
    },
  },
}));
