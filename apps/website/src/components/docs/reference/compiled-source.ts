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
    if (blocks.length !== 1 || !["tsx", "ts"].includes(blocks[0]!.lang ?? "")) {
      throw new Error("CompiledExample requires one TypeScript code fence");
    }
    context.replaceNode(node, {
      type: "mdxJsxFlowElement",
      name: node.name,
      attributes: [
        ...node.attributes,
        {
          type: "mdxJsxAttribute",
          name: "source",
          value: {
            type: "mdxJsxAttributeValueExpression",
            value: JSON.stringify(blocks[0]!.value),
          },
        },
      ],
      children: [],
    });
  },
});

export function compiledSource(markdown: string) {
  const match = /^```tsx?\n([\s\S]*?)\n```/m.exec(markdown);
  if (!match) {
    throw new Error("CompiledExample requires a TypeScript code fence");
  }
  return match[1]!;
}
