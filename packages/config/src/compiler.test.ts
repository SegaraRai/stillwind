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
            pxToRem: false,
          },
          "unknown-utility": "deny",
        },
      },
    }),
  ).toMatchObject({
    utilityOrder: "authored",
    hashLength: 5,
    rootFontSize: 16,
    lint: {
      rules: {
        "prefer-canonical-utilities": {
          severity: "warn",
          pxToRem: false,
        },
        "unknown-utility": "deny",
      },
    },
  });
});

test("root font size accepts positive finite numbers", () => {
  expect(
    normalizeCompilerConfig({
      rootFontSize: 20.5,
    }).rootFontSize,
  ).toBe(20.5);
  for (const rootFontSize of [0, -1, Number.NaN, Number.POSITIVE_INFINITY, "16"]) {
    expect(() =>
      validateCompilerConfig({
        rootFontSize,
      }),
    ).toThrow("config.rootFontSize");
  }
});

test("browser targets normalize to an owned list and reject unsupported syntax", () => {
  expect(normalizeCompilerConfig({}).target).toEqual([]);
  expect(normalizeCompilerConfig({ target: "chrome111" }).target).toEqual(["chrome111"]);
  const target = ["firefox113", "safari17"];
  const normalized = normalizeCompilerConfig({ target });
  target[0] = "firefox112";
  expect(normalized.target).toEqual(["firefox113", "safari17"]);
  for (const target of [
    "chrome",
    "es2022",
    "safari16..2",
    "chrome65536",
    ["chrome111", "unknown1"],
  ]) {
    expect(() => validateCompilerConfig({ target })).toThrow("config.target");
  }
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
