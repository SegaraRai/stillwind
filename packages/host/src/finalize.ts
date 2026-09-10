/** Adds the runtime helpers (and optionally virtual CSS) required by compiled output. */
export function finalizeReactModule(code: string, stylesheetSpecifier?: string): string {
  return finalizeReactModuleWithMap(code, stylesheetSpecifier).code;
}

export type FinalizedModuleEdit = { from: number; to: number };

/** Finalizes compiled output while retaining edits needed to remap source spans. */
export function finalizeReactModuleWithMap(code: string, stylesheetSpecifier?: string) {
  const macroImportPattern =
    /^\s*import\s+(?:[^;]+?)\s+from\s+["']stillwind\/(react|solid)["'];?\s*$/gm;
  const removed = [...code.matchAll(macroImportPattern)].map((match) => ({
    from: match.index,
    to: match.index + match[0].length,
  }));
  const withoutMacroImport = code.replace(macroImportPattern, "");
  const frontend = /["']stillwind\/solid["']/.test(code) ? "solid" : "react";
  const runtimeImport = renderRuntimeImport(withoutMacroImport, frontend);
  const stylesheetImport = stylesheetSpecifier
    ? `import ${JSON.stringify(stylesheetSpecifier)};\n`
    : "";
  const prefix = runtimeImport + stylesheetImport;
  return { code: prefix + withoutMacroImport, prefix, removed };
}

function renderRuntimeImport(code: string, frontend: "react" | "solid"): string {
  const names = [
    "stillwindAnimation",
    "stillwindAssetClass",
    "stillwindClsx",
    "stillwindHandleClass",
    "stillwindRef",
    "stillwindScope",
    "stillwindViewTransition",
    "stillwindViewTransitionStyle",
  ].filter((name) => code.includes(`${name}(`));
  return names.length > 0
    ? `import { ${names.join(", ")} } from "stillwind/${frontend}/runtime";\n`
    : "";
}
