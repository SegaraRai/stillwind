import { expect, test } from "vite-plus/test";

import { generatePackageIndex, transformPackageMarkdown } from "./package-docs";

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

test("builds a bundled llms.txt index grouped by section with relative paths", () => {
  const pages = new Map([
    [
      "guides/style.md",
      '---\ntitle: "Style basics"\ndescription: "Write utilities in style()."\n---\n\nBody\n',
    ],
    ["index.md", '---\ntitle: "Getting started"\ndescription: "Set up Stillwind."\n---\n\nBody\n'],
    ["cli.md", '---\ntitle: "CLI"\ndescription: "Run lint, fmt, and inspect."\n---\n\nBody\n'],
    ["macros/index.md", '---\ntitle: "Macros"\ndescription: "Compile-time macros."\n---\n\nBody\n'],
    ["macros/style.md", '---\ntitle: "style()"\ndescription: "Compile utilities."\n---\n\nBody\n'],
  ]);

  expect(generatePackageIndex(pages)).toBe(
    [
      "# Stillwind documentation",
      "",
      "Index of the Markdown pages bundled in `docs/`. Paths are relative to this file.",
      "",
      "## Pages",
      "",
      "- [Getting started](index.md): Set up Stillwind.",
      "- [CLI](cli.md): Run lint, fmt, and inspect.",
      "",
      "## guides",
      "",
      "- [Style basics](guides/style.md): Write utilities in style().",
      "",
      "## macros",
      "",
      "- [Macros](macros/index.md): Compile-time macros.",
      "- [style()](macros/style.md): Compile utilities.",
      "",
    ].join("\n"),
  );
});
