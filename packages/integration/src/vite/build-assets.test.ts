import { createHash } from "node:crypto";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { transform } from "esbuild";
import type { Rollup } from "vite";
import { expect, onTestFinished, test, vi } from "vite-plus/test";

import { browserSourceMap } from "./browser-source-map.ts";
import { emitBuildCss, emitBuildResources } from "./build-assets.ts";

test("build resources share canonical files and skip unused placeholders", async () => {
  const root = await mkdtemp(join(tmpdir(), "stillwind-build-assets-"));
  onTestFinished(() => rm(root, { recursive: true, force: true }));
  const file = join(root, "icon.svg");
  await writeFile(file, "PUBLIC_ICON");
  const { context, emitted } = assetContext();
  context.getFileName = () => 'assets/nested/icon-"quoted".svg';

  const replacements = await emitBuildResources(
    context,
    "url(__first__) url(__second__)",
    new Map([
      ["__first__", { id: file, file }],
      ["__second__", { id: `${file}?url`, file }],
      ["__unused__", { id: join(root, "missing.svg"), file: join(root, "missing.svg") }],
    ]),
    "assets",
  );

  expect(emitted).toHaveLength(1);
  expect(emitted[0]).toMatchObject({ type: "asset", name: "icon.svg" });
  expect(replacements).toEqual(
    new Map([
      ["__first__", 'nested/icon-\\"quoted\\".svg'],
      ["__second__", 'nested/icon-\\"quoted\\".svg'],
    ]),
  );
});

test.each([false, true])(
  "CSS emission hashes before appending the private map URL (minify: %s)",
  async (minifyCss) => {
    const { context, emitted } = assetContext();
    const text = ".x { color: red; }";
    const sourceMap = JSON.stringify({
      version: 3,
      sources: ["file:///private/server.ts"],
      sourcesContent: ["PRIVATE_SOURCE"],
      names: [],
      mappings: "AAAA",
    });
    const publicSourceMap = vi.fn((_name, map) =>
      browserSourceMap(map, () => "stillwind-source:opaque"),
    );
    const expectedCss = minifyCss
      ? (
          await transform(
            `${text}\n/*# sourceMappingURL=data:application/json;base64,${Buffer.from(sourceMap).toString("base64")} */`,
            {
              loader: "css",
              minify: true,
              sourcemap: "external",
              sourcefile: "stillwind.css",
            },
          )
        ).code
      : text;
    const hash = createHash("sha256").update(expectedCss).digest("hex").slice(0, 16);

    const reference = await emitBuildCss(
      {
        settings: {
          minifyCss,
          assetsDirectory: "nested/assets",
        },
        publicSourceMap,
      },
      "ssr",
      context,
      text,
      sourceMap,
    );

    expect(reference).toBe(`nested/assets/stillwind-${hash}.css`);
    expect(emitted.map((asset) => asset.fileName)).toEqual([
      `nested/assets/stillwind-${hash}.css.map`,
      `nested/assets/stillwind-${hash}.css`,
    ]);
    expect(emitted[1]).toMatchObject({
      source: `${expectedCss}\n/*# sourceMappingURL=stillwind-${hash}.css.map */`,
    });
    expect(publicSourceMap).toHaveBeenCalledWith("ssr", expect.any(String));
    expect(JSON.stringify(emitted)).not.toMatch(/PRIVATE_SOURCE|file:|sourcesContent/);
  },
);

function assetContext() {
  const emitted: Rollup.EmittedFile[] = [];
  const context: Pick<Rollup.PluginContext, "emitFile" | "getFileName"> = {
    emitFile(asset) {
      emitted.push(asset);
      return asset.fileName ?? `asset-${emitted.length}`;
    },
    getFileName: (reference) => reference,
  };
  return { context, emitted };
}
