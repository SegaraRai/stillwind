import { mkdtemp, rm, writeFile } from "node:fs/promises";
import path from "node:path";

import { expect, test } from "vite-plus/test";

import { SourceFormatter } from "../dist/node.js";

test("source formatter serializes independent buffers and refreshes configuration and theme", async () => {
  const root = await mkdtemp(
    path.resolve(import.meta.dirname, "../../host/.test-source-formatter-"),
  );
  const config = path.join(root, "stillwind.config.ts");
  const file = path.join(root, "entry.tsx");
  const source = 'import {style} from "stillwind/react";const card=style("flex block");';
  let formatter: SourceFormatter | undefined;
  try {
    await writeFile(
      config,
      'export default {projects:[{css:"root.css",files:["*.tsx"],typeFacts:false,types:false}]}',
    );
    await writeFile(
      path.join(root, "root.css"),
      "@utility block { display: block; } @utility flex { display: flex; } @stillwind-slot utilities;",
    );
    formatter = new SourceFormatter(config);
    const outputs = await Promise.all([
      formatter.format({
        file,
        source,
      }),
      formatter.format({
        file,
        source: source.replace("flex block", "block flex"),
      }),
    ]);
    expect(outputs.map((output) => output.code)).toEqual([
      source.replace("flex block", "block flex"),
      source.replace("flex block", "block flex"),
    ]);
    await expect(
      formatter.format({
        file,
        source: source + "const broken=(",
      }),
    ).rejects.toThrow();
    await writeFile(
      path.join(root, "root.css"),
      "@utility flex { display: flex; } @utility block { display: block; } @stillwind-slot utilities;",
    );
    expect(
      (
        await formatter.format({
          file,
          source: outputs[0].code,
        })
      ).code,
    ).toBe(source);
    await writeFile(
      config,
      'export default {projects:[{css:"root.css",files:["*.tsx"],typeFacts:false,types:false,sort:{stringLiterals:false}}]}',
    );
    expect(
      (
        await formatter.format({
          file,
          source: outputs[0].code,
        })
      ).code,
    ).toBe(outputs[0].code);
    expect(
      (
        await formatter.format({
          file: path.join(root, "outside.ts"),
          source,
        })
      ).code,
    ).toBe(source);
  } finally {
    await formatter?.close();
    await rm(root, { recursive: true, force: true });
  }
});
