import { expect, test } from "vite-plus/test";

import { convert } from "./astro.ts";

test.each([
  ["a statement", '---\nconst a = "あ";\n---\n<p>x</p>\n'],
  ["a line comment", '---\nimport { style } from "stillwind/astro";\n// あ\n---\n<p>x</p>\n'],
  ["a line comment before an indented fence", "---\nconst a = 1;\n// c\n  ---\n<p>x</p>\n"],
])("the frontmatter realm covers the body when the body ends with %s", async (_, source) => {
  const projection = await convert(source, "/src/Component.astro");

  const realm = projection.importRealms[0];
  const start = source.indexOf("---") + 3;
  const end = source.lastIndexOf("---");
  expect(realm).toEqual({
    id: 0,
    start,
    end,
  });
});

test("fences after leading template text form no frontmatter realm", async () => {
  const projection = await convert(
    "<!-- c -->\n---\nconst a = 1;\n---\n<p>x</p>\n",
    "/src/Component.astro",
  );

  expect(projection.importRealms).toEqual([]);
});
