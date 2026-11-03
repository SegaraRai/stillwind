import { mkdtemp, rm, writeFile } from "node:fs/promises";
import path from "node:path";

import { stillwind } from "@stillwind/integration/vite";
import { build } from "vite";
import { expect, test } from "vite-plus/test";

test("Vite warns only for an application import of the configured CSS entrypoint", async () => {
  const root = await mkdtemp(path.join(import.meta.dirname, ".test-entrypoint-import-"));
  try {
    await writeFile(
      path.join(root, "root.css"),
      "@utility red{color:red} @stillwind-slot utilities;\n",
    );
    await writeFile(path.join(root, "other.css"), ".other{color:blue}\n");
    await writeFile(
      path.join(root, "entry.ts"),
      'import "./root.css";import "./other.css";import {style} from "stillwind/react";export const props=style("red");',
    );
    await writeFile(
      path.join(root, "stillwind.config.mts"),
      'export default {projects:[{css:"root.css",files:["entry.ts"]}]};',
    );
    const warnings: string[] = [];
    await build({
      root,
      configFile: false,
      logLevel: "silent",
      plugins: [stillwind()],
      build: {
        write: false,
        minify: false,
        lib: {
          entry: path.join(root, "entry.ts"),
          formats: ["es"],
        },
        rollupOptions: {
          onwarn: (warning) => {
            warnings.push(warning.message);
          },
        },
      },
    });
    expect(warnings.filter((warning) => warning.includes("CSS entrypoint"))).toEqual([
      expect.stringContaining("root.css is imported as an ordinary stylesheet"),
    ]);
  } finally {
    await rm(root, { recursive: true, force: true, maxRetries: 5 });
  }
});
