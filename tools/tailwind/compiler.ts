import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

import { compile } from "tailwindcss";

const tailwindRoot = dirname(createRequire(import.meta.url).resolve("tailwindcss/package.json"));

function resolveStylesheet(id: string, base: string): string {
  const builtin = new Map([
    ["tailwindcss", "index.css"],
    ["tailwindcss/theme", "theme.css"],
    ["tailwindcss/preflight", "preflight.css"],
    ["tailwindcss/utilities", "utilities.css"],
  ]);
  const builtinPath = builtin.get(id);
  return builtinPath ? join(tailwindRoot, builtinPath) : resolve(base, id);
}

interface CompileTailwindCssOptions {
  css: string;
  candidates: string[];
  base: string;
  from?: string;
}

export async function compileTailwindCss({
  css,
  candidates,
  base,
  from = join(base, "fixture.css"),
}: CompileTailwindCssOptions): Promise<string> {
  const compiler = await compile(css, {
    base,
    from,
    async loadStylesheet(id, importerBase) {
      const path = resolveStylesheet(id, importerBase);
      return { path, base: dirname(path), content: readFileSync(path, "utf8") };
    },
    async loadModule(id, importerBase) {
      const path = resolve(importerBase, id);
      const loaded = await import(/* @vite-ignore */ pathToFileURL(path).href);
      return { path, base: dirname(path), module: loaded.default ?? loaded };
    },
  });
  return compiler.build(candidates).replaceAll("\r\n", "\n").trimEnd();
}
