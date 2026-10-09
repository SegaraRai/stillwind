import { readdir } from "node:fs/promises";
import path from "node:path";

import type { LoaderContext } from "webpack";

import type { TurbopackLoaderOptions } from "../turbopack/project.ts";

/** Loader settings that reconstruct document CSS from Next app and pages routes. */
export interface NextLoaderOptions extends TurbopackLoaderOptions {
  /** Configured page extensions without a leading dot, matched case-sensitively. */
  extensions: string[];
}

/**
 * Discovers route sources beneath app or src/app and pages or src/pages.
 * @param context - Loader context that receives visited directory and missing-directory dependencies.
 * @param options - Evaluated absolute project root and Next page extensions.
 * @returns Sorted absolute route paths. App traversal skips private directories and non-route filenames.
 * @throws If traversal fails for a reason other than a missing directory.
 */
export async function routeEntries(
  context: LoaderContext<NextLoaderOptions>,
  options: NextLoaderOptions,
) {
  const entries: string[] = [];
  /**
   * Traverses a route directory and appends matching files to entries.
   * @param directory - Absolute directory registered as a context dependency before reading.
   * @param app - Whether app-directory private folders and route filename constraints apply.
   * @returns Completion of recursive traversal.
   * @throws If enumeration fails.
   */
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
