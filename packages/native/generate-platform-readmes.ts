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

for (const target of config.targets) {
  const readme = [
    `# \`${manifest.name}-${target.platformArchABI}\``,
    overview,
    `This package provides the **${target.triple}** binary for Stillwind's \`@stillwind/native\` bindings.`,
  ];
  await writeFile(
    path.join(root, "npm", target.platformArchABI, "README.md"),
    `${readme.join("\n\n")}\n`,
  );
}
