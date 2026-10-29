import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

import { expect, test } from "vite-plus/test";

import { browserSuites, checkBrowserSuites, planSites, requireSiteChecks } from "./ci.ts";

const packages = [
  { name: "@stillwind/native", directory: "packages/native", dependencies: [] },
  { name: "stillwind", directory: "packages/core", dependencies: ["@stillwind/native"] },
  {
    name: "@stillwind-internal/apps-website",
    directory: "apps/website",
    dependencies: ["stillwind"],
  },
  { name: "@stillwind-internal/apps-play", directory: "apps/play", dependencies: ["stillwind"] },
  { name: "@stillwind-internal/e2e-play", directory: "e2e/play", dependencies: [] },
  { name: "@stillwind-internal/tools-sites", directory: "tools/sites", dependencies: [] },
  { name: "unrelated", directory: "packages/unrelated", dependencies: [] },
];

test("website changes omit playground checks and unrelated packages omit all site work", () => {
  expect(planSites(packages, ["apps/website/src/pages/index.astro"])).toMatchObject({
    sites: ["website"],
    browser: [],
    deploy: ["website"],
    full: false,
  });
  expect(planSites(packages, ["packages/unrelated/src/index.ts"])).toMatchObject({
    sites: [],
    browser: [],
    deploy: [],
    full: false,
  });
  expect(planSites(packages, ["docs/TODO.md", "apps/play/DEVELOPMENT.md"])).toMatchObject({
    sites: [],
    browser: [],
    deploy: [],
  });
});

test.each([
  "packages/core/src/index.ts",
  "crates/css/src/lib.rs",
  "tools/tailwind/generate.ts",
  "tools/sites/prepare.ts",
  "pnpm-lock.yaml",
  ".github/workflows/sites.yml",
])("shared input %s selects both sites and all browser scenarios", (file) => {
  expect(planSites(packages, [file])).toMatchObject({
    sites: ["website", "play"],
    browser: [...browserSuites],
    deploy: ["website", "play"],
  });
});

test("manual or incomplete comparisons select every check", () => {
  expect(planSites(packages, null)).toMatchObject({
    sites: ["website", "play"],
    browser: [...browserSuites],
    deploy: ["website", "play"],
    full: true,
  });
});

test.each(browserSuites)(
  "a change to the %s scenario builds its fixture without deploying",
  (suite) => {
    expect(planSites(packages, [`e2e/play/src/${suite}.test.ts`])).toMatchObject({
      sites: ["play"],
      browser: [suite],
      deploy: [],
    });
  },
);

test("shared browser infrastructure selects all scenarios, and unit tests do not deploy", () => {
  expect(planSites(packages, ["e2e/play/src/playground.ts"])).toMatchObject({
    sites: ["play"],
    browser: [...browserSuites],
    deploy: [],
  });
  expect(planSites(packages, ["apps/website/src/page.test.ts"])).toMatchObject({
    sites: ["website"],
    browser: [],
    deploy: [],
  });
});

test.each([
  { sites: [], browser: [], deploy: [] },
  { sites: ["website"], browser: [], deploy: ["website"] },
  { sites: ["play"], browser: ["hmr"], deploy: [] },
])("the required gate accepts planned skips: %j", (selection) => {
  const needs = {
    changes: {
      result: "success",
      outputs: { plan: JSON.stringify({ ...selection, full: false, affectedPackages: [] }) },
    },
    build: { result: selection.sites.length ? "success" : "skipped" },
    browser: { result: selection.browser.length ? "success" : "skipped" },
  };
  expect(() => requireSiteChecks(needs)).not.toThrow();
  for (const job of ["build", "browser", "changes"] as const) {
    for (const result of ["failure", "cancelled"]) {
      expect(() => requireSiteChecks({ ...needs, [job]: { ...needs[job], result } })).toThrow();
    }
  }
  expect(() =>
    requireSiteChecks({
      ...needs,
      build: { result: needs.build.result === "success" ? "skipped" : "success" },
    }),
  ).toThrow();
});

test.each([
  undefined,
  "null",
  "{}",
  '{"sites":["unknown"],"browser":[],"deploy":[],"full":false}',
  '{"sites":[],"browser":["hmr"],"deploy":[],"full":false}',
])("the gate rejects a missing or invalid plan: %s", (plan) => {
  expect(() =>
    requireSiteChecks({
      changes: { result: "success", outputs: plan === undefined ? {} : { plan } },
      build: { result: "skipped" },
      browser: { result: "skipped" },
    }),
  ).toThrow();
});

test("the matrix covers every browser test, including tests added in subdirectories", async ({
  onTestFinished,
}) => {
  const directory = await mkdtemp(path.join(tmpdir(), "stillwind-browser-matrix-"));
  onTestFinished(() => rm(directory, { recursive: true, force: true }));
  for (const suite of browserSuites) {
    await writeFile(path.join(directory, `${suite}.test.ts`), "");
  }
  await expect(checkBrowserSuites(directory)).resolves.toBeUndefined();
  await mkdir(path.join(directory, "nested"));
  await writeFile(path.join(directory, "nested/new.test.ts"), "");
  await expect(checkBrowserSuites(directory)).rejects.toThrow("Update browserSuites");
});
