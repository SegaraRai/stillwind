import { spawn } from "node:child_process";
import { readFile, readdir, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

/** Runs Fresh with Deno and maps unpublished workspace entrypoints to their built files. */
export async function runFresh(root: string, mode: string, port: number): Promise<number> {
  const imports = {
    ...JSON.parse(await readFile(join(root, "deno.json"), "utf8")).imports,
  };
  const packages = fileURLToPath(new URL("../../../packages/", import.meta.url));
  for (const entry of await readdir(packages, { withFileTypes: true })) {
    if (!entry.isDirectory()) {
      continue;
    }
    const directory = join(packages, entry.name);
    const metadata = JSON.parse(await readFile(join(directory, "package.json"), "utf8"));
    if (
      ![
        "@stillwind/config",
        "@stillwind/host",
        "@stillwind/integration",
        "@stillwind/native",
        "@stillwind/projection",
        "stillwind",
      ].includes(metadata.name)
    ) {
      continue;
    }
    const localRequire = createRequire(join(directory, "package.json"));
    for (const name of Object.keys(metadata.dependencies ?? {})) {
      const canonical =
        metadata.dependencies?.[name]?.match(/^npm:((?:@[^/]+\/)?[^@]+)/)?.[1] ?? name;
      if (imports[name] || name === "stillwind" || name.startsWith("@stillwind/")) {
        continue;
      }
      let dependency: string;
      try {
        dependency = dirname(localRequire.resolve(name));
      } catch {
        continue;
      }
      while (dirname(dependency) !== dependency) {
        try {
          const installed = JSON.parse(await readFile(join(dependency, "package.json"), "utf8"));
          if (installed.name === canonical) {
            imports[name] = `npm:${canonical}@${installed.version}`;
            for (const key of Object.keys(installed.exports ?? {})) {
              if (key.startsWith("./") && !key.includes("*")) {
                imports[name + key.slice(1)] =
                  `npm:${canonical}@${installed.version}${key.slice(1)}`;
              }
            }
            break;
          }
        } catch {
          /* An entrypoint can be below directories without package metadata. */
        }
        dependency = dirname(dependency);
      }
    }
    for (const [key, value] of Object.entries(metadata.exports ?? {})) {
      const entry = value as {
        default?: string;
        types?: string;
      };
      const target =
        typeof value === "string"
          ? value
          : ((mode === "check" ? entry.types : undefined) ?? entry.default);
      if (target && !key.includes("*")) {
        imports[metadata.name + (key === "." ? "" : key.slice(1))] = pathToFileURL(
          join(directory, target),
        ).href;
      }
    }
    if (mode === "check") {
      if (metadata.name === "@stillwind/native") {
        const binding = join(directory, metadata.exports["."].types);
        const declaration = join(root, ".fresh-native.d.ts");
        // File URL declarations do not receive Deno's implicit npm Node globals.
        await writeFile(
          declaration,
          'import type { Buffer } from "node:buffer";\n' + (await readFile(binding, "utf8")),
        );
        imports[metadata.name] = pathToFileURL(declaration).href;
        imports[pathToFileURL(binding).href] = pathToFileURL(declaration).href;
      }
      for (const declaration of await readdir(join(directory, "dist"), {
        recursive: true,
        withFileTypes: true,
      })) {
        if (declaration.isFile() && declaration.name.endsWith(".d.ts")) {
          const file = pathToFileURL(join(declaration.parentPath, declaration.name)).href;
          imports[file.replace(/\.d\.ts$/, ".js")] = file;
        }
      }
    }
  }
  const importMap = join(root, ".fresh-import-map.json");
  await writeFile(importMap, JSON.stringify({ imports }));
  const sources = [
    "runner.ts",
    "vite.fresh.config.ts",
    "main.ts",
    "src/App.tsx",
    "routes/_app.tsx",
    "routes/next.tsx",
  ];
  const runDeno = (args: string[]) => {
    const child = spawn("deno", args, {
      cwd: root,
      stdio: "inherit",
      windowsHide: true,
      env: {
        ...process.env,
        DENO_NO_PACKAGE_JSON: "1",
      },
    });
    return new Promise<number | null>((resolve, reject) => {
      child.once("error", reject);
      child.once("exit", resolve);
    });
  };
  if (mode !== "check") {
    const cached = await runDeno([
      "cache",
      "--unstable-bare-node-builtins",
      "--import-map",
      importMap,
      ...sources,
    ]);
    if (cached !== 0) {
      throw new Error(`Deno cache exited with code ${cached}`);
    }
  }
  const args =
    mode === "check"
      ? ["check", "--import-map", importMap, ...sources]
      : [
          "run",
          "-A",
          "--unstable-bare-node-builtins",
          "--import-map",
          importMap,
          "runner.ts",
          mode,
          String(port),
        ];
  return (await runDeno(args)) ?? 1;
}
