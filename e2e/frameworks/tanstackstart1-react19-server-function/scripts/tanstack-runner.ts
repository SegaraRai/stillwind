import path from "node:path";
import { pathToFileURL } from "node:url";

const [bundler, mode, root] = process.argv.slice(2);
process.chdir(root);
if (bundler === "vite") {
  if (mode === "start") {
    await import(pathToFileURL(path.join(root, ".output/server/index.mjs")).href);
  } else {
    const { createBuilder, createServer } = await import("vite-plus");
    if (mode === "build") {
      await (await createBuilder({ root })).buildApp();
    } else {
      const server = await createServer({ root });
      await server.listen();
      server.printUrls();
    }
  }
} else if (mode === "start") {
  const { serve } = await import("srvx");
  const { staticMiddleware } = await import("srvx/static");
  const entry = await import(pathToFileURL(path.join(root, "dist/server/index.js")).href);
  serve({
    port: Number(process.env.PORT),
    middleware: [staticMiddleware({ dir: path.join(root, "dist/client") })],
    fetch: entry.default.fetch,
  });
} else {
  const { createRsbuild } = await import("@rsbuild/core");
  const { default: config } = await import(
    pathToFileURL(path.join(root, "rsbuild.config.mjs")).href
  );
  const instance = await createRsbuild({ cwd: root, rsbuildConfig: config });
  if (mode === "build") {
    const result = await instance.build();
    await result.close();
  } else {
    await instance.startDevServer();
  }
}
