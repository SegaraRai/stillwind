import { fileURLToPath } from "node:url";

import type { NativeStylesheetPart } from "@stillwind/native";

/** An asset referenced by CSS returned through an explicit module value. */
export interface CssValueResource {
  /** Token in the CSS text that the fragment loader replaces with an asset URL. */
  placeholder: string;
  /** Resolver identity of the asset, including any resource query. */
  id: string;
  /** Module request relative to the producer directory, stored in the fragment payload. */
  request: string;
}

/** CSS and asset references encoded in the query of an isolated fragment loader request. */
export interface CssFragment {
  /** CSS text for one stylesheet occurrence. */
  css: string;
  /** Resolved assets whose placeholders occur in this fragment. */
  resources: CssValueResource[];
}

/**
 * Generates a module that imports CSS fragments and exports their ordered values.
 *
 * Each fragment request encodes its CSS and referenced assets. Equal payloads produce equal
 * requests, while the value retains occurrence order.
 *
 * @param parts - Stylesheet occurrences in cascade order. Repeated occurrences keep their
 * positions.
 * @param resources - Resolved assets whose placeholders may occur in the parts.
 * @returns JavaScript exporting `parts`, the joined `css` string, and the same string as its
 * default export.
 */
export function cssValueModule(
  parts: readonly NativeStylesheetPart[],
  resources: readonly CssValueResource[],
): string {
  const imports = parts.map((part, index) => {
    const payload = Buffer.from(
      JSON.stringify({
        css: part.css,
        resources: resources.filter((resource) => part.css.includes(resource.placeholder)),
      } satisfies CssFragment),
    ).toString("base64url");
    const request = `!!${fileURLToPath(
      new URL("./fragment-loader.js", import.meta.resolve("@stillwind/integration/webpack")),
    )}!${fileURLToPath(
      new URL("./stillwind-fragment.js", import.meta.resolve("@stillwind/integration/webpack")),
    )}?css=${payload}`;
    return `import { css as p${index} } from ${JSON.stringify(request)};\n`;
  });
  const entries = parts.map(
    (part, index) => `  { key: ${JSON.stringify(part.key)}, css: p${index} },\n`,
  );
  return [
    "// Stillwind CSS value (webpack)\n",
    ...imports,
    "\n",
    "export const parts = [\n",
    ...entries,
    "];\n",
    'export const css = parts.map((part) => part.css).join("");\n',
    "export default css;\n",
  ].join("");
}
