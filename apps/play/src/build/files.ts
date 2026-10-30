import { resolveVirtualModule } from "@stillwind/host/resolution";

/**
 * Reads runtime files from one immutable build revision without transferring unused dependencies.
 */
export class BuildFiles {
  readonly #resolution: Map<string, string>;
  readonly #contents = new Map<string, Promise<string>>();
  readonly #load: (file: string) => Promise<string>;

  /**
   * @param files - Absolute paths paired with package manifest text or null for deferred contents.
   * @param load - Reads a listed file from the captured workspace revision and rejects obsolete reads.
   */
  constructor(
    files: readonly (readonly [string, string | null])[],
    load: (file: string) => Promise<string>,
  ) {
    this.#resolution = new Map(files.map(([file, text]) => [file, text ?? ""]));
    this.#load = load;
    for (const [file, text] of files) {
      if (text !== null) {
        this.#contents.set(file, Promise.resolve(text));
      }
    }
  }

  /**
   * Resolves a runtime import using file identities and package manifests from this revision.
   * @param source - Relative, absolute, or package import specifier.
   * @param importer - Absolute virtual path of the importing module.
   * @returns A listed file identity, or undefined when resolution fails.
   * @throws If a consulted package manifest contains invalid JSON.
   */
  resolve(source: string, importer: string): string | undefined {
    return resolveVirtualModule(this.#resolution, source, importer);
  }

  /**
   * Reads a file once for this build, including empty files and explicitly imported declarations or JSON.
   *
   * @param file - Absolute virtual path listed in the captured inventory.
   * @returns File text. Concurrent requests share the same read.
   * @throws If the file is absent, the workspace revision changed, or the provider cannot read it.
   */
  async read(file: string): Promise<string> {
    if (!this.#resolution.has(file)) {
      throw new Error(`Missing virtual file ${file}`);
    }
    let contents = this.#contents.get(file);
    if (!contents) {
      contents = this.#load(file);
      this.#contents.set(file, contents);
    }
    return contents;
  }
}
