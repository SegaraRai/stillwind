import type {} from "@volar/typescript";
import { NodeTypes, type RootNode, type TemplateChildNode } from "@vue/compiler-dom";
import { parse } from "@vue/compiler-sfc";
import * as vue from "@vue/language-core";
import ts from "typescript";

import {
  converterIdentity,
  sourceRegions,
  type ConvertedFramework,
  type ScriptRegion,
} from "./index.ts";

export function convert(source: string, filename: string): ConvertedFramework {
  const parsed = parse(source, { filename });
  if (parsed.errors.length) throw new Error(parsed.errors.map(String).join("\n"));
  const options = vue.getDefaultCompilerOptions();
  const plugin = vue.createVueLanguagePlugin<string>(ts, {}, options, (id) => id);
  const root = plugin.createVirtualCode?.(filename, "vue", ts.ScriptSnapshot.fromString(source), {
    getAssociatedScript: () => undefined,
  });
  if (!root) throw new Error(`Vue converter did not produce a projection for ${filename}`);
  try {
    const service = plugin.typescript?.getServiceScript(root);
    if (!service) throw new Error(`Vue converter did not produce a service script for ${filename}`);
    const scripts: ScriptRegion[] = [];
    const { script, scriptSetup } = parsed.descriptor;
    if (script) scripts.push({ id: 0, start: script.loc.start.offset, end: script.loc.end.offset });
    if (scriptSetup)
      scripts.push({ id: 1, start: scriptSetup.loc.start.offset, end: scriptSetup.loc.end.offset });
    return {
      framework: "Vue",
      language: service.extension === ".tsx" ? "tsx" : "ts",
      source: service.code.snapshot.getText(0, service.code.snapshot.getLength()),
      fw2js_map: {
        kind: "volar",
        mappings: service.code.mappings.map(
          ({ sourceOffsets, generatedOffsets, lengths, generatedLengths }) => ({
            sourceOffsets: [...sourceOffsets],
            generatedOffsets: [...generatedOffsets],
            lengths: [...lengths],
            ...(generatedLengths ? { generatedLengths: [...generatedLengths] } : {}),
          }),
        ),
      },
      converter: converterIdentity("@vue/language-core", { options, typescript: ts.version }),
      import_realms: scripts,
      regions: quotedRegions(
        source,
        parsed.descriptor.template?.ast,
        sourceRegions(source, scripts, scriptSetup ? 1 : script ? 0 : undefined),
      ),
    };
  } finally {
    plugin.disposeVirtualCode?.(filename, root);
  }
}

function quotedRegions(
  source: string,
  root: RootNode | undefined,
  regions: ConvertedFramework["regions"],
) {
  if (!root) return regions;
  const quotes: Array<{ start: number; end: number; attribute_quote: "single" | "double" }> = [];
  function visit(node: RootNode | TemplateChildNode) {
    if (node.type === NodeTypes.ELEMENT) {
      for (const prop of node.props) {
        if (prop.type !== NodeTypes.DIRECTIVE || !prop.exp) continue;
        const start = prop.exp.loc.start.offset,
          end = prop.exp.loc.end.offset;
        const quote = source[start - 1];
        if ((quote === '"' || quote === "'") && source[end] === quote) {
          quotes.push({ start, end, attribute_quote: quote === '"' ? "double" : "single" });
        }
      }
    }
    if (node.type === NodeTypes.ROOT || node.type === NodeTypes.ELEMENT)
      for (const child of node.children) visit(child);
  }
  visit(root);
  for (const quote of quotes) {
    const index = regions.findIndex(
      (region) => region.start <= quote.start && quote.end <= region.end,
    );
    if (index < 0) continue;
    const region = regions[index]!;
    regions.splice(
      index,
      1,
      ...(region.start < quote.start ? [{ ...region, end: quote.start }] : []),
      { ...region, ...quote },
      ...(quote.end < region.end ? [{ ...region, start: quote.end }] : []),
    );
  }
  return regions;
}
