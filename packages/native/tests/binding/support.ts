import { pathToFileURL } from "node:url";

/**
 * Converts a test path to an absolute file URI.
 *
 * @param path - Path resolved from the test process's working directory.
 * @returns File URI used by source reports and prepared inputs.
 */
export const sourceUri = (path: string) => pathToFileURL(path).href;

/**
 * Creates one CSS document for a one-shot compilation.
 *
 * @param content - Stylesheet text.
 * @returns CSS input with a stable document identity.
 */
export const cssInput = (content: string) => ({
  content,
  name: "root.css",
  uri: sourceUri("root.css"),
  sourceMapSource: sourceUri("root.css"),
});

/**
 * Creates a TS or TSX input and its stylesheet for a one-shot compilation.
 *
 * @param source - Original TSX source.
 * @param moduleId - Module identity and path used to construct the document URI.
 * @param theme - Stylesheet text.
 * @returns A prepared input with independent source and stylesheet documents.
 */
export const oneShotInput = (source: string, moduleId: string, theme: string) => ({
  source,
  moduleId: moduleId,
  uri: sourceUri(moduleId),
  sourceMapSource: sourceUri(moduleId),
  language: moduleId.endsWith(".tsx") ? "tsx" : "ts",
  moduleKind: "unambiguous",
  css: cssInput(theme),
});
