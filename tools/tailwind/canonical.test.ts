import { readFileSync, readdirSync } from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

import { __unstable__loadDesignSystem } from "tailwindcss";
import plugin from "tailwindcss/plugin";
import { expect, test } from "vite-plus/test";

const directory = new URL("../../fixtures/canonical/", import.meta.url);
const require = createRequire(import.meta.url);
const theme = readFileSync(require.resolve("tailwindcss/theme.css"), "utf8");

interface CanonicalFixture {
  name: string;
  css: string;
  baseTheme: string;
  cases: {
    input: string[];
    expected: string[];
    options?: { rem?: number; collapse?: boolean; logicalToPhysical?: boolean };
    contains?: boolean;
    plugin?: boolean;
  }[];
}

for (const file of readdirSync(directory).filter((name) => name.endsWith(".json"))) {
  const fixture: CanonicalFixture = JSON.parse(readFileSync(new URL(file, directory), "utf8"));
  test(`${file}: ${fixture.name}`, async () => {
    const system = await __unstable__loadDesignSystem(fixture.css, {
      async loadStylesheet() {
        return {
          path: "",
          base: "",
          content: `${theme}\n${fixture.baseTheme}\n@tailwind utilities;`,
        };
      },
      async loadModule() {
        return {
          path: "plugin.js",
          base: "",
          module: plugin((api) => {
            api.matchComponents(
              {
                myicon: () => {
                  throw new Error("Rejected speculative plugin value");
                },
              },
              {
                values: {
                  icon: fileURLToPath(import.meta.url),
                },
              },
            );
          }),
        };
      },
    });
    for (const item of fixture.cases) {
      for (const [prefix, suffix] of [
        ["", ""],
        ["focus:", ""],
        ["", "!"],
      ]) {
        const input = item.input.map((value) => `${prefix}${value}${suffix}`);
        const expected = item.expected.map((value) => `${prefix}${value}${suffix}`);
        const actual = system.canonicalizeCandidates(input, item.options);
        expect(actual, JSON.stringify(input)).toEqual(
          item.contains ? expect.arrayContaining(expected) : expected,
        );
      }
    }
  });
}
