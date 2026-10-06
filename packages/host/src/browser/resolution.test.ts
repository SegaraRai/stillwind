import { expect, test } from "vite-plus/test";

import { resolveVirtualModule } from "./resolution.ts";

test("virtual resolution handles local files, package exports, and CSS conditions", () => {
  const files = new Map([
    ["/workspace/component.tsx", ""],
    ["/workspace/button/index.ts", ""],
    [
      "/workspace/node_modules/library/package.json",
      JSON.stringify({
        exports: {
          ".": {
            browser: "./browser.js",
            default: "./node.js",
          },
          "./theme": {
            style: "./theme.css",
            default: "./theme.js",
          },
        },
      }),
    ],
    ["/workspace/node_modules/library/browser.js", ""],
    ["/workspace/node_modules/library/theme.css", ""],
  ]);
  expect(resolveVirtualModule(files, "./button", "/workspace/component.tsx")).toBe(
    "/workspace/button/index.ts",
  );
  expect(resolveVirtualModule(files, "library", "/workspace/component.tsx")).toBe(
    "/workspace/node_modules/library/browser.js",
  );
  expect(resolveVirtualModule(files, "library/theme", "/workspace/component.tsx", "style")).toBe(
    "/workspace/node_modules/library/theme.css",
  );
  expect(
    resolveVirtualModule(files, "library/private", "/workspace/component.tsx"),
  ).toBeUndefined();
});

test("virtual exports preserve condition order and select specific wildcard subpaths", () => {
  const files = new Map([
    ["/workspace/value.ts", ""],
    [
      "/workspace/node_modules/library/package.json",
      JSON.stringify({
        exports: {
          ".": { default: "./fallback.js", browser: "./browser.js" },
          "./*": "./general/*.js",
          "./feature/*": "./specific/*.js",
          "./escape/*": "../*.js",
        },
      }),
    ],
    ["/workspace/node_modules/library/fallback.js", ""],
    ["/workspace/node_modules/library/browser.js", ""],
    ["/workspace/node_modules/library/specific/item.js", ""],
  ]);
  expect(resolveVirtualModule(files, "./value.js", "/workspace/entry.ts")).toBe(
    "/workspace/value.ts",
  );
  expect(resolveVirtualModule(files, "library", "/workspace/entry.ts")).toBe(
    "/workspace/node_modules/library/fallback.js",
  );
  expect(resolveVirtualModule(files, "library/feature/item", "/workspace/entry.ts")).toBe(
    "/workspace/node_modules/library/specific/item.js",
  );
  expect(resolveVirtualModule(files, "library/escape/item", "/workspace/entry.ts")).toBeUndefined();
});
