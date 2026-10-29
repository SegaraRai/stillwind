import type { NativeStylesheetPart } from "@stillwind/native";
import type { Rollup } from "vite";
import { expect, test, vi } from "vite-plus/test";

import { buildHooks, type BuildOutput } from "./build.ts";
import { runtimeId } from "./module-ids.ts";
import type { ViteProject } from "./project.ts";

test.each(["single", "split"] as const)(
  "%s emission recovers cached and lazy metadata before planning complete CSS",
  async (emission) => {
    const fixture = buildFixture(emission);

    await fixture.renderStart();

    expect(fixture.events).toEqual(["compile:/lazy.ts", "projection:/lazy.ts", "plan"]);
    expect(fixture.documentPlan).toHaveBeenCalledWith("client", fixture.context, true);
    const manifest = fixture.output()!.manifest!;
    const cssAssets = fixture.emitted.filter((asset) => asset.fileName?.endsWith(".css"));
    expect(cssAssets).toHaveLength(emission === "single" ? 1 : 2);
    if (emission === "single") {
      expect(manifest.parts).toEqual([
        {
          id: "document",
          href: `/app/${fixture.output()!.buildCss}`,
        },
      ]);
      expect(manifest.modules).toEqual({
        entry: ["document"],
        lazy: ["document"],
      });
      expect(cssAssets[0]).toMatchObject({
        source: fixture.css,
      });
    } else {
      expect(fixture.output()!.buildCss).toBeUndefined();
      expect(manifest.parts.map((part) => part.id)).toEqual(["0:red", "1:blue", "2:red"]);
      expect(manifest.parts[0].href).toBe(manifest.parts[2].href);
      expect(manifest.modules).toEqual({
        entry: ["0:red", "1:blue", "2:red"],
        lazy: ["0:red", "1:blue", "2:red"],
      });
      expect(cssAssets.map((asset) => ("source" in asset ? asset.source : undefined))).toEqual([
        ".red{color:red}",
        ".blue{color:blue}",
      ]);
    }
    expect(fixture.emitted.at(-1)).toEqual({
      type: "asset",
      fileName: "stillwind-manifest.json",
      source: JSON.stringify(manifest, null, 2),
    });
  },
);

test("development rendering leaves build output and the document plan untouched", async () => {
  const fixture = buildFixture("single");
  fixture.project.settings.watching = true;

  await fixture.renderStart();

  expect(fixture.events).toEqual([]);
  expect(fixture.output()).toBeUndefined();
  expect(fixture.emitted).toEqual([]);
});

/**
 * Creates a completed graph with cached and lazy producers plus repeated stylesheet partitions.
 * @param emission - Document asset mode exercised by the production hooks.
 * @returns Recording project and hook context, emitted assets, event order, and captured build output.
 */
function buildFixture(emission: "single" | "split") {
  const events: string[] = [];
  const inputs = new Map([
    [
      "/cached.ts",
      {
        moduleId: "entry",
        dependencies: ["lazy"],
      },
    ],
  ]);
  /**
   * Creates a partition for exercising build emission and repeated partition keys.
   * @param key - Partition identity shared by repeated entries.
   * @param css - CSS emitted for the partition.
   * @param owner - Producer module that requires this partition.
   * @returns A partition in slot zero with one producer and an empty source map.
   */
  const part = (key: string, css: string, owner: string): NativeStylesheetPart => ({
    key,
    css,
    producers: [owner],
    banners: [],
    slot: 0,
    sequence: 0,
    sourceMap: {
      sources: [],
      entries: [],
    },
  });
  const rawParts = [
    part("red", ".red{color:red}", "entry"),
    part("blue", ".blue{color:blue}", "lazy"),
    part("red", ".red{color:red}", "entry"),
  ];
  const css = rawParts.map((part) => part.css).join("");
  const compile = vi.fn(async (_name, _code, id) => {
    events.push(`compile:${id}`);
    inputs.set(id, {
      moduleId: "lazy",
      dependencies: ["entry"],
    });
  });
  const documentPlan = vi.fn(async () => {
    events.push("plan");
    return {
      originalCss: css,
      banners: [],
      designSystem: "design",
      producers: [
        {
          module: "entry",
        },
        {
          module: "lazy",
        },
      ],
      rawParts,
      resources: new Map(),
    };
  });
  let output: BuildOutput | undefined;
  const project = {
    config: {
      emission,
    },
    settings: {
      watching: false,
      assetsDirectory: "assets",
      publicBase: "/app/",
      minifyCss: false,
      sourceMaps: false,
    },
    inputs: () => inputs,
    compile,
    documentPlan,
    environment: () => ({
      markProjection: async (id: string) => {
        await Promise.resolve();
        events.push(`projection:${id}`);
      },
    }),
    buildOutput: () => output,
    setBuildOutput: (_name: string, result: BuildOutput) => {
      output = result;
    },
  } as unknown as ViteProject;
  const emitted: Rollup.EmittedFile[] = [];
  const context = {
    environment: {
      name: "client",
    },
    getModuleIds: () => [runtimeId, "/cached.ts", "/lazy.ts?projection"],
    getModuleInfo: (id: string) => ({
      meta: {
        stillwind:
          id === "/cached.ts"
            ? {
                input: "cached input",
              }
            : id === "/lazy.ts?projection"
              ? {
                  id: "/lazy.ts",
                  input: "lazy input",
                  projection: true,
                }
              : {},
      },
    }),
    emitFile: (asset: Rollup.EmittedFile) => {
      emitted.push(asset);
      return asset.fileName!;
    },
    getFileName: (reference: string) => reference,
  };
  const renderStart = buildHooks(project).renderStart;
  if (typeof renderStart !== "function") {
    expect.fail("Expected a renderStart hook");
  }
  return {
    project,
    context,
    compile,
    documentPlan,
    emitted,
    events,
    css,
    output: () => output,
    renderStart: () =>
      renderStart.call(
        context as unknown as Rollup.PluginContext,
        {} as Rollup.NormalizedOutputOptions,
        {} as Rollup.NormalizedInputOptions,
      ),
  };
}
