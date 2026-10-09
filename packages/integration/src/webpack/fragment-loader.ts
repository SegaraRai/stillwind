import type { LoaderContext } from "webpack";

import type { CssFragment } from "./css-value.ts";

/**
 * Decodes an immutable CSS fragment into a module that substitutes bundler-resolved asset URLs.
 * @param this - Loader context whose css query contains base64url-encoded UTF-8 fragment JSON.
 * @returns JavaScript importing referenced assets and exporting the fragment's CSS.
 * @throws If the css query is absent or its decoded JSON is invalid.
 */
export default function webpackFragmentLoader(this: LoaderContext<unknown>): string {
  const encoded = new URLSearchParams(this.resourceQuery).get("css");
  if (!encoded) {
    throw new Error("Missing Stillwind CSS fragment");
  }
  const { css, resources } = JSON.parse(
    Buffer.from(encoded, "base64url").toString("utf8"),
  ) as CssFragment;
  const imports = resources.map(
    (resource, index) => `import asset${index} from ${JSON.stringify(resource.id)};\n`,
  );
  const replacements = resources
    .map(
      (resource, index) =>
        `.replaceAll(${JSON.stringify(resource.placeholder)}, JSON.stringify(typeof asset${index} === "string" ? asset${index} : asset${index}.src).slice(1, -1))`,
    )
    .join("");
  return [
    "// Stillwind CSS fragment (webpack)\n",
    ...imports,
    "\n",
    `export const css = ${JSON.stringify(css)}${replacements};\n`,
  ].join("");
}
