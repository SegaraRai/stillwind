import { expect, test } from "vite-plus/test";

import { registrationManifest } from "./manifest.ts";

test("registers shared CSS once while keeping entry metadata and other assets", () => {
  const manifest = {
    first: {
      file: "first.js",
      css: ["shared.css"],
      assets: ["image.svg"],
    },
    second: {
      file: "second.js",
      imports: ["first"],
      css: ["shared.css", "route.css"],
      assets: ["image.svg"],
    },
  };
  expect(JSON.parse(registrationManifest(JSON.stringify(manifest)))).toEqual({
    first: {
      file: "first.js",
      css: ["shared.css"],
      assets: ["image.svg"],
    },
    second: {
      file: "second.js",
      imports: ["first"],
      css: ["route.css"],
      assets: [],
    },
  });
});
