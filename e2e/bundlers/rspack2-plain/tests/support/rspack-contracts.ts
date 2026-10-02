import fs from "node:fs/promises";
import path from "node:path";

import { TraceMap, originalPositionFor, sourceContentFor } from "@jridgewell/trace-mapping";
import { resolveSourceMapFile } from "@stillwind/e2e-harness/source-map";
import { expect } from "vite-plus/test";

import type { RspackApplication } from "./rspack-application.ts";

export async function expectBuildStage(
  app: RspackApplication,
  stage: "cold" | "warm" | "css-edit" | "unreachable" | "macro-removal",
): Promise<void> {
  if (stage === "css-edit") {
    await app.put("root.css", app.theme("rgb(44,55,66)"));
  }
  if (stage === "unreachable") {
    await app.put(
      "main.js",
      `import {props} from './static.js'; document.body.innerHTML='<p id="main">Main</p>'; document.querySelector('#main').className=props.className??'';`,
    );
  }
  if (stage === "macro-removal") {
    await app.put("static.js", "export const props={};");
  }
  const stats = await app.build();
  if (stage === "cold" || stage === "warm") {
    const cssFile = stats.chunks
      .filter((chunk) => chunk.initial)
      .flatMap((chunk) => chunk.files)
      .find((file) => file.endsWith(".css"));
    expect(cssFile).toBeTruthy();
    const cssText = await fs.readFile(path.join(app.dir, "dist", cssFile!), "utf8");
    const cssMap = new TraceMap(
      JSON.parse(await fs.readFile(path.join(app.dir, "dist", cssFile + ".map"), "utf8")),
    );
    const offset = [...cssText.matchAll(/\.sw_/g)].find(({ index }) => {
      const prefix = cssText.slice(0, index);
      const origin = originalPositionFor(cssMap, {
        line: prefix.split("\n").length,
        column: index - prefix.lastIndexOf("\n") - 1,
      });
      return (
        origin.source &&
        resolveSourceMapFile(
          origin.source!,
          path.join(app.dir, "dist", cssFile + ".map"),
          app.dir,
        ) === path.join(app.dir, "static.js")
      );
    })?.index;
    expect(
      offset !== undefined,
      JSON.stringify({ sources: cssMap.sources, root: app.dir, cssFile }),
    ).toBeTruthy();
    const prefix = cssText.slice(0, offset);
    const origin = originalPositionFor(cssMap, {
      line: prefix.split("\n").length,
      column: offset! - prefix.lastIndexOf("\n") - 1,
    });
    expect(origin.source, JSON.stringify(origin)).toBeTruthy();
    expect(
      resolveSourceMapFile(origin.source!, path.join(app.dir, "dist", cssFile + ".map"), app.dir),
    ).toBe(path.join(app.dir, "static.js"));
    const source = await fs.readFile(path.join(app.dir, "static.js"), "utf8");
    const macroOffset = source.indexOf("style(");
    expect(macroOffset).not.toBe(-1);
    const sourcePrefix = source.slice(0, macroOffset);
    expect(origin.line).toBe(sourcePrefix.split("\n").length);
    expect(origin.column).toBe(macroOffset - sourcePrefix.lastIndexOf("\n") - 1);
    expect(sourceContentFor(cssMap, origin.source!)).toBe(
      await fs.readFile(path.join(app.dir, "static.js"), "utf8"),
    );
    app.results.checks.push({ name: `${app.mode}-${stage}-css-source-map`, passed: true });
  }
  await app.page.goto(app.url);
  const expected =
    stage === "macro-removal"
      ? "rgb(0, 0, 0)"
      : ["cold", "warm"].includes(stage)
        ? "rgb(11, 22, 33)"
        : "rgb(44, 55, 66)";
  await app.page.waitForFunction(
    (expected: string) =>
      getComputedStyle(document.querySelector<HTMLElement>("#main")!).color === expected,
    expected,
  );
  if (stage === "macro-removal") {
    expect(
      stats.chunks.flatMap((chunk) => chunk.files).filter((file) => file.endsWith(".css")).length,
    ).toBe(0);
    app.results.checks.push({ name: `${app.mode}-macro-removal`, passed: true });
    return;
  }
  const bg = await app.page.locator("#main").evaluate((el) => getComputedStyle(el).backgroundImage);
  expect(bg).toMatch(/\.svg/);
  expect((await fetch(bg.match(/url\("([^"]+)"\)/)![1])).status).toBe(200);
  if (stage === "unreachable") {
    expect(
      stats.chunks.flatMap((chunk) => chunk.files).filter((file) => file.endsWith(".css")).length,
    ).toBe(1);
    expect(!stats.modules.some((module) => module.name === "./lazy.js")).toBeTruthy();
    app.results.checks.push({ name: `${app.mode}-unreachable-producer`, passed: true });
    return;
  }
  await app.page.locator("#load").click();
  await app.page.waitForFunction(
    () =>
      getComputedStyle(document.querySelector<HTMLElement>("#lazy")!).color === "rgb(70, 80, 90)",
  );
  const producers = stats.modules.filter((module) => /\.\/(static|lazy)\.js$/.test(module.name));
  expect(producers.length).toBe(2);
  if (stage === "warm") {
    expect(
      producers.every((module) => !module.built),
      JSON.stringify(producers),
    ).toBeTruthy();
  }
  app.results.checks.push({
    name: `${app.mode}-${stage}-browser-css`,
    passed: true,
    producers: producers.map((module) => ({ name: module.name, built: module.built })),
    cssChunks: stats.chunks.flatMap((chunk) => chunk.files).filter((file) => file.endsWith(".css")),
  });
}
