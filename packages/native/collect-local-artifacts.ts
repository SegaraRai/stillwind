import { access, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";

import { NapiCli, readNapiConfig } from "@napi-rs/cli";

const root = import.meta.dirname;
const manifestPath = path.join(root, "package.json");
const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
const config = await readNapiConfig(manifestPath);
const targets: string[] = [];
for (const target of config.targets) {
  const extension = target.platform === "wasi" ? "wasm" : "node";
  const artifact = path.join(
    root,
    "dist",
    ...(target.platform === "wasi" ? ["wasm"] : []),
    `${config.binaryName}.${target.platformArchABI}.${extension}`,
  );
  try {
    await access(artifact);
    targets.push(target.triple);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "ENOENT") {
      throw error;
    }
  }
}
const temporary = await mkdtemp(path.join(root, ".test-collect-"));
try {
  const configPath = path.join(temporary, "napi.json");
  await writeFile(configPath, JSON.stringify({ ...manifest.napi, targets }));
  await new NapiCli().artifacts({
    cwd: root,
    packageJsonPath: "package.json",
    configPath,
    outputDir: "dist",
    buildOutputDir: "dist/wasm",
    npmDir: "npm",
  });
  // WASI loaders prefer debug companions, which would override the collected release module.
  for (const target of config.targets) {
    if (target.platform === "wasi" && targets.includes(target.triple)) {
      await rm(
        path.join(
          root,
          "npm",
          target.platformArchABI,
          `${config.binaryName}.${target.platformArchABI}.debug.wasm`,
        ),
        { force: true },
      );
    }
  }
} finally {
  await rm(temporary, { recursive: true, force: true });
}
