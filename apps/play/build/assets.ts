import { cp, mkdir, readdir, readFile, rm, stat, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";

import { build } from "esbuild";

import { bundleRefresh } from "./refresh.ts";

const root = path.resolve(import.meta.dirname, "..");
/** Iconify sets available in the playground: two monochrome sets and one colored set. */
const ICON_SETS = ["mdi", "lucide", "fluent-emoji-flat"];
const output = path.join(root, "dist/assets-static");
if (path.dirname(output) !== path.join(root, "dist")) {
  throw new Error("Invalid generated asset directory");
}
await rm(output, { recursive: true, force: true });
await mkdir(output, { recursive: true });
await build({
  entryPoints: [
    path.join(root, "node_modules/@codingame/monaco-vscode-api/workers/extensionHost.worker.js"),
  ],
  outfile: path.join(output, "extension-host.js"),
  bundle: true,
  format: "iife",
  platform: "browser",
  target: "esnext",
});
const files: [string, string][] = [];
for (const [name, directory] of [
  ["stillwind", "../../packages/core"],
  ["@stillwind/config", "../../packages/config"],
  ["@stillwind/plugin-iconify", "../../plugins/iconify"],
  ["valibot", "../../packages/config/node_modules/valibot"],
  ["@iconify/utils", "../../plugins/iconify/node_modules/@iconify/utils"],
  ["@types/react", "node_modules/@types/react"],
  ["@types/react-dom", "node_modules/@types/react-dom"],
] as const) {
  await collect(path.resolve(root, directory), `/workspace/node_modules/${name}`);
}
// The Iconify plugin imports only the self-contained `svgo/browser` bundle.
const svgo = path.resolve(root, "../../plugins/iconify/node_modules/svgo");
for (const file of ["package.json", "dist/svgo.browser.js"]) {
  files.push([
    `/workspace/node_modules/svgo/${file}`,
    await readFile(path.join(svgo, file), "utf8"),
  ]);
}
const require = createRequire(path.join(root, "package.json"));
// Icon sets need only their manifests and icon data. The hidden workspace manifest lists them so
// that the Iconify plugin offers their icons as completions.
for (const set of ICON_SETS) {
  const directory = path.dirname(require.resolve(`@iconify-json/${set}/package.json`));
  for (const file of ["package.json", "icons.json"]) {
    files.push([
      `/workspace/node_modules/@iconify-json/${set}/${file}`,
      await readFile(path.join(directory, file), "utf8"),
    ]);
  }
}
files.push([
  "/workspace/package.json",
  JSON.stringify({
    private: true,
    devDependencies: Object.fromEntries(ICON_SETS.map((set) => [`@iconify-json/${set}`, "*"])),
  }),
]);
await bundleRefresh(
  require.resolve("react-refresh/babel"),
  path.join(output, "react-refresh-babel.js"),
);
await collect(
  path.dirname(require.resolve("csstype/package.json")),
  "/workspace/node_modules/csstype",
);
await writeFile(path.join(output, "workspace.json"), JSON.stringify(files));
await cp(
  path.join(root, "node_modules/esbuild-wasm/esbuild.wasm"),
  path.join(output, "esbuild.wasm"),
);
await cp(
  path.resolve(root, "../../packages/vscode-extension/dist/extension"),
  path.join(output, "extensions/stillwind"),
  {
    recursive: true,
    filter: (file) =>
      file !== path.resolve(root, "../../packages/vscode-extension/dist/extension/dist/node"),
  },
);
await build({
  entryPoints: [path.join(root, "src/preview/runtime.ts")],
  outfile: path.join(output, "preview-runtime.js"),
  bundle: true,
  format: "iife",
  platform: "browser",
  define: { "process.env.NODE_ENV": '"development"' },
  minify: true,
});
await writeFile(
  path.join(output, "_headers"),
  "/*\n  Cross-Origin-Opener-Policy: same-origin\n  Cross-Origin-Embedder-Policy: require-corp\n",
);

/** Captures package text for the browser's filesystem, excluding tools and development sources. */
async function collect(directory: string, virtual: string): Promise<void> {
  for (const entry of await readdir(directory)) {
    if (
      ["node_modules", ".git", ".cache", "src", "tests", "typecheck", "dist"].includes(entry) &&
      (entry !== "dist" || virtual.includes("/dist"))
    ) {
      continue;
    }
    const file = path.join(directory, entry);
    const info = await stat(file);
    if (info.isDirectory()) {
      await collect(file, `${virtual}/${entry}`);
    } else if (/\.(?:[cm]?js|[cm]?ts|tsx|json|css)$/.test(entry) && !entry.endsWith(".map")) {
      files.push([`${virtual}/${entry}`, await readFile(file, "utf8")]);
    }
  }
}
