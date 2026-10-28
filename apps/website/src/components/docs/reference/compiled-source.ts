import type { MdastPluginInput } from "@cloudflare/nimbus-docs/types";

import type { DiagnosticExample } from "./diagnostic-example";

export const compiledSourcePlugin = (
  diagnostics: ReadonlyMap<string, DiagnosticExample>,
  revision: string,
): MdastPluginInput => ({
  name: `stillwind-reference-examples-${revision}`,
  mdxJsxFlowElement(node, context) {
    if (node.name === "DiagnosticExample") {
      const code = node.attributes.find(
        (attribute) => attribute.type === "mdxJsxAttribute" && attribute.name === "code",
      );
      const example =
        code?.type === "mdxJsxAttribute" && typeof code.value === "string"
          ? diagnostics.get(code.value)
          : undefined;
      if (!example) {
        throw new Error("DiagnosticExample requires a known lint rule or error code");
      }
      context.replaceNode(node, {
        ...node,
        attributes: [
          ...node.attributes,
          {
            type: "mdxJsxAttribute",
            name: "example",
            value: { type: "mdxJsxAttributeValueExpression", value: JSON.stringify(example) },
          },
        ],
        children: [],
      });
      return;
    }
    if (node.name !== "CompiledExample") {
      return;
    }
    const blocks = node.children.filter((child) => child.type === "code");
    const scripts = blocks.filter((block) => ["tsx", "ts"].includes(block.lang ?? ""));
    const stylesheets = blocks.filter((block) => block.lang === "css");
    if (
      scripts.length > 1 ||
      stylesheets.length > 1 ||
      scripts.length + stylesheets.length !== blocks.length ||
      (!scripts.length && !stylesheets.length)
    ) {
      throw new Error(
        "CompiledExample requires one TypeScript code fence, one CSS code fence, or one of each",
      );
    }
    const attribute = (name: string, value: string) => ({
      type: "mdxJsxAttribute" as const,
      name,
      value: {
        type: "mdxJsxAttributeValueExpression" as const,
        value: JSON.stringify(value),
      },
    });
    context.replaceNode(node, {
      type: "mdxJsxFlowElement",
      name: node.name,
      attributes: [
        ...node.attributes,
        ...scripts.map((block) => attribute("source", block.value)),
        ...stylesheets.map((block) => attribute("stylesheet", block.value)),
      ],
      children: [],
    });
  },
});

export function compiledSource(markdown: string) {
  const fence = (languages: string) =>
    new RegExp(String.raw`^\`\`\`(?:${languages})\n([\s\S]*?)\n\`\`\``, "m").exec(markdown)?.[1];
  const input = { source: fence("tsx|ts"), stylesheet: fence("css") };
  if (input.source === undefined && input.stylesheet === undefined) {
    throw new Error("CompiledExample requires a TypeScript or CSS code fence");
  }
  return input;
}
