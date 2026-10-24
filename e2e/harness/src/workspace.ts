import {
  cp,
  mkdir,
  mkdtemp,
  readdir,
  readFile,
  realpath,
  rm,
  symlink,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { isAbsolute, join, relative, resolve, sep } from "node:path";

const excluded = new Set([
  "node_modules",
  ".e2e",
  "results",
  "dist",
  "build",
  ".fresh-import-map.json",
  ".fresh-native.d.ts",
  "_fresh",
  ".cache",
  ".output",
  ".nuxt",
  ".next",
  ".astro",
  ".svelte-kit",
  "cache",
  ".vite",
  ".vite-plus",
]);

/**
 * Resolves a fixture-relative path and rejects paths outside its work directory.
 */
export function fixturePath(root: string, file: string): string {
  const resolved = resolve(root, file);
  const local = relative(resolve(root), resolved);
  if (
    !local ||
    isAbsolute(file) ||
    isAbsolute(local) ||
    local === ".." ||
    local.startsWith(`..${sep}`)
  ) {
    throw new Error(`Path escapes fixture: ${file}`);
  }
  return resolved;
}

/**
 * Copies application sources while retaining access to the package's dependencies.
 * Gatsby uses the system temporary directory so its LMDB cache can use NTFS on Windows.
 */
export async function createWorkspace(packageRoot: string) {
  const root = resolve(packageRoot);
  const metadata = JSON.parse(
    await readFile(join(root, "package.json"), "utf8").catch((error: NodeJS.ErrnoException) => {
      if (error.code === "ENOENT") {
        return "{}";
      }
      throw error;
    }),
  );
  const workspaceRoot = metadata.dependencies?.gatsby
    ? join(tmpdir(), "stillwind-e2e-gatsby")
    : join(root, ".e2e");
  await mkdir(workspaceRoot, { recursive: true });
  const directory = await mkdtemp(join(workspaceRoot, "run-"));
  try {
    if (metadata.dependencies?.gatsby) {
      const dependencies = join(root, "node_modules");
      for (const entry of await readdir(dependencies)) {
        if (entry.startsWith(".")) {
          continue;
        }
        const names = entry.startsWith("@")
          ? (await readdir(join(dependencies, entry))).map((name) => join(entry, name))
          : [entry];
        for (const name of names) {
          const target = join(directory, "node_modules", name);
          await mkdir(resolve(target, ".."), { recursive: true });
          await symlink(
            await realpath(join(dependencies, name)),
            target,
            process.platform === "win32" ? "junction" : "dir",
          );
        }
      }
    }
    for (const entry of await readdir(root)) {
      if (
        excluded.has(entry) ||
        (entry === "public" && metadata.dependencies?.gatsby) ||
        (entry === "server" && metadata.dependencies?.["@builder.io/qwik"])
      ) {
        continue;
      }
      await cp(join(root, entry), join(directory, entry), {
        recursive: true,
        filter: (source) =>
          !relative(root, source)
            .split(sep)
            .some((part) => excluded.has(part)),
      });
      if (/^tsconfig.*\.json$/.test(entry)) {
        const config = JSON.parse(await readFile(join(root, entry), "utf8"));
        const bases: string[] =
          typeof config.extends === "string"
            ? [config.extends]
            : Array.isArray(config.extends)
              ? config.extends
              : [];
        const relocated = bases.map((base) => {
          if (!base.startsWith(".")) {
            return base;
          }
          const file = resolve(root, base);
          const local = relative(root, file);
          return local === ".." || local.startsWith(`..${sep}`)
            ? relative(directory, file).split(sep).join("/")
            : base;
        });
        if (bases.some((base, index) => base !== relocated[index])) {
          config.extends = Array.isArray(config.extends) ? relocated : relocated[0];
          await writeFile(join(directory, entry), JSON.stringify(config, null, 2));
        }
      }
    }
    if (metadata.name === "@stillwind-internal/e2e-fresh2-vite7-preact10") {
      const {
        dependencies: _dependencies,
        devDependencies: _devDependencies,
        ...runtime
      } = metadata;
      await writeFile(join(directory, "package.json"), JSON.stringify(runtime, null, 2));
    }
  } catch (error) {
    await rm(directory, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
    throw error;
  }
  return {
    directory,
    async dispose() {
      const local = relative(workspaceRoot, directory);
      if (local.startsWith("..") || isAbsolute(local)) {
        throw new Error("Invalid work directory");
      }
      await rm(directory, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
    },
  };
}

/**
 * Changes exactly one literal and returns a function that restores the original bytes.
 */
export async function editFile(root: string, edit: { file: string; from: string; to: string }) {
  const file = fixturePath(root, edit.file);
  const original = await readFile(file);
  const source = original.toString("utf8");
  if (source.split(edit.from).length !== 2) {
    throw new Error(`Expected one occurrence in ${edit.file}: ${edit.from}`);
  }
  await writeFile(file, source.replace(edit.from, edit.to));
  return async () => {
    await writeFile(file, original);
  };
}
