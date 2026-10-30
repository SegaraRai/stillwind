import { cp, mkdir, readdir, readFile, realpath, rm, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import process from "node:process";

import { writeExtensionLicenses } from "./licenses.ts";
import { extensionManifest } from "./src/manifest.ts";

/** License files that the extension publishes, and the files that it copies them from. */
const licenses = {
  LICENSE: "LICENSE",
  "RUST-THIRD-PARTY-LICENSES": "../native/RUST-THIRD-PARTY-LICENSES",
  "TAILWINDCSS-MIT-LICENSE": "TAILWINDCSS-MIT-LICENSE",
  "WEBREF-MIT-LICENSE": "WEBREF-MIT-LICENSE",
};

const root = import.meta.dirname;
const output = path.join(root, "dist/extension");
const browser = path.join(output, "dist/browser");
const typescriptLibrary = path.resolve(root, "../host/node_modules/typescript-6/lib");
const require = createRequire(import.meta.url);

/**
 * Packages whose files the build copies without bundling: the Rolldown WebAssembly
 * binary and the TypeScript library files. The Stillwind binary is covered by the
 * copied Rust, Tailwind CSS, and Webref licenses.
 */
const copiedPackages = [
  path.dirname(require.resolve("@rolldown/browser/package.json")),
  path.dirname(typescriptLibrary),
];

const packaging =
  process.argv.includes("--package-web") || process.argv.includes("--package-desktop");

if (!packaging) {
  await mkdir(browser, { recursive: true });
  await cp(path.join(root, "src/server/bootstrap.js"), path.join(browser, "server.js"));
  const native = path.resolve(root, "../native/npm/wasm32-wasi");
  await cp(
    path.join(native, "stillwind_native.wasm32-wasi.wasm"),
    path.join(browser, "stillwind_native.wasm32-wasi.wasm"),
  );
  const rolldownBrowser = path.dirname(require.resolve("@rolldown/browser"));
  await cp(
    path.join(rolldownBrowser, "rolldown-binding.wasm32-wasi.wasm"),
    path.join(browser, "rolldown-binding.wasm32-wasi.wasm"),
  );
  const libraries: [string, string][] = await Promise.all(
    (await readdir(typescriptLibrary))
      .filter((name) => name.endsWith(".d.ts"))
      .map(async (name): Promise<[string, string]> => [
        `/node_modules/typescript/lib/${name}`,
        await readFile(path.join(typescriptLibrary, name), "utf8"),
      ]),
  );
  for (const [name, directory] of [
    ["stillwind", "../core"],
    ["@stillwind/config", "../config"],
  ] as const) {
    await capturePackage(path.resolve(root, directory), `/__stillwind/packages/${name}`, libraries);
  }
  await writeFile(path.join(browser, "libraries.json"), JSON.stringify(libraries));
  await writeFile(
    path.join(output, "package.json"),
    JSON.stringify(
      {
        ...extensionManifest,
        repository: {
          type: "git",
          url: "https://github.com/SegaraRai/stillwind",
        },
      },
      null,
      2,
    ),
  );
  await cp(path.join(root, "README.md"), path.join(output, "README.md"));
  await cp(path.join(root, "icon.png"), path.join(output, "icon.png"));
  for (const [name, source] of Object.entries(licenses)) {
    await cp(path.join(root, source), path.join(output, name));
  }
  writeExtensionLicenses(output, copiedPackages);
  await writeFile(
    path.join(output, "dist/node/package.json"),
    JSON.stringify({
      type: "module",
    }),
  );
}

if (process.argv.includes("--package-web") || process.argv.includes("--package-desktop")) {
  const desktop = process.argv.includes("--package-desktop");
  const stage = path.join(root, `dist/${desktop ? "desktop" : "web"}`);
  await clean(stage);
  await cp(output, stage, {
    recursive: true,
    filter: (file) => desktop || file !== path.join(output, "dist/node"),
  });
  const manifest: Record<string, unknown> = {
    ...extensionManifest,
    files: ["dist/**", "icon.png", "NPM-THIRD-PARTY-LICENSES", ...Object.keys(licenses)],
    repository: {
      type: "git",
      url: "https://github.com/SegaraRai/stillwind",
    },
  };
  manifest.main = "./dist/node/extension.cjs";
  if (!desktop) {
    delete manifest.main;
  } else {
    const copied = new Map<string, string>();
    await copyDependencies(
      path.resolve(root, "../language-server"),
      path.join(stage, "dist/node"),
      "@stillwind/language-server",
      copied,
    );
    // Some copied dependencies have no license files, so the desktop package lists
    // the npm packages among them as well. Workspace packages are covered by LICENSE.
    const installed = [...copied.values()].filter((directory) =>
      directory.split(path.sep).includes("node_modules"),
    );
    writeExtensionLicenses(stage, [...copiedPackages, ...installed]);
  }
  await writeFile(path.join(stage, "package.json"), JSON.stringify(manifest, null, 2));
  const { createVSIX } = await import("@vscode/vsce");
  await createVSIX({
    cwd: stage,
    packagePath: path.join(root, `dist/stillwind-${desktop ? "desktop" : "web"}.vsix`),
    dependencies: false,
    ...(desktop
      ? {
          target: `${process.platform}-${process.arch}`,
        }
      : {
          target: "web",
        }),
  });
}

/** Copies the production dependency closure as real directories for an independently installable VSIX. */
async function copyDependencies(
  directory: string,
  stage: string,
  name: string,
  copied: Map<string, string>,
): Promise<void> {
  const manifest = JSON.parse(await readFile(path.join(directory, "package.json"), "utf8")) as {
    name: string;
    version: string;
    files?: string[];
    dependencies?: Record<string, string>;
    optionalDependencies?: Record<string, string>;
    os?: string[];
    cpu?: string[];
  };
  if (
    (manifest.os && !manifest.os.includes(process.platform)) ||
    (manifest.cpu && !manifest.cpu.includes(process.arch))
  ) {
    return;
  }
  const target = path.join(stage, "node_modules", name);
  if (copied.get(target) === directory) {
    return;
  }
  if (copied.has(target)) {
    throw new Error(`Conflicting packaged dependency: ${name}`);
  }
  copied.set(target, directory);
  const published = manifest.files?.map((file) => file.replace(/^\.\//, "").split(/[/*]/)[0]);
  await cp(directory, target, {
    recursive: true,
    dereference: true,
    filter: (file) => {
      const relative = path.relative(directory, file);
      if (!relative) {
        return true;
      }
      const first = relative.split(path.sep, 1)[0];
      if (["node_modules", ".git", ".cache", ".build"].includes(first)) {
        return false;
      }
      return (
        !published || published.includes(first) || /^(package\.json|readme|license)/i.test(first)
      );
    },
  });
  for (const name of Object.keys({
    ...manifest.dependencies,
    ...manifest.optionalDependencies,
  })) {
    let dependency: string;
    try {
      dependency = await dependencyDirectory(directory, name);
    } catch (error) {
      if (manifest.optionalDependencies?.[name]) {
        continue;
      }
      throw error;
    }
    const shared = path.join(root, "dist/desktop/dist/node");
    const globalTarget = path.join(shared, "node_modules", name);
    await copyDependencies(
      dependency,
      copied.has(globalTarget) && copied.get(globalTarget) !== dependency ? target : shared,
      name,
      copied,
    );
  }
}

async function dependencyDirectory(directory: string, name: string): Promise<string> {
  while (true) {
    try {
      return await realpath(path.join(directory, "node_modules", name));
    } catch (error) {
      const parent = path.dirname(directory);
      if (parent === directory) {
        throw error;
      }
      directory = parent;
    }
  }
}

/** Removes only a generated directory contained by this package's dist directory. */
async function clean(directory: string): Promise<void> {
  const relative = path.relative(path.join(root, "dist"), path.resolve(directory));
  if (!relative || relative.startsWith("..") || path.isAbsolute(relative)) {
    throw new Error(`Invalid generated directory: ${directory}`);
  }
  await rm(directory, { recursive: true, force: true });
}

/** Captures published text needed when a virtual workspace has no installed Stillwind packages. */
async function capturePackage(
  directory: string,
  prefix: string,
  files: [string, string][],
): Promise<void> {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    if (["src", "node_modules", "typecheck", "tests", ".cache"].includes(entry.name)) {
      continue;
    }
    const file = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      await capturePackage(file, `${prefix}/${entry.name}`, files);
    } else if (/\.(?:[cm]?[jt]s|css|json)$/.test(entry.name)) {
      files.push([`${prefix}/${entry.name}`, await readFile(file, "utf8")]);
    }
  }
}
