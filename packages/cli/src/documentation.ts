import { existsSync } from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";

/**
 * Finds the documentation page of a report code.
 *
 * @param code - Compiler code, such as `SW01001`, or lint rule name.
 * @param root - Root directory of the installed `stillwind` package, when one resolves.
 * @returns The absolute path of the page in that package, which matches the installed version.
 * Otherwise, the page on the website. Codes without a page return undefined.
 */
export function documentationPage(code: string, root: string | undefined): string | undefined {
  const section = /^SW\d+$/.test(code) ? "errors" : /^[a-z][a-z-]*$/.test(code) ? "lints" : null;
  if (!section) {
    return undefined;
  }
  const file = root && path.join(root, "docs", section, `${code}.md`);
  return file && existsSync(file) ? file : `https://stillwind.dev/docs/${section}/${code}`;
}

/**
 * Finds the `stillwind` package that a configuration file resolves.
 *
 * @param config - Absolute configuration path.
 * @returns The package root, or undefined when no package resolves.
 */
export function stillwindRoot(config: string | null): string | undefined {
  try {
    return config
      ? path.dirname(createRequire(config).resolve("stillwind/package.json"))
      : undefined;
  } catch {
    return undefined;
  }
}
