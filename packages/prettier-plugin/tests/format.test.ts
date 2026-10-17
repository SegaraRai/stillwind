import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { format, formatWithCursor } from "prettier";
import { expect, test } from "vite-plus/test";

import * as plugin from "../dist/index.js";

async function project(run: (root: string) => Promise<void>) {
  const root = await mkdtemp(path.resolve(import.meta.dirname, "../../host/.test-prettier-"));
  try {
    await writeFile(
      path.join(root, "stillwind.config.ts"),
      'export default {projects:[{css:"root.css",files:["*.{ts,tsx,js,jsx,mts,cts,mjs,cjs,vue,svelte,astro}"]}]}',
    );
    await writeFile(
      path.join(root, "root.css"),
      "@utility block{display:block} @utility flex{display:flex} @utility p-4{padding:1rem} @stillwind-slot utilities;",
    );
    await run(root);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}

test("Prettier sorts unsaved buffers, keeps ignored nodes, and is idempotent", async () => {
  await project(async (root) => {
    const filepath = path.join(root, "entry.tsx");
    const disk = 'import {style} from "stillwind/react";const disk=style("p-4");';
    await writeFile(filepath, disk);
    const source =
      'import {style} from "stillwind/react";\n// prettier-ignore\nconst ignored=style("flex block");\nconst value=style("flex block p-4");\n';
    const options = { filepath, plugins: [plugin] };
    const output = await format(source, options);
    expect(output).toContain('const ignored=style("flex block");');
    expect(output).toContain('style("block flex p-4")');
    expect(await format(output, options)).toBe(output);
    expect(await readFile(filepath, "utf8")).toBe(disk);
  });
});

test("CSS formatting sorts apply arguments and respects ignores and formatting ranges", async () => {
  await project(async (root) => {
    const filepath = path.join(root, "root.css");
    const definitions = await readFile(filepath, "utf8");
    const source = `${definitions}\n/* prettier-ignore */\n.ignored { @apply flex block; }\n.before { @apply flex block; }\n.selected { @apply flex block; }\n`;
    const options = {
      filepath,
      plugins: [plugin],
    };
    const output = await format(source, options);
    expect(output).toContain(".ignored { @apply flex block; }");
    expect(output.match(/@apply block flex/g)).toHaveLength(2);
    expect(await format(output, options)).toBe(output);
    const selected = await format(source, {
      ...options,
      rangeStart: source.indexOf(".selected"),
      rangeEnd: source.length,
    });
    expect(selected).toContain(".before { @apply flex block; }");
    expect(selected).toContain("@apply block flex;");
    expect(await readFile(filepath, "utf8")).toBe(definitions);
  });
});

test("range formatting changes only the selected source and cursors follow reordered utilities", async () => {
  await project(async (root) => {
    const filepath = path.join(root, "entry.tsx");
    const source =
      'import {style} from "stillwind/react";\nconst before=style("flex block");\nconst selected=style("flex block");\nconst after=style("flex block");\n';
    const result = await format(source, {
      filepath,
      plugins: [plugin],
      rangeStart: source.indexOf("const selected"),
      rangeEnd: source.indexOf("const after"),
    });
    expect(result).toContain('const before=style("flex block");');
    expect(result).toContain('const after=style("flex block");');
    expect(result).toContain('style("block flex")');
    const cursorSource =
      'import {style} from "stillwind/react";const value=style("flex block p-4");';
    const cursor = await formatWithCursor(cursorSource, {
      filepath,
      plugins: [plugin],
      cursorOffset: cursorSource.indexOf("flex block") + 1,
    });
    expect(cursor.formatted.slice(cursor.cursorOffset - 1, cursor.cursorOffset + 3)).toBe("flex");
  });
});

test("framework parsers delegate printing after sorting original scripts and expressions", async () => {
  await project(async (root) => {
    const cases = [
      {
        extension: "vue",
        plugins: [plugin],
        source:
          '<script setup lang="ts">import {style} from "stillwind/vue";\n// prettier-ignore\nconst ignored=style("flex block");\nconst card=style("flex block");</script><template><div v-bind="card" /></template>',
      },
      {
        extension: "svelte",
        plugins: [fileURLToPath(import.meta.resolve("prettier-plugin-svelte")), plugin],
        source:
          '<script lang="ts">import {style} from "stillwind/svelte";\n// prettier-ignore\nconst ignored=style("flex block");\nconst card=style("flex block");</script><div {...card}></div>',
      },
      {
        extension: "astro",
        plugins: [fileURLToPath(import.meta.resolve("prettier-plugin-astro")), plugin],
        source:
          '---\nimport {style} from "stillwind/astro";\n// prettier-ignore\nconst ignored=style("flex block");\nconst card=style("flex block");\n---\n<div {...card}></div>',
      },
    ];
    for (const entry of cases) {
      const options = {
        filepath: path.join(root, `entry.${entry.extension}`),
        plugins: entry.plugins,
      };
      const output = await format(entry.source, options);
      expect(output, entry.extension).toContain('style("block flex")');
      expect(output, entry.extension).toContain('style("flex block")');
      expect(await format(output, options), entry.extension).toBe(output);
    }
  });
});

test("files without a configuration retain ordinary formatting and invalid explicit config fails", async () => {
  const filepath = path.resolve(import.meta.dirname, "unconfigured.tsx");
  const source = 'const value="flex block";';
  expect(await format(source, { filepath, plugins: [plugin] })).toBe(
    await format(source, { filepath }),
  );
  const options = { filepath, plugins: [plugin], stillwindConfig: "missing-stillwind.config.ts" };
  await expect(format(source, options)).rejects.toThrow();
});

test("ignored utility literals cannot move across neighboring array inputs", async () => {
  await project(async (root) => {
    const source =
      'import {style} from "stillwind/react";const base=style("p-4");const ref=style.ref();const value=style(["flex",\n// prettier-ignore\n"flex block",\nbase,ref,"flex block"]);';
    const output = await format(source, {
      filepath: path.join(root, "entry.jsx"),
      plugins: [plugin],
    });
    expect(output).toMatch(
      /"flex",\s*\/\/ prettier-ignore\s*"flex block",\s*ref,\s*base,\s*"block flex"/,
    );
  });
});
