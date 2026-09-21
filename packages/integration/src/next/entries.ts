import { readdir } from "node:fs/promises";
import path from "node:path";

import type { LoaderContext } from "webpack";

import type { TurbopackLoaderOptions } from "../turbopack/project.ts";

export interface NextLoaderOptions extends TurbopackLoaderOptions {
  extensions: string[];
}

export async function routeEntries(
  context: LoaderContext<NextLoaderOptions>,
  options: NextLoaderOptions,
) {
  const entries: string[] = [];
  const visit = async (directory: string, app: boolean): Promise<void> => {
    context.addContextDependency(directory);
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      const file = path.join(directory, entry.name);
      if (entry.isDirectory()) {
        if (!app || !entry.name.startsWith("_")) {
          await visit(file, app);
        }
      } else if (
        entry.isFile() &&
        options.extensions.some((extension) => entry.name.endsWith(`.${extension}`))
      ) {
        const stem = entry.name.slice(0, entry.name.indexOf("."));
        if (
          !app ||
          /^(page|layout|template|loading|error|global-error|not-found|default|forbidden|unauthorized)$/.test(
            stem,
          )
        ) {
          entries.push(file);
        }
      }
    }
  };
  for (const name of ["app", "pages"]) {
    for (const prefix of ["", "src"]) {
      const directory = path.join(options.root, prefix, name);
      try {
        await visit(directory, name === "app");
        break;
      } catch (error) {
        if ((error as NodeJS.ErrnoException).code !== "ENOENT") {
          throw error;
        }
        context.addMissingDependency(directory);
      }
    }
  }
  return entries.sort();
}
