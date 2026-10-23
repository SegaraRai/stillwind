import type { IconifyJSON } from "@iconify/types";
import type { PluginContext } from "@stillwind/config";

/**
 * A custom icon set: Iconify data, an absolute JSON file path, or a function that
 * returns Iconify data synchronously or asynchronously.
 */
export type CollectionSource = IconifyJSON | string | (() => IconifyJSON | Promise<IconifyJSON>);

/** Host operations that icon set lookup uses. */
export type CollectionContext = Pick<PluginContext, "root" | "watch" | "resolve" | "read">;

/** Icon set data and the file that it comes from. */
interface LoadedCollection {
  /** Icon set data, or undefined when no source provides the set. */
  data?: IconifyJSON;
  /** Absolute path of the JSON file, or undefined for data that options supply. */
  file?: string;
}

/**
 * Parsed icon sets keyed by file path, with the text that they were parsed from. Statements of
 * later loads reuse the data while the text is unchanged, because large sets take long to parse.
 */
const parsed = new Map<
  string,
  {
    /** Text of the file when it was parsed. */
    text: string;
    /** Parsed icon set. */
    data: IconifyJSON;
  }
>();

/**
 * Finds and caches the icon sets of one loaded `@plugin` statement.
 *
 * A set comes from the first source that provides it: the configured sources, then the
 * `@iconify-json/<set>` package, then the `@iconify/json` package. Packages are resolved by the
 * host from the stylesheet that contains the statement. Each lookup registers the set's file with
 * the host, so a changed file reloads the design system, and the new statement context reads the
 * file again.
 */
export class Collections {
  readonly #sources: Readonly<Record<string, CollectionSource>>;
  readonly #context: CollectionContext;
  readonly #loaded = new Map<string, Promise<LoadedCollection>>();
  #bundled?: Promise<string[]>;

  /**
   * @param sources - Configured icon sets keyed by set prefix.
   * @param context - Project directory and host file operations.
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
   * Lists the set prefixes whose icons editors always complete: the configured sets and the
   * `@iconify-json/*` packages in the dependencies of the project's `package.json`. The sets of the
   * `@iconify/json` package are omitted, because that package contains every Iconify set.
   *
   * @returns Set prefixes without duplicates.
   */
  async prefixes(): Promise<string[]> {
    const prefixes = new Set(Object.keys(this.#sources));
    const manifest = `${this.#context.root.replace(/[\\/]$/, "")}/package.json`;
    this.#context.watch(manifest);
    let json: Record<string, Record<string, string> | undefined>;
    try {
      json = JSON.parse(await this.#context.read(manifest));
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
   * Lists the set prefixes of the `@iconify/json` package from its `collections.json` file, and
   * registers that file with the host.
   *
   * @returns Set prefixes, or an empty array when the package is not installed or its file cannot
   * be read.
   */
  bundledPrefixes(): Promise<string[]> {
    this.#bundled ??= (async () => {
      const file = await this.#context.resolve("@iconify/json/collections.json");
      if (!file) {
        return [];
      }
      this.#context.watch(file);
      try {
        return Object.keys(JSON.parse(await this.#context.read(file)) as Record<string, unknown>);
      } catch {
        // An unreadable list leaves completion with the sets from other sources.
        return [];
      }
    })();
    return this.#bundled;
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
        data: await this.#read(source),
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

    for (const request of [
      `@iconify-json/${prefix}/icons.json`,
      `@iconify/json/json/${prefix}.json`,
    ]) {
      const file = await this.#context.resolve(request);
      if (file) {
        return {
          data: await this.#read(file),
          file,
        };
      }
    }

    return {};
  }

  /**
   * Reads and parses an icon set JSON file, reusing earlier data for unchanged text.
   *
   * @param file - Absolute file path.
   * @returns The parsed icon set.
   * @throws If reading or parsing fails.
   */
  async #read(file: string): Promise<IconifyJSON> {
    const text = await this.#context.read(file);
    const cached = parsed.get(file);
    if (cached?.text === text) {
      return cached.data;
    }
    const data = JSON.parse(text) as IconifyJSON;
    parsed.set(file, {
      text,
      data,
    });
    return data;
  }
}
