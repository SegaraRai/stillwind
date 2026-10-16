import { mkdtemp, rm, writeFile } from "node:fs/promises";
import path from "node:path";

import { stillwind } from "@stillwind/integration/vite";
import { build } from "vite";
import { expect, test } from "vite-plus/test";

test("denied stylesheet lint findings neither fail a build nor change its output", async () => {
  const root = await mkdtemp(path.join(import.meta.dirname, ".test-stylesheet-lint-"));
  try {
    await writeFile(
      path.join(root, "root.css"),
      "@custom-variant hover (&:hover); @utility red{color:red} @stillwind-slot utilities;\n.card { @apply hover:hover:red missing; }\n",
    );
    await writeFile(
      path.join(root, "entry.ts"),
      'import {style} from "stillwind/react";export const props=style("red");',
    );
    const warnings: string[] = [];
    /**
     * Builds the library entry with every stylesheet rule at `severity`.
     * @param severity - Severity of the rules that the stylesheet violates.
     * @returns The emitted file names and contents, in output order.
     */
    const compile = async (severity: "off" | "deny") => {
      await writeFile(
        path.join(root, "stillwind.config.mts"),
        `export default {projects:[{css:"root.css",files:["entry.ts"],lint:{rules:{"redundant-variant":"${severity}","unknown-utility":"${severity}"}}}]};`,
      );
      const result = await build({
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
      return (Array.isArray(result) ? result : [result]).flatMap((result) => {
        if (!("output" in result)) {
          expect.fail("Expected build output");
        }
        return result.output.map((output) => [
          output.fileName,
          output.type === "asset" ? String(output.source) : output.code,
        ]);
      });
    };
    const off = await compile("off");
    expect(await compile("deny")).toEqual(off);
    expect(warnings.filter((warning) => /redundant-variant|unknown-utility/.test(warning))).toEqual(
      [],
    );
  } finally {
    await rm(root, { recursive: true, force: true, maxRetries: 5 });
  }
});
