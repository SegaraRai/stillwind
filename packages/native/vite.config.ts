import { defineConfig } from "vite-plus";

/** Vite+ task configuration for the local Node-API addon. */
const targets: Record<string, string> = {
  "win32-x64": "win32-x64-msvc",
  "win32-arm64": "win32-arm64-msvc",
  "darwin-x64": "darwin-x64",
  "darwin-arm64": "darwin-arm64",
  "linux-x64": "linux-x64-gnu",
  "linux-arm64": "linux-arm64-gnu",
};
const currentTarget = targets[`${process.platform}-${process.arch}`];

if (!currentTarget) {
  throw new Error(`No Stillwind native target for ${process.platform}-${process.arch}`);
}

export default defineConfig({
  run: {
    tasks: {
      "sync-platform-packages": {
        command: "napi create-npm-dirs --package-json-path package.json --npm-dir npm",
      },
      build: {
        command: `napi build --release --platform --no-js --no-const-enum --manifest-path ../../crates/napi/Cargo.toml --output-dir npm/${currentTarget} --dts ../../binding.d.ts --package-json-path package.json`,
      },
      typecheck: {
        command: "tsc --noEmit",
        dependsOn: ["build"],
      },
      check: {
        command: "node --test test/loader.test.mjs",
        dependsOn: ["typecheck"],
      },
      "prepare-publish": {
        command:
          "napi pre-publish --package-json-path package.json --npm-dir npm --root-publisher pnpm",
      },
    },
  },
});
