import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import path from "node:path";

import { expect, onTestFinished, test } from "vite-plus/test";

import { mergeConfig, validateConfig, type ProjectionProvider, type BaseConfig } from "./index.ts";
import { findConfiguration, loadConfiguration, resolveIntegrationConfig } from "./node.ts";

async function fixture() {
  const root = await mkdtemp(path.join(import.meta.dirname, ".test-config-"));
  onTestFinished(() => rm(root, { recursive: true, force: true }));
  const configFile = path.join(root, "stillwind.config.ts");
  const write = (projects: object[]) =>
    writeFile(configFile, `export default ${JSON.stringify({ projects })}`);
  return { root, configFile, write };
}

test("discovery and project paths use the configuration directory", async () => {
  const { root, configFile, write } = await fixture();
  const app = path.join(root, "apps/web");
  await mkdir(app, { recursive: true });
  await write([{ root: "apps/web", css: "theme.css", files: ["src/*.tsx"] }]);
  expect(findConfiguration(app)).toBe(configFile);
  const resolved = await resolveIntegrationConfig({}, app);
  expect(resolved.config).toEqual({ root: app, css: "theme.css" });
  expect(resolved.dependencies).toContain(configFile);
  expect(resolved.missing).toContain(path.join(app, "stillwind.config.ts"));
});

test("an explicit file uses the application root while inline overrides preserve false", async () => {
  const { root, configFile, write } = await fixture();
  await write([{ root: "old", css: "theme.css", files: ["*.tsx"], automatic: true }]);
  const config = (
    await resolveIntegrationConfig(
      {
        configFile: path.basename(configFile),
        root: "new",
        automatic: false,
        css: undefined,
      },
      root,
    )
  ).config;
  expect(config).toEqual({
    root: path.join(root, "new"),
    css: "theme.css",
    automatic: false,
  });
});

test("configFile false bypasses an invalid configuration", async () => {
  const { root, configFile } = await fixture();
  await writeFile(configFile, "throw new Error('configuration evaluated')");
  const resolved = await resolveIntegrationConfig({ configFile: false, css: "inline.css" }, root);
  expect(resolved.config.css).toBe("inline.css");
  expect(resolved.dependencies).toEqual([]);
  await expect(resolveIntegrationConfig({ css: "inline.css" }, root)).rejects.toThrow(
    "configuration evaluated",
  );
});

test("missing configuration accepts complete inline options and rejects explicit missing paths", async () => {
  const { root } = await fixture();
  expect((await resolveIntegrationConfig({ css: "inline.css" }, root)).config.root).toBe(root);
  await expect(resolveIntegrationConfig({}, root)).rejects.toThrow("must specify CSS");
  await expect(
    resolveIntegrationConfig({ configFile: "absent.ts", css: "inline.css" }, root),
  ).rejects.toThrow("Cannot find Stillwind configuration");
});

test("unnamed projects can be selected by their distinct roots", async () => {
  const { root, write } = await fixture();
  await write([
    { css: "base.css", files: ["*.ts"] },
    { root: "web", css: "web.css", files: ["*.tsx"] },
  ]);
  expect((await resolveIntegrationConfig({}, path.join(root, "web"))).config.css).toBe("web.css");
});

test("overlapping project roots require an explicit name", async () => {
  const { root, write } = await fixture();
  await write([
    { css: "first.css", files: ["*.ts"] },
    { name: "web", css: "second.css", files: ["*.tsx"] },
  ]);
  await expect(resolveIntegrationConfig({}, root)).rejects.toThrow(
    "Cannot select one Stillwind project",
  );
  expect((await resolveIntegrationConfig({ project: "web" }, root)).config.css).toBe("second.css");
  await expect(resolveIntegrationConfig({ project: "absent" }, root)).rejects.toThrow(
    "expected one matching project",
  );
});

test("projects outside the application root do not silently select an array element", async () => {
  const { root, write } = await fixture();
  await write([
    { root: "first", css: "first.css", files: ["*.ts"] },
    { root: "second", css: "second.css", files: ["*.tsx"] },
  ]);
  await expect(resolveIntegrationConfig({}, root)).rejects.toThrow(
    "Cannot select one Stillwind project",
  );
});

test.each(["ts", "cjs", "json"])(
  "a changed %s helper is tracked and evaluated afresh",
  async (extension) => {
    const { root, configFile } = await fixture();
    const helper = path.join(root, `settings.${extension}`);
    const settings = (css: string) => ({ projects: [{ css, files: ["*.tsx"] }] });
    const source = (css: string) =>
      extension === "json"
        ? JSON.stringify(settings(css))
        : `${extension === "cjs" ? "module.exports =" : "export default"} ${JSON.stringify(settings(css))}`;
    await writeFile(helper, source("first.css"));
    await writeFile(
      configFile,
      `import settings from './settings.${extension}'; export default async () => settings;`,
    );
    const first = await loadConfiguration(configFile);
    expect(first.dependencies).toContain(helper);
    expect(first.config.projects[0].css).toBe("first.css");
    await writeFile(helper, source("second.css"));
    expect((await loadConfiguration(configFile)).config.projects[0].css).toBe("second.css");
  },
);

test("configuration merge replaces arrays and validates duplicate optional names", () => {
  expect(
    mergeConfig({ files: ["a"], automatic: true }, { files: ["b"], automatic: false }),
  ).toEqual({ files: ["b"], automatic: false });
  expect(mergeConfig({ kind: "custom", root: "." }, { kind: undefined })).toEqual({
    kind: "custom",
    root: ".",
  });
  expect(() =>
    validateConfig({
      projects: [
        { name: "web", css: "a.css", files: ["*.tsx"] },
        { name: "web", css: "b.css", files: ["*.ts"] },
      ],
    }),
  ).toThrow("Duplicate Stillwind project name");
});

test("nested overrides preserve naming callbacks and replace providers when kind changes", () => {
  const utilityClass = () => "utility";
  const refMarker = () => "marker";
  const base: BaseConfig = { naming: { utilityClass, refMarker } };
  expect(
    mergeConfig(base, { naming: { utilityClass: () => "override", refMarker: undefined } }).naming
      ?.refMarker,
  ).toBe(refMarker);
  const project: { projection: ProjectionProvider } = {
    projection: {
      kind: "custom",
      prepare: async () => expect.fail("Preparation should not execute"),
    },
  };
  expect(mergeConfig(project, { projection: { kind: "framework" } })).toEqual({
    projection: { kind: "framework" },
  });
});

test("bundled helpers retain their own directory and unused imports do not execute", async () => {
  const { root, configFile } = await fixture();
  const helperRoot = path.join(root, "config");
  await mkdir(helperRoot);
  await writeFile(path.join(helperRoot, "paths.ts"), "export const root = import.meta.dirname;");
  await writeFile(path.join(root, "unused.cjs"), "throw new Error('unused import executed');");
  await writeFile(
    configFile,
    `import {root} from './config/paths.ts';
import type {Unused} from './unused.cjs';
export default {projects:[{root,css:'theme.css',files:['*.tsx']}]};`,
  );
  expect((await loadConfiguration(configFile)).config.projects[0].root).toBe(helperRoot);
});
