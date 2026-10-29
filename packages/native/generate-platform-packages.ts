import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";

import { readNapiConfig } from "@napi-rs/cli";

const root = import.meta.dirname;
const manifestPath = path.join(root, "package.json");
const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
const config = await readNapiConfig(manifestPath);
const overview = (await readFile(path.join(root, "README.md"), "utf8")).split(/\r?\n\r?\n/)[1];
if (!overview) {
  throw new Error("README.md must contain a Stillwind overview after its heading.");
}

// `napi create-npm-dirs` rewrites `files` for each platform package. npm includes
// `LICENSE` without a `files` entry, but the copies of third-party licenses need one.
for (const target of config.targets) {
  const directory = path.join(root, "npm", target.platformArchABI);
  const platformManifestPath = path.join(directory, "package.json");
  const platformManifest = JSON.parse(await readFile(platformManifestPath, "utf8"));
  for (const license of [
    "RUST-THIRD-PARTY-LICENSES",
    "TAILWINDCSS-MIT-LICENSE",
    "WEBREF-MIT-LICENSE",
  ]) {
    if (!platformManifest.files.includes(license)) {
      platformManifest.files.push(license);
    }
  }
  await writeFile(platformManifestPath, `${JSON.stringify(platformManifest, null, 2)}\n`);

  const readme = [
    `# \`${manifest.name}-${target.platformArchABI}\``,
    overview,
    `This package provides the **${target.triple}** binary for Stillwind's \`@stillwind/native\` bindings.`,
  ];
  await writeFile(path.join(directory, "README.md"), `${readme.join("\n\n")}\n`);
}
