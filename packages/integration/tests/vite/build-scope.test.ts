import { subscribe, unsubscribe } from "node:diagnostics_channel";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import path from "node:path";

import { stillwind } from "@stillwind/integration/vite";
import { build } from "vite";
import { expect, test } from "vite-plus/test";

test("a build prepares the checker once for pages that an HTML entry imports dynamically", async () => {
  const root = await mkdtemp(path.join(import.meta.dirname, ".test-build-scope-"));
  const phases: string[] = [];
  const listener = (message: unknown) => {
    phases.push((message as { phase: string }).phase);
  };
  subscribe("stillwind:host:phase", listener);
  try {
    await writeFile(
      path.join(root, "root.css"),
      "@utility red{color:red} @utility blue{color:blue} @stillwind-slot utilities;\n",
    );
    await writeFile(
      path.join(root, "index.html"),
      '<!doctype html><script type="module" src="./main.ts"></script>\n',
    );
    await writeFile(
      path.join(root, "main.ts"),
      'import "./root.css";void import("./first.ts");void import("./second.ts");',
    );
    for (const [name, utility] of [
      ["first", "red"],
      ["second", "blue"],
    ]) {
      await writeFile(
        path.join(root, `${name}.ts`),
        `import {style} from "stillwind/react";import {${utility}} from "./tones.ts";export const props=style(${utility});`,
      );
    }
    await writeFile(
      path.join(root, "tones.ts"),
      'export const red = "red" as const;export const blue = "blue" as const;',
    );
    await writeFile(
      path.join(root, "stillwind.config.mts"),
      'export default {projects:[{css:"root.css",files:["*.ts"]}]};',
    );
    await build({
      root,
      configFile: false,
      logLevel: "silent",
      plugins: [stillwind()],
      build: {
        write: false,
        minify: false,
      },
    });
    expect(phases.filter((phase) => phase === "checker-prepare")).toHaveLength(1);
  } finally {
    unsubscribe("stillwind:host:phase", listener);
    await rm(root, { recursive: true, force: true, maxRetries: 5 });
  }
});
