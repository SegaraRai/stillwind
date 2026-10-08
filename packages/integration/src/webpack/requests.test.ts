import path from "node:path";

import type { BuildConfig } from "@stillwind/config";
import { expect, test } from "vite-plus/test";

import { classifyRequest, requestOptions, virtualRequest } from "./requests.ts";
import { loaderOptionsId } from "./rules.ts";

test.each([
  ["module.ts", "?variant=one", "?variant=one", false],
  ["module.ts", "?stillwind.css", "", false],
  ["module.ts", "?stillwind-module-css", "", false],
  ["component.vue", "?vue&type=script", "", false],
  ["component.svelte", "?svelte&type=style", "", false],
  ["component.astro", "?astro&type=script", "", false],
  ["module.ts", "?raw=true", "?raw=true", true],
  ["module.ts", "?url", "?url", true],
] as const)("classifies %s%s for compilation", (file, query, suffix, passthrough) => {
  const resource = path.resolve(file);
  const request = classifyRequest(resource, query);
  expect(request.id).toBe(resource + suffix);
  expect(request.passthrough).toBe(passthrough);
});

test("SFC virtual requests carry reconstructible configuration", () => {
  const config: BuildConfig = {
    css: "./root.css",
    automatic: false,
  };
  const resource = path.resolve("component.vue");
  const loader = path.resolve("loader.js");
  const request = virtualRequest(resource, true, config, loader, "stillwind.css");
  const encoded = request.match(/\?stillwindOptions=([^!]+)!/)?.[1];
  expect(encoded).toBeTruthy();
  expect(requestOptions({ stillwindOptions: encoded })).toEqual(config);
  expect(request.endsWith(`${resource.replaceAll("\\", "/")}?stillwind.css`)).toBe(true);
});

test("SFC naming callbacks use the registered rule configuration", () => {
  const config: BuildConfig = {
    css: "./root.css",
    naming: {
      utilityClass: (_context, hash) => "custom-" + hash.toString("hex"),
    },
    namingRevision: "test",
  };
  const resource = path.resolve("component.svelte");
  const loader = path.resolve("loader.js");
  expect(virtualRequest(resource, true, config, loader, "stillwind-module-css")).toBe(
    `!!${loader.replaceAll("\\", "/")}??${loaderOptionsId(config)}!${resource.replaceAll("\\", "/")}?stillwind-module-css`,
  );
  expect(requestOptions(config)).toBe(config);
  expect(virtualRequest(path.resolve("module.ts"), false, config, loader, "stillwind.css")).toBe(
    "./module.ts?stillwind.css",
  );
});
