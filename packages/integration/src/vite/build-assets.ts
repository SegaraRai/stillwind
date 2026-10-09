import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { posix } from "node:path";

import { transform as transformCss } from "esbuild";
import type { Rollup } from "vite";

import { readableFile } from "./file-access.ts";
import type { ViteProject } from "./project.ts";
import type { StylesheetResource } from "./resources.ts";

/**
 * Emits referenced resource files and computes URLs relative to the CSS asset directory.
 *
 * Only resources referenced by `css` are emitted. References to the same readable file share one
 * emitted asset within this call.
 *
 * @param context - Build context that emits assets and resolves their filenames.
 * @param css - CSS text containing resource placeholder tokens.
 * @param resources - Resolved resources indexed by placeholder token.
 * @param assetsDirectory - Output directory containing the emitted CSS, using forward slashes.
 * @returns Placeholder replacements containing escaped paths relative to `assetsDirectory`.
 * @throws If a referenced resource cannot be read or emitted.
 */
export async function emitBuildResources(
  context: Pick<Rollup.PluginContext, "emitFile" | "getFileName">,
  css: string,
  resources: ReadonlyMap<string, StylesheetResource>,
  assetsDirectory: string,
): Promise<Map<string, string>> {
  const assets = new Map<string, string>();
  const replacements = new Map<string, string>();
  for (const [placeholder, resource] of resources) {
    if (!css.includes(placeholder)) {
      continue;
    }
    const file = await readableFile(resource.id);
    let reference = assets.get(file);
    if (!reference) {
      reference = context.emitFile({
        type: "asset",
        name: file.replaceAll("\\", "/").split("/").at(-1),
        source: await readFile(file),
      });
      assets.set(file, reference);
    }
    replacements.set(
      placeholder,
      JSON.stringify(posix.relative(assetsDirectory, context.getFileName(reference))).slice(1, -1),
    );
  }
  return replacements;
}

/**
 * Emits a stylesheet and its optional external source map with filenames derived from its content.
 *
 * Minification follows project settings. Source maps use public source names and are emitted beside
 * the CSS asset.
 *
 * @param project - CSS minification settings, asset directory, and public source-map rewriting
 * operations.
 * @param name - Vite environment name, or `undefined` for `client`.
 * @param context - Build context used to emit the CSS and map assets.
 * @param text - CSS text whose resource placeholders have already been replaced.
 * @param sourceMap - Serialized upstream CSS map, or `undefined` to omit a map.
 * @returns The bundler reference ID of the emitted CSS asset, suitable for `getFileName`.
 * @throws If CSS transformation, map decoding, or asset emission fails.
 */
export async function emitBuildCss(
  project: {
    /** Effective minification policy and emitted asset directory for the environment. */
    settings: Pick<ViteProject["settings"], "minifyCss" | "assetsDirectory">;
    /** Rewrites source-map identities for publicly emitted CSS. */
    publicSourceMap: ViteProject["publicSourceMap"];
  },
  name: string | undefined,
  context: Pick<Rollup.PluginContext, "emitFile">,
  text: string,
  sourceMap?: string,
): Promise<string> {
  const { settings, publicSourceMap } = project;
  if (settings.minifyCss) {
    const input = sourceMap
      ? `${text}\n/*# sourceMappingURL=data:application/json;base64,${Buffer.from(sourceMap).toString("base64")} */`
      : text;
    const result = await transformCss(input, {
      loader: "css",
      minify: true,
      sourcemap: sourceMap ? "external" : false,
      sourcefile: "stillwind.css",
    });
    text = result.code;
    sourceMap = result.map || undefined;
  }
  const hash = createHash("sha256").update(text).digest("hex").slice(0, 16);
  if (sourceMap) {
    sourceMap = JSON.stringify(publicSourceMap(name, sourceMap));
    context.emitFile({
      type: "asset",
      fileName: `${settings.assetsDirectory}/stillwind-${hash}.css.map`,
      source: sourceMap,
    });
    text += `\n/*# sourceMappingURL=stillwind-${hash}.css.map */`;
  }
  return context.emitFile({
    type: "asset",
    fileName: `${settings.assetsDirectory}/stillwind-${hash}.css`,
    source: text,
  });
}
