import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";

import { expect, test } from "vite-plus/test";

import {
  buildCssTargetFixture,
  cssTargetCases,
  cssTargetExpectedRoot,
} from "./css-target-fixture.ts";
import { compileContext, createTestServer } from "./vite-server-fixture.ts";

test.each(cssTargetCases)("$name", async (testCase) => {
  const result = await buildCssTargetFixture(testCase);
  for (const kind of ["document", "fragment"] as const) {
    const expected = await readFile(
      path.join(cssTargetExpectedRoot, `${testCase.expected}.${kind}.css`),
      "utf8",
    );
    expect(result[kind] + "\n").toBe(expected);
    expect(result[kind]).not.toMatch(/__stillwind_resource|@fs/);
    expect(result[kind]).toMatch(/icon-[\w-]+\.svg/);
  }
});

test.each([{ target: undefined }, { target: [] }])(
  "development client and SSR retain the target override $target",
  async ({ target }) => {
    const source = 'import {style} from "stillwind/react"; export const props = style("current");';
    const { root, project, server } = await createTestServer(
      { target },
      "@utility current{background-color:--compat-color(currentcolor,50%);} @stillwind-slot utilities;",
      {
        build: { cssTarget: "chrome120" },
        environments: { ssr: { build: { target: "node22" } } },
      },
    );
    const entry = path.join(root, "entry.ts");
    await writeFile(entry, source);
    expect(project.config.target).toEqual(target ?? ["chrome120"]);
    for (const name of ["client", "ssr"] as const) {
      const context = compileContext(server, name);
      expect(await project.compile(name, source, entry, context)).toBeDefined();
      const plan = await project.documentPlan(name, context, true);
      expect(/@supports\s*\(color:\s*color-mix/.test(plan.css)).toBe(target !== undefined);
    }
  },
);

test("disabling CSS minification preserves target inheritance in both emission modes", async () => {
  for (const emission of ["single", "split"] as const) {
    const result = await buildCssTargetFixture({
      name: emission,
      expected: "modern",
      emission,
      build: { cssTarget: "chrome120", cssMinify: false },
    });
    for (const css of [result.document, result.fragment]) {
      expect(css).toMatch(/color-mix\(in oklab,\s*currentcolor 50%,\s*transparent\)/);
      expect(css).not.toMatch(/@supports\s*\(color:\s*color-mix/);
      expect(css).toContain("rgba(255, 0, 0, 0.5)");
    }
  }
});
