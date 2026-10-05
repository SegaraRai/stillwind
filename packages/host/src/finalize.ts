/**
 * Prepends an optional side-effect stylesheet import to compiled JavaScript.
 *
 * @param code - Compiled JavaScript text. The import is inserted before all existing text.
 * @param stylesheetSpecifier - Module specifier to import. Omission or an empty string leaves the
 * code unchanged.
 * @returns Final JavaScript text with the optional import and its following newline.
 */
export function finalizeModule(code: string, stylesheetSpecifier?: string): string {
  return finalizeModuleWithMap(code, stylesheetSpecifier).code;
}

/**
 * Half-open UTF-16 range removed from the compiled buffer during host finalization. The current
 * finalizer returns no removed ranges.
 */
export type FinalizedModuleEdit = { from: number; to: number };

/**
 * Finalizes compiled JavaScript and describes the inserted prefix for source-map generation.
 *
 * @param code - Compiled JavaScript before host finalization.
 * @param stylesheetSpecifier - Optional side-effect import specifier. A falsy value inserts no
 * import.
 * @returns Final code, the exact prefix inserted before it, and an empty list of removed ranges.
 */
export function finalizeModuleWithMap(code: string, stylesheetSpecifier?: string) {
  const stylesheetImport = stylesheetSpecifier
    ? `import ${JSON.stringify(stylesheetSpecifier)};\n`
    : "";
  return {
    code: stylesheetImport + code,
    prefix: stylesheetImport,
    removed: [] as FinalizedModuleEdit[],
  };
}
