import { spawnSync } from "node:child_process";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";

const require = createRequire(import.meta.url);
const args = [
  join(dirname(require.resolve("@napi-rs/cli/package.json")), "dist/cli.js"),
  "build",
  "--release",
  "--platform",
  "--esm",
  "--no-const-enum",
  "--manifest-path",
  "../../crates/napi/Cargo.toml",
  "--output-dir",
  ".build/native",
  "--dts",
  "binding.d.ts",
  "--package-json-path",
  "package.json",
];
// Release builds set this variable, because linking against the runner's glibc would raise the
// glibc version that published Linux binaries require. The toolchain only runs on Linux hosts.
if (process.env.NATIVE_USE_NAPI_CROSS === "true") {
  args.push("--use-napi-cross");
}

const { status, error } = spawnSync(process.execPath, args, {
  cwd: import.meta.dirname,
  stdio: "inherit",
});
if (error) {
  throw error;
}
process.exitCode = status ?? 1;
