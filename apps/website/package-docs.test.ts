import { expect, test } from "vite-plus/test";

import { transformPackageMarkdown } from "./package-docs";

test.each(["/docs", "/docs/"])(
  "converts the docs root %s and nested links to package files",
  (root) => {
    const markdown = `> Documentation Index
> Fetch the complete documentation index at: https://preview.example/llms.txt

[Getting started](${root}#installation)
[Style](/docs/guides/style#conditions)

Source: https://preview.example/docs/guides/design-systems`;
    const files = new Map([
      ["", "index.md"],
      ["guides/style", "guides/style.md"],
    ]);

    expect(transformPackageMarkdown(markdown, "guides/design-systems.md", files)).toBe(
      "[Getting started](../index.md#installation)\n[Style](style.md#conditions)\n",
    );
  },
);

test.each(["https://stillwind.dev", "https://preview.example"])(
  "keeps online file links on the production origin when built at %s",
  (origin) => {
    const markdown = `> Documentation Index
> Fetch the complete documentation index at: ${origin}/llms.txt

[Index](/docs/llms.txt)
[Full documentation](/llms-full.txt)

Source: ${origin}/docs/agents/index.mdx`;

    expect(transformPackageMarkdown(markdown, "agents.md", new Map())).toBe(
      "[Index](https://stillwind.dev/docs/llms.txt)\n[Full documentation](https://stillwind.dev/llms-full.txt)\n",
    );
  },
);
