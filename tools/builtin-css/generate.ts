import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { parseArgs } from "node:util";

import { bundleCss } from "./bundle.ts";

const {
  values: { check },
} = parseArgs({
  options: {
    check: {
      type: "boolean",
    },
  },
});

const root = path.resolve(import.meta.dirname, "../../packages/core");
const assets = path.resolve(import.meta.dirname, "../../crates/test-support/assets");

for (const target of ["index.css", "css/builtins.css"]) {
  const generated = (await bundleCss(path.resolve(root, target))).trimEnd() + "\n";
  const destination = path.join(assets, path.basename(target));
  if (check) {
    const current = await readFile(destination, "utf8").catch((error: NodeJS.ErrnoException) => {
      if (error.code === "ENOENT") {
        return null;
      }
      throw error;
    });
    if (current?.replaceAll("\r\n", "\n") !== generated) {
      throw new Error(`${destination} is stale. Run vp run generate:builtin-css.`);
    }
  } else {
    await mkdir(path.dirname(destination), { recursive: true });
    await writeFile(destination, generated);
  }
}
