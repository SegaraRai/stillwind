/** Adds optional virtual CSS to output whose imports are finalized by native. */
export function finalizeReactModule(code: string, stylesheetSpecifier?: string): string {
  return finalizeReactModuleWithMap(code, stylesheetSpecifier).code;
}

export type FinalizedModuleEdit = { from: number; to: number };

/** Finalizes compiled output while retaining edits needed to remap source spans. */
export function finalizeReactModuleWithMap(code: string, stylesheetSpecifier?: string) {
  const stylesheetImport = stylesheetSpecifier
    ? `import ${JSON.stringify(stylesheetSpecifier)};\n`
    : "";
  return {
    code: stylesheetImport + code,
    prefix: stylesheetImport,
    removed: [] as FinalizedModuleEdit[],
  };
}
