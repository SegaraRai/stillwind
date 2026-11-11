import {
  cpSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { svelte } from "@sveltejs/vite-plugin-svelte";
import { build } from "vite";
import { expect, test } from "vite-plus/test";

const packageRoot = fileURLToPath(new URL("../..", import.meta.url));

test("SvelteKit SSR builds bundle the handle from an installed package", async () => {
  // A workspace link is never externalized, so the package is copied into node_modules as an
  // installed dependency would be.
  const root = mkdtempSync(path.join(packageRoot, ".test-sveltekit-handle-"));
  try {
    const installed = path.join(root, "node_modules", "@stillwind", "integration");
    mkdirSync(path.join(installed, "dist"), { recursive: true });
    cpSync(path.join(packageRoot, "package.json"), path.join(installed, "package.json"));
    cpSync(path.join(packageRoot, "dist", "sveltekit"), path.join(installed, "dist", "sveltekit"), {
      recursive: true,
    });
    writeFileSync(
      path.join(root, "package.json"),
      JSON.stringify({
        name: "sveltekit-handle-fixture",
        type: "module",
        dependencies: {
          "@stillwind/integration": "*",
        },
      }),
    );
    writeFileSync(
      path.join(root, "hooks.server.js"),
      'export { handle } from "@stillwind/integration/sveltekit/handle";\n',
    );
    await build({
      root,
      configFile: false,
      logLevel: "silent",
      plugins: [svelte()],
      build: {
        ssr: "hooks.server.js",
        outDir: "dist",
        rollupOptions: {
          // SvelteKit supplies this module when it builds the application.
          external: ["$app/paths"],
        },
      },
    });
    const dist = path.join(root, "dist");
    const output = readdirSync(dist)
      .filter((file) => file.endsWith(".js"))
      .map((file) => readFileSync(path.join(dist, file), "utf8"))
      .join("\n");
    expect(output).not.toMatch(/\bfrom\s*["']@stillwind\/integration/);
    expect(output).toMatch(/transformPageChunk/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
