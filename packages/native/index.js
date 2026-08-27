import { createRequire } from "node:module"

const require = createRequire(import.meta.url)
const packages = {
  "win32-x64": "@stillwind/native-win32-x64-msvc",
  "win32-arm64": "@stillwind/native-win32-arm64-msvc",
  "darwin-x64": "@stillwind/native-darwin-x64",
  "darwin-arm64": "@stillwind/native-darwin-arm64",
  "linux-x64": "@stillwind/native-linux-x64-gnu",
  "linux-arm64": "@stillwind/native-linux-arm64-gnu",
}
const packageName = packages[`${process.platform}-${process.arch}`]

if (!packageName) throw new Error(`@stillwind/native has no binary package for ${process.platform}-${process.arch}`)

let binding
try {
  binding = require(packageName)
} catch (error) {
  throw new Error(
    `Unable to load ${packageName}. Reinstall dependencies, or build the local target with \`vp run --filter @stillwind/native build\`.`,
    { cause: error },
  )
}

export const compileReact = binding.compileReact
