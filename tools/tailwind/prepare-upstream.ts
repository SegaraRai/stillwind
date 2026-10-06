import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { parseArgs } from "node:util";

import installed from "tailwindcss/package.json" with { type: "json" };

import { prepareTailwindSource, refreshTailwindSource, type TailwindSource } from "./upstream.ts";

parseArgs({
  options: {},
});

const sourceFile = resolve(import.meta.dirname, "upstream-source.json");
let source: TailwindSource | undefined;
try {
  source = JSON.parse(readFileSync(sourceFile, "utf8"));
} catch (error) {
  if ((error as NodeJS.ErrnoException).code !== "ENOENT") {
    throw error;
  }
}

const options = {
  cacheDirectory: resolve(import.meta.dirname, ".cache/upstream"),
  expectedVersion: installed.version,
};

if (!source || source.version !== installed.version) {
  source = await refreshTailwindSource({
    ...options,
    sourceFile,
  });
} else {
  await prepareTailwindSource({
    ...options,
    source,
  });
}

console.log(`Prepared Tailwind ${source.version} source at ${source.revision}`);
