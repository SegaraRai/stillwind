import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";

import type { IconifyJSON } from "@iconify/types";

/**
 * Icon set given in plugin options: icon set data, an absolute path of an icon set JSON file, or a
 * function that returns icon set data.
 */
export type CollectionSource = IconifyJSON | string | (() => IconifyJSON | Promise<IconifyJSON>);

/** Host inputs that icon set lookup uses. */
export interface CollectionContext {
  /** Absolute project directory, from which installed icon set packages are resolved. */
  readonly root: string;
  /**
   * Registers a file that an icon set comes from.
   *
   * @param path - Absolute file path.
   */
  watch(path: string): void;
}

/** Icon set data and the file that it comes from. */
interface LoadedCollection {
  /** Icon set data, or undefined when no source provides the set. */
  data?: IconifyJSON;
  /** Absolute path of the JSON file, or undefined for data that options supply. */
  file?: string;
}

/**
 * Finds and caches the icon sets of one loaded `@plugin` statement.
 *
 * A set comes from the first source that provides it: the configured sources, then the
 * `@iconify-json/<set>` package, then the `@iconify/json` package. Packages are resolved from the
 * project directory. Each lookup registers the set's file with the host, so a changed file reloads
 * the design system, and the new statement context reads the file again.
 */
export class Collections {
  readonly #sources: Readonly<Record<string, CollectionSource>>;
  readonly #context: CollectionContext;
  readonly #loaded = new Map<string, Promise<LoadedCollection>>();

  /**
   * @param sources - Configured icon sets keyed by set prefix.
   * @param context - Project directory and file watcher.
   */
  constructor(sources: Readonly<Record<string, CollectionSource>>, context: CollectionContext) {
    this.#sources = sources;
    this.#context = context;
  }

  /**
   * Returns one icon set.
   *
   * @param prefix - Icon set prefix, such as `mdi`.
   * @returns The icon set data, or undefined when no source provides the set.
   * @throws If a configured source fails or a JSON file cannot be read or parsed.
   */
  async get(prefix: string): Promise<IconifyJSON | undefined> {
    let loaded = this.#loaded.get(prefix);
    if (!loaded) {
      loaded = this.#load(prefix);
      this.#loaded.set(prefix, loaded);
    }
    const { data, file } = await loaded;
    if (file) {
      this.#context.watch(file);
    }
    return data;
  }

  /**
   * Lists the set prefixes that editors complete: the configured sets and the `@iconify-json/*`
   * packages in the dependencies of the project's `package.json`. The sets of the `@iconify/json`
   * package are omitted, because that package contains every Iconify set.
   *
   * @returns Set prefixes without duplicates.
   */
  async prefixes(): Promise<string[]> {
    const prefixes = new Set(Object.keys(this.#sources));
    const manifest = path.join(this.#context.root, "package.json");
    this.#context.watch(manifest);
    let json: Record<string, Record<string, string> | undefined>;
    try {
      json = JSON.parse(await readFile(manifest, "utf8"));
    } catch {
      // A project without a readable manifest lists only the configured sets.
      return [...prefixes];
    }
    for (const field of ["dependencies", "devDependencies", "optionalDependencies"]) {
      for (const name of Object.keys(json[field] ?? {})) {
        if (name.startsWith("@iconify-json/")) {
          prefixes.add(name.slice("@iconify-json/".length));
        }
      }
    }
    return [...prefixes];
  }

  /**
   * Loads one icon set from the first source that provides it.
   *
   * @param prefix - Icon set prefix.
   * @returns The icon set and its file, or an empty result when no source provides the set.
   * @throws If a configured source fails or a JSON file cannot be read or parsed.
   */
  async #load(prefix: string): Promise<LoadedCollection> {
    const source = Object.hasOwn(this.#sources, prefix) ? this.#sources[prefix] : undefined;
    if (typeof source === "function") {
      return {
        data: await source(),
      };
    }
    if (typeof source === "string") {
      return {
        data: await readCollection(source),
        file: source,
      };
    }
    if (source) {
      return {
        data: source,
      };
    }

    if (/[/\\]|^\.*$/.test(prefix)) {
      return {};
    }

    const require = createRequire(path.join(this.#context.root, "package.json"));
    for (const request of [
      `@iconify-json/${prefix}/icons.json`,
      `@iconify/json/json/${prefix}.json`,
    ]) {
      let file: string;
      try {
        file = require.resolve(request);
      } catch {
        continue;
      }
      return {
        data: await readCollection(file),
        file,
      };
    }

    return {};
  }
}

/**
 * Reads an icon set JSON file.
 *
 * @param file - Absolute file path.
 * @returns The parsed icon set.
 * @throws If reading or parsing fails.
 */
async function readCollection(file: string): Promise<IconifyJSON> {
  return JSON.parse(await readFile(file, "utf8")) as IconifyJSON;
}
