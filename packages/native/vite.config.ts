/** Vite+ task configuration for the local Node-API addon. */
const currentTarget = {
  "win32-x64": "win32-x64-msvc",
  "win32-arm64": "win32-arm64-msvc",
  "darwin-x64": "darwin-x64",
  "darwin-arm64": "darwin-arm64",
  "linux-x64": "linux-x64-gnu",
  "linux-arm64": "linux-arm64-gnu",
}[`${process.platform}-${process.arch}`];

if (!currentTarget)
  throw new Error(`No Stillwind native target for ${process.platform}-${process.arch}`);

export default {
  run: {
    tasks: {
      "sync-platform-packages": {
        command: "napi create-npm-dirs --package-json-path package.json --npm-dir npm",
      },
      build: {
        command: `napi build --release --platform --no-js --manifest-path ../../crates/napi/Cargo.toml --output-dir npm/${currentTarget} --package-json-path package.json`,
      },
      check: {
        command: "node --test test/loader.test.mjs",
        dependsOn: ["build"],
      },
      "prepare-publish": {
        command:
          "napi pre-publish --package-json-path package.json --npm-dir npm --root-publisher pnpm",
      },
    },
  },
};
