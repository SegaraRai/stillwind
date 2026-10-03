import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, realpathSync, rmSync, symlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

import { expect, test } from "vite-plus/test";

test("packed root package falls back to the separately packed WASI package", () => {
  const temporary = mkdtempSync(path.join(tmpdir(), "stillwind-wasi-package-"));
  const root = path.resolve(import.meta.dirname, "..");
  try {
    const packages = path.join(temporary, "node_modules", "@stillwind");
    mkdirSync(packages, { recursive: true });
    for (const [name, source] of [
      ["native", root],
      ["native-wasm32-wasi", path.join(root, "npm/wasm32-wasi")],
    ]) {
      const archive = path.join(temporary, `${name}.tgz`);
      execFileSync(process.platform === "win32" ? "pnpm.exe" : "pnpm", ["pack", "--out", archive], {
        cwd: source,
        stdio: "pipe",
      });
      const destination = path.join(packages, name);
      mkdirSync(destination);
      execFileSync("tar", ["-xzf", archive, "--strip-components=1", "-C", destination]);
    }
    const wasiModules = path.join(packages, "native-wasm32-wasi/node_modules");
    for (const name of ["@emnapi/core", "@emnapi/runtime", "@napi-rs/wasm-runtime"]) {
      const destination = path.join(wasiModules, name);
      mkdirSync(path.dirname(destination), { recursive: true });
      symlinkSync(
        realpathSync(path.join(root, "npm/wasm32-wasi/node_modules", name)),
        destination,
        process.platform === "win32" ? "junction" : "dir",
      );
    }
    const script = `
      const binding = await import('@stillwind/native');
      console.log(JSON.stringify({
        target: binding.__napiBindingTarget,
        css: binding.minifyCss('.a { color: red }').css,
      }));
    `;
    for (const force of [undefined, "error"]) {
      const env = { ...process.env };
      delete env.NAPI_RS_NATIVE_LIBRARY_PATH;
      delete env.NAPI_RS_WASI_FLAVOR;
      delete env.NAPI_RS_FORCE_WASI;
      if (force) {
        env.NAPI_RS_FORCE_WASI = force;
      }
      const output = execFileSync(process.execPath, ["--input-type=module", "-e", script], {
        cwd: temporary,
        env,
        encoding: "utf8",
        timeout: 30_000,
        stdio: ["ignore", "pipe", "pipe"],
      });
      expect(JSON.parse(output)).toStrictEqual({ target: "wasm32-wasi", css: ".a{color:red}" });
    }
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
}, 60_000);
