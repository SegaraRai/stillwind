import type {} from "@volar/typescript";
import { NodeTypes, type RootNode, type TemplateChildNode } from "@vue/compiler-dom";
import { parse } from "@vue/compiler-sfc";
import * as vue from "@vue/language-core";
import ts from "typescript-6";

import { originalRange, PreparationError, type ScriptRegion } from "../preparation.ts";
import type { SourceProjection } from "../protocol.ts";
import { converterIdentity, sourceRegions } from "./common.ts";

/**
 * Converts a Vue component to a service script and records original scopes and attribute quotes.
 *
 * @param source - Complete original Vue single-file component text.
 * @param filename - Component filename used by the parser and the Vue language plugin.
 * @returns Generated TypeScript or TSX with copied Volar mappings and any required global
 * declarations. The setup realm inherits ordinary script imports when both scripts exist.
 * Template edits use the setup realm, or the ordinary script realm when only that script exists.
 * The created virtual code is disposed before this function returns or throws after its creation.
 * @throws {PreparationError} If component parsing fails, no virtual code or service script is
 * produced, or original script ranges are invalid.
 * @throws If converter metadata cannot be loaded or the language plugin otherwise fails.
 */
export function convert(source: string, filename: string): SourceProjection {
  const options = vue.getDefaultCompilerOptions();
  const converter = converterIdentity("@vue/language-core", {
    options,
    typescript: ts.version,
  });

  const parsed = parse(source, { filename });
  if (parsed.errors.length) {
    const error = parsed.errors[0];
    throw new PreparationError(
      {
        kind: "syntax",
        detail: String(error),
      },
      converter,
      typeof error === "object" && "loc" in error
        ? originalRange(source, error.loc?.start.offset, error.loc?.end.offset)
        : undefined,
    );
  }

  const globals = (
    vue as typeof vue & {
      /** Optional declaration generator. Omission leaves checker globals to the caller. */
      generateGlobalTypes?: (compilerOptions: typeof options) => string;
    }
  ).generateGlobalTypes?.(options);
  const plugin = vue.createVueLanguagePlugin<string>(ts, {}, options, (id) => id);

  const root = plugin.createVirtualCode?.(filename, "vue", ts.ScriptSnapshot.fromString(source), {
    getAssociatedScript: () => undefined,
  });
  if (!root) {
    throw new PreparationError(
      {
        kind: "projection",
        detail: "Vue converter did not produce a projection",
      },
      converter,
    );
  }

  try {
    const service = plugin.typescript?.getServiceScript(root);
    if (!service) {
      throw new PreparationError(
        {
          kind: "projection",
          detail: "Vue converter did not produce a service script",
        },
        converter,
      );
    }

    const scripts: ScriptRegion[] = [];
    const { script, scriptSetup } = parsed.descriptor;
    if (script) {
      scripts.push({
        id: 0,
        start: script.loc.start.offset,
        end: script.loc.end.offset,
      });
    }
    if (scriptSetup) {
      scripts.push({
        id: 1,
        start: scriptSetup.loc.start.offset,
        end: scriptSetup.loc.end.offset,
        ...(script ? { parent: 0 } : {}),
      });
    }

    return {
      ...(globals
        ? {
            checkerGlobals: globals,
          }
        : {}),
      syntax: {
        language: service.extension === ".tsx" ? "tsx" : "ts",
        moduleKind: "module",
      },
      code: service.code.snapshot.getText(0, service.code.snapshot.getLength()),
      map: {
        kind: "volar",
        mappings: service.code.mappings.map(
          ({ sourceOffsets, generatedOffsets, lengths, generatedLengths }) => ({
            sourceOffsets: [...sourceOffsets],
            generatedOffsets: [...generatedOffsets],
            lengths: [...lengths],
            ...(generatedLengths
              ? {
                  generatedLengths: [...generatedLengths],
                }
              : {}),
          }),
        ),
      },
      converter,
      importRealms: scripts,
      editableRegions: quotedRegions(
        source,
        parsed.descriptor.template?.ast,
        sourceRegions(source, scripts, scriptSetup ? 1 : script ? 0 : undefined, converter),
      ),
    };
  } finally {
    plugin.disposeVirtualCode?.(filename, root);
  }
}

/**
 * Splits editable ranges around Vue directive expressions to retain their surrounding quotes.
 *
 * @param source - Original component text used to inspect the characters around each expression.
 * @param root - Parsed template AST, or undefined when no template is available.
 * @param regions - Sorted editable ranges to update in place. Expressions must be fully contained
 * in one range to receive quote context.
 * @returns The same array with contained, quoted directive expressions split into separate ranges.
 * If the template is absent, the array is unchanged.
 */
function quotedRegions(
  source: string,
  root: RootNode | undefined,
  regions: SourceProjection["editableRegions"],
) {
  if (!root) {
    return regions;
  }

  const quotes: {
    /** Inclusive original UTF-16 offset of the directive expression, excluding its quote. */
    start: number;
    /** Exclusive original UTF-16 offset, before the closing quote. */
    end: number;
    /** HTML quote surrounding the expression, retained when compiler edits are inserted. */
    attributeQuote: "single" | "double";
  }[] = [];

  /**
   * Records directive expressions whose original text has matching surrounding quotes.
   *
   * @param node - Template subtree parsed from `source`. Root and element children are traversed,
   * and matching expression ranges are appended to `quotes` without changing the AST.
   */
  function visit(node: RootNode | TemplateChildNode) {
    if (node.type === NodeTypes.ELEMENT) {
      for (const prop of node.props) {
        if (prop.type !== NodeTypes.DIRECTIVE || !prop.exp) {
          continue;
        }
        const start = prop.exp.loc.start.offset;
        const end = prop.exp.loc.end.offset;
        const quote = source[start - 1];
        if ((quote === '"' || quote === "'") && source[end] === quote) {
          quotes.push({
            start,
            end,
            attributeQuote: quote === '"' ? "double" : "single",
          });
        }
      }
    }
    if (node.type === NodeTypes.ROOT || node.type === NodeTypes.ELEMENT) {
      for (const child of node.children) {
        visit(child);
      }
    }
  }
  visit(root);

  for (const quote of quotes) {
    const index = regions.findIndex(
      (region) => region.start <= quote.start && quote.end <= region.end,
    );
    if (index < 0) {
      continue;
    }
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
