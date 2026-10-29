import { createRequire } from "node:module";

import { bundleCss } from "@stillwind-internal/tools-builtin-css/bundle";
import { minifyCss } from "@stillwind-internal/tools-builtin-css/minify";

import { importKey, type RecordedStylesheet } from "./stylesheet";

const require = createRequire(import.meta.url);

let recording: Promise<RecordedStylesheet> | undefined;

/**
 * Bundles and compacts the default package for the browser's utility lookup.
 * The recording keeps the package's library origin and contains no file URIs.
 */
export function recordDefaultStylesheet(): Promise<RecordedStylesheet> {
  recording ??= record();
  return recording;
}

async function record(): Promise<RecordedStylesheet> {
  const importer = "utilities.css";
  const specifier = "stillwind";
  const content = minifyCss(await bundleCss(require.resolve(specifier)));
  return {
    root: {
      name: importer,
      content: '@import "stillwind";',
      lint: true,
    },
    imports: {
      [importKey({ importer, specifier })]: {
        name: "stillwind/index.css",
        content,
        lint: false,
        origin: {
          kind: "library",
          name: "stillwind",
        },
      },
    },
  };
}
