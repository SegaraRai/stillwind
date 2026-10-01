import { cp, mkdir, mkdtemp, readdir, readFile, rm, writeFile } from "node:fs/promises";
import { isAbsolute, join, relative, resolve, sep } from "node:path";

const excluded = new Set([
  "node_modules",
  ".e2e",
  "results",
  "dist",
  "build",
  ".output",
  ".nuxt",
  ".next",
  ".astro",
  ".svelte-kit",
  "cache",
  ".vite",
  ".vite-plus",
]);

/** Resolves a fixture-relative path and rejects paths outside its work directory. */
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

/** Copies application files below their package so dependency resolution uses that package. */
export async function createWorkspace(packageRoot: string) {
  const root = resolve(packageRoot);
  await mkdir(join(root, ".e2e"), { recursive: true });
  const directory = await mkdtemp(join(root, ".e2e", "run-"));
  try {
    for (const entry of await readdir(root)) {
      if (excluded.has(entry)) {
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
        if (typeof config.extends === "string" && config.extends.startsWith(".")) {
          const base = resolve(root, config.extends);
          const local = relative(root, base);
          if (local === ".." || local.startsWith(`..${sep}`)) {
            config.extends = relative(directory, base).split(sep).join("/");
            await writeFile(join(directory, entry), JSON.stringify(config, null, 2));
          }
        }
      }
    }
  } catch (error) {
    await rm(directory, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
    throw error;
  }
  return {
    directory,
    async dispose() {
      const local = relative(join(root, ".e2e"), directory);
      if (local.startsWith("..") || isAbsolute(local)) {
        throw new Error("Invalid work directory");
      }
      await rm(directory, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
    },
  };
}

/** Changes exactly one literal and returns a function that restores the original bytes. */
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
