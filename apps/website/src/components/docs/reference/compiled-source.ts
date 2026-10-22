import type { MdastPluginInput } from "@cloudflare/nimbus-docs/types";

export const compiledSourcePlugin = {
  name: "stillwind-compiled-example",
  mdxJsxFlowElement(node, context) {
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
} satisfies MdastPluginInput;

export function compiledSource(markdown: string) {
  const match = /^```tsx?\n([\s\S]*?)\n```/m.exec(markdown);
  if (!match) {
    throw new Error("CompiledExample requires a TypeScript code fence");
  }
  return match[1]!;
}
