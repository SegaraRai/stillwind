import { expect, test } from "vite-plus/test";

import { iconLicense } from "./license.ts";

test("formats supplied attribution without inventing missing metadata", () => {
  expect(iconLicense(undefined)).toBe("");
  expect(
    iconLicense({
      name: "Example Icons",
      author: {
        name: "作者",
        url: "https://example.com",
      },
      license: {
        title: "Example License",
        spdx: "MIT",
        url: "https://example.com/license",
      },
    }),
  ).toContain(
    "/*! Example Icons | 作者 | https://example.com | Example License | MIT | https://example.com/license */",
  );
  expect(
    iconLicense({
      name: "*/",
      author: {
        name: "Author",
      },
      license: {
        title: "MIT",
      },
    }),
  ).toContain("/*! * / | Author | MIT */");
});
