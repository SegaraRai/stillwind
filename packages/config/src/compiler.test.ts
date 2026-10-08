import { expect, test } from "vite-plus/test";

import { normalizeCompilerConfig, validateCompilerConfig } from "./compiler.ts";

test("compiler normalization applies defaults and retains lint options", () => {
  expect(
    normalizeCompilerConfig({
      lint: {
        rules: {
          "canonical-utilities": {
            rootFontSize: 20,
          },
          "unknown-utility": "deny",
        },
      },
    }),
  ).toMatchObject({
    utilityOrder: "authored",
    hashLength: 5,
    lint: {
      rules: {
        "canonical-utilities": {
          severity: "warn",
          pxToRem: true,
          rootFontSize: 20,
        },
        "unknown-utility": "deny",
      },
    },
  });
});

test("sorting targets are validated independently of lint reporting", () => {
  const config = {
    lint: {
      rules: {
        "sort-utilities": "off",
      },
    },
    sort: {
      stringLiterals: false,
      arrays: true,
      attributes: true,
    },
  };
  expect(() => validateCompilerConfig(config)).not.toThrow();
  expect(() =>
    validateCompilerConfig({
      sort: {
        attributes: "yes",
      },
    }),
  ).toThrow();
});
