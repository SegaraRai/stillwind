import { createRequire } from "node:module"

const require = createRequire(import.meta.url)
const bindings = {
  "win32-x64": "./shorth_native.win32-x64-msvc.node",
  "win32-arm64": "./shorth_native.win32-arm64-msvc.node",
  "darwin-x64": "./shorth_native.darwin-x64.node",
  "darwin-arm64": "./shorth_native.darwin-arm64.node",
  "linux-x64": "./shorth_native.linux-x64-gnu.node",
  "linux-arm64": "./shorth_native.linux-arm64-gnu.node",
}
const bindingPath = bindings[`${process.platform}-${process.arch}`]

if (!bindingPath) {
  throw new Error(`@shorth/native has no local addon for ${process.platform}-${process.arch}`)
}

let binding
try {
  binding = require(bindingPath)
} catch (error) {
  throw new Error(
    "@shorth/native is not built. Run `vp exec napi build --release --platform --manifest-path crates/shorth-napi/Cargo.toml --output-dir packages/native --package-json-path packages/native/package.json`.",
    { cause: error },
  )
}

export const compileReact = binding.compileReact
