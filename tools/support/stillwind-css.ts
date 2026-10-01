import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const packageRoot = fileURLToPath(new URL("../../packages/core/", import.meta.url));

export function resolveFixtureCss(css: string, directory = packageRoot): string {
  return css.replace(
    /@import\s+(["'])([^"']+)\1\s*([^;]*);/g,
    (rule: string, _quote: string, name: string, tail: string) => {
      const path =
        name === "stillwind"
          ? resolve(packageRoot, "index.css")
          : name.startsWith("stillwind/")
            ? resolve(packageRoot, "css", name.slice(10))
            : name.startsWith("./")
              ? resolve(directory, name)
              : null;
      if (!path) {
        return rule;
      }
      const content = resolveFixtureCss(readFileSync(path, "utf8"), dirname(path));
      // Fixtures supply candidates explicitly; source(none) only disables discovery.
      // This adapter does not imply production support for Tailwind import modifiers.
      const modifiers = tail.replace(/\bsource\(none\)/g, "").trim();
      const layer = /^layer\(([^)]+)\)$/.exec(modifiers);
      if (layer) {
        return `@layer ${layer[1]}{${content}}`;
      }
      if (modifiers) {
        throw new Error(`Unsupported fixture import modifier: ${tail}`);
      }
      return content;
    },
  );
}
