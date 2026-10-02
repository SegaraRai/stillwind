import { mkdtempSync, rmSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { stillwind } from "@stillwind/integration/vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import vue from "@vitejs/plugin-vue";
import { dev as astroDev } from "astro";
import { createServer, type PluginOption } from "vite";

import { writeFrameworkFixture, type Framework } from "../shared/framework-fixtures.ts";

const packageRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

const close: Array<() => Promise<void>> = [];
const roots: string[] = [];
let port = 5174;
for (const framework of ["vue", "svelte", "astro"] satisfies Framework[]) {
  const root = mkdtempSync(path.join(packageRoot, ".test-framework-"));
  roots.push(root);
  writeFrameworkFixture(root, framework);

  const plugin = stillwind({
    css: path.join(root, "root.css"),
    types: false,
  });
  if (framework === "astro") {
    const config: Parameters<typeof astroDev>[0] = {
      root,
      configFile: false,
      logLevel: "silent",
      devToolbar: {
        enabled: false,
      },
      server: {
        host: "127.0.0.1",
        port,
      },
      vite: {
        cacheDir: path.join(root, ".vite"),
        plugins: [plugin],
      } as unknown as Parameters<typeof astroDev>[0]["vite"],
    };
    const server = await astroDev(config);
    close.push(() => server.stop());
  } else {
    const frameworkPlugin = {
      vue: () => vue(),
      svelte: () => svelte({ configFile: false }),
    }[framework]();
    const server = await createServer({
      root,
      configFile: false,
      logLevel: "silent",
      cacheDir: path.join(root, ".vite"),
      plugins: [plugin, frameworkPlugin],
      server: {
        host: "127.0.0.1",
        port,
        strictPort: true,
      },
    });
    await server.listen();
    close.push(() => server.close());
  }

  console.log(
    JSON.stringify({
      framework,
      root,
      url: `http://127.0.0.1:${port}`,
    }),
  );

  port++;
}

let stopping = false;
async function stop() {
  if (stopping) {
    return;
  }
  stopping = true;
  await Promise.all(close.map((stop) => stop()));
  for (const root of roots) {
    rmSync(root, { recursive: true, force: true });
  }
}

process.once("SIGINT", () => {
  void stop();
});

process.once("SIGTERM", () => {
  void stop();
});
