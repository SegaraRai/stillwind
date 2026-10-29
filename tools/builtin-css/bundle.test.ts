import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

import { expect, test } from "vite-plus/test";

import { bundleCss } from "./bundle.ts";

test("expands repeated local imports in order and keeps named layers at their positions", async () => {
  const directory = await mkdtemp(path.join(tmpdir(), "stillwind-builtin-css-"));
  try {
    await writeFile(
      path.join(directory, "entry.css"),
      `@layer theme, utilities;
@import "./theme.css" layer(theme);
@import './registry.css';
@import './registry.css' layer(utilities);`,
    );
    await writeFile(path.join(directory, "theme.css"), "@theme { --spacing: 0.25rem; }");
    await writeFile(
      path.join(directory, "registry.css"),
      `@import "./utility.css";
@stillwind-arbitrary-property *;`,
    );
    const utility = "/* Spacing */\n@stillwind-utility p-* { padding: --value(--spacing-*); }";
    await writeFile(path.join(directory, "utility.css"), utility);
    const registry = `${utility}\n@stillwind-arbitrary-property *;`;
    expect(await bundleCss(path.join(directory, "entry.css"))).toBe(
      "@layer theme, utilities;\n@layer theme {\n@theme { --spacing: 0.25rem; }\n}\n" +
        `${registry}\n@layer utilities {\n${registry}\n}`,
    );
    await writeFile(path.join(directory, "utility.css"), `@import "./entry.css";`);
    await expect(bundleCss(path.join(directory, "entry.css"))).rejects.toThrow(
      "Circular CSS import",
    );
    await writeFile(path.join(directory, "utility.css"), `@import "./theme.css" screen;`);
    await expect(bundleCss(path.join(directory, "entry.css"))).rejects.toThrow(
      "Unsupported CSS import",
    );
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
