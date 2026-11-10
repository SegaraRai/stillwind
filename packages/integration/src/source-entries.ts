/**
 * Selects the source files that a build loads into the checker before its first compilation.
 *
 * The main entry points are {@link isSourceEntry} and {@link buildSourceEntries}.
 */
import { listProjectFiles } from "@stillwind/config/node";
import type { ProjectScope } from "@stillwind/config/node";

/**
 * Tests whether a file can be a compilation entry.
 *
 * @param id - Path without a query.
 * @returns True for script and component files. Declaration files are not entries.
 */
export function isSourceEntry(id: string): boolean {
  return /\.(?:[cm]?[jt]sx?|astro|svelte|vue)$/.test(id) && !/\.d\.[cm]?ts$/.test(id);
}

/**
 * Lists the build entries together with the source files of the selected project.
 *
 * A build can reach source files only through dynamic imports, which preparation does not follow.
 * Each source that a later compilation adds then rebuilds the checker project, so a build loads
 * these files with its first compilation.
 *
 * @param entries - Absolute entry paths of the build.
 * @param scope - Target globs of the selected project, or undefined when no configuration file
 * selects a project.
 * @returns The entries followed by the script and component files that the scope selects, without
 * duplicates.
 * @throws If filesystem enumeration fails.
 */
export async function buildSourceEntries(
  entries: readonly string[],
  scope: ProjectScope | undefined,
): Promise<string[]> {
  return [
    ...new Set([...entries, ...(scope ? await listProjectFiles(scope) : []).filter(isSourceEntry)]),
  ];
}
