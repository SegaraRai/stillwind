import { expect, test } from "vite-plus/test";

import { normalizeCompilerConfig, validateCompilerConfig } from "./compiler.ts";

test.each([null, [], { sort: [] }, { naming: [] }])(
  "compiler options require objects: %j",
  (value) => {
    expect(() => validateCompilerConfig(value)).toThrow("must be an object");
  },
);

test("compiler normalization applies defaults and retains lint options", () => {
  expect(
    normalizeCompilerConfig({
      lint: {
        rules: {
          "prefer-canonical-utilities": {
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
        "prefer-canonical-utilities": {
          severity: "warn",
          pxToRem: true,
          rootFontSize: 20,
        },
        "unknown-utility": "deny",
      },
    },
  });
});

test("false disables all sorting targets and remains disabled after normalization", () => {
  const normalized = normalizeCompilerConfig({
    sort: false,
  });
  expect(normalized.sort).toEqual({
    stringLiterals: false,
    arrays: false,
    attributes: false,
  });
  expect(normalizeCompilerConfig(normalized).sort).toEqual(normalized.sort);
});

test.each([
  {
    lint: true,
  },
  {
    sort: true,
  },
])("only false disables a configuration: %j", (config) => {
  expect(() => validateCompilerConfig(config)).toThrow();
});

test("sorting targets are validated independently of lint reporting", () => {
  const config = {
    lint: {
      rules: {
        "prefer-sorted-utilities": "off",
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
