import fs from "node:fs";
import path from "node:path";

import { createRsbuild } from "@rsbuild/core";

import * as bundler from "../../../packages/integration/dist/rsbuild/index.js";
const dir = process.argv[2];
const rsbuild = await createRsbuild({
  cwd: dir,
  rsbuildConfig: {
    root: dir,
    plugins: [bundler.pluginStillwind({ css: path.join(dir, "root.css"), root: dir })],
    source: { entry: { main: path.join(dir, "main.js") } },
    output: {
      sourceMap: { js: "source-map", css: true },
      distPath: { root: path.join(dir, "dist") },
      assetPrefix: "/",
      minify: false,
      dataUriLimit: 0,
    },
    tools: {
      rspack: {
        devtool: "source-map",
        cache: {
          type: "persistent",
          storage: { type: "filesystem", directory: path.join(dir, "cache") },
        },
      },
    },
  },
});
const result = await rsbuild.build();
try {
  const stats = result.stats.stats?.[0] ?? result.stats;
  if (stats.hasErrors()) {
    throw new Error(stats.toString());
  }
  fs.writeFileSync(
    path.join(dir, "stats.json"),
    JSON.stringify(
      stats.toJson({
        all: false,
        assets: true,
        chunks: true,
        modules: true,
        cachedModules: true,
        nestedModules: true,
        orphanModules: true,
        groupModulesByAttributes: false,
        groupModulesByCacheStatus: false,
      }),
      null,
      2,
    ),
  );
} finally {
  await result.close();
}
