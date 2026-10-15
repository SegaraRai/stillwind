import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";

import { compileTailwindCss } from "@stillwind-internal/tools-tailwind/compiler";
import { parseMarkdownFixtureCases } from "@stillwind-internal/tools-tailwind/markdown-fixtures";
import { NativeDesignSystemLoader } from "@stillwind/native";
import { generate, parse, walk } from "css-tree";
import { expect, test } from "vite-plus/test";

import { comparisonCss } from "../comparison.ts";
import { stillwindFixtureCss } from "../stillwind-css.ts";

const fixtureRoot = resolve(import.meta.dirname, "../../../fixtures/tailwind-v4");
const fixtures = parseMarkdownFixtureCases(
  readFileSync(join(fixtureRoot, "cases/color-fallback.fixture.md"), "utf8").replaceAll(
    "\r\n",
    "\n",
  ),
);
const cases = fixtures.flatMap(({ name, input, candidates }) =>
  candidates.split(" ").map((candidate) => ({ name, css: input, candidate })),
);

test.each(cases)(
  "$name matches Tailwind without a target: $candidate",
  async ({ css, candidate }) => {
    const expected = await compileTailwindCss({
      css,
      candidates: [candidate],
      base: fixtureRoot,
    });
    const rules: string[] = [];
    walk(parse(expected), {
      visit: "Atrule",
      enter(node) {
        if (
          node.name === "layer" &&
          node.prelude &&
          generate(node.prelude) === "utilities" &&
          node.block
        ) {
          walk(node.block, {
            visit: "ClassSelector",
            enter(selector) {
              selector.name = "x";
            },
          });
          rules.push(...node.block.children.toArray().map((rule) => generate(rule)));
        }
      },
    });
    const loader = new NativeDesignSystemLoader();
    loader.provide(0, "test.css", stillwindFixtureCss(css));
    const actual = loader.editorQuery().previewCandidate(candidate);
    expect(actual).not.toBeNull();
    expect(comparisonCss(actual!.replaceAll("&", ".x"))).toBe(comparisonCss(rules.join("")));
  },
);
