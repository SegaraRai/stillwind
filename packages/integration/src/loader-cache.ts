/**
 * Reuses values that bundler loaders rebuild from the same files in one process.
 *
 * webpack, Rspack, and Turbopack create a loader context for each module, and Turbopack runs loaders
 * in worker processes that serve many modules. Evaluated configuration and a loaded design system
 * with its compiler session can serve every call while the files that they read stay unchanged.
 *
 * The main entry points are {@link LoaderCache} and {@link recordInputs}.
 */
import { createHash } from "node:crypto";

/**
 * Reads a file to check a cached value.
 *
 * @param id - Absolute file path.
 * @returns A promise for the file text, or undefined when the file does not exist.
 * @throws If reading fails for another reason.
 */
export type InputReader = (id: string) => Promise<string | undefined>;

/** Files that the creation of a cached value read or registered as dependencies. */
export interface LoaderInputs {
  /**
   * SHA-256 digests of file contents, keyed by absolute path. Undefined marks a file that was
   * registered after creation, whose next check records its digest.
   */
  files: Map<string, string | undefined>;
  /** Absolute paths that did not exist. */
  missing: Set<string>;
  /** Directories whose listings affect the value. A value with directories is never reused. */
  directories: Set<string>;
}

/** Use of a cached value by one loader call. */
export interface LoaderLease<T> {
  /** Shared value. The caller must not use it after {@link LoaderLease.done}. */
  value: T;
  /** Files to register as dependencies of the loader result. */
  inputs: LoaderInputs;
  /** Ends this use. Calling it again has no effect. */
  done: () => void;
}

/** One cached value with its inputs and current uses. */
interface CacheEntry<T> {
  value: T;
  inputs: LoaderInputs;
  /** Number of leases that have not ended. */
  active: number;
  /** Whether a newer value replaced this one, so that the last lease closes it. */
  replaced: boolean;
  /** Timer that closes the value after the last lease ends, if it is still idle. */
  idle?: ReturnType<typeof setTimeout>;
}

/**
 * Cache of values keyed by the settings that create them.
 *
 * Concurrent calls with the same key share one creation. A value is closed when a newer value
 * replaces it and no lease uses it, and also after it has been idle for the configured time. A
 * closed value must stay usable, because an idle value remains in the cache.
 */
export class LoaderCache<T> {
  readonly #entries = new Map<string, Promise<CacheEntry<T>>>();
  readonly #close: (value: T) => void;
  readonly #idleMs: number;

  /**
   * @param close - Releases resources of a value, such as a checker process. The value must stay
   * usable after this call.
   * @param idleMs - Milliseconds without leases after which a value is closed. The timer does not
   * keep the process alive.
   */
  constructor(close: (value: T) => void = () => {}, idleMs = Number.POSITIVE_INFINITY) {
    this.#close = close;
    this.#idleMs = idleMs;
  }

  /**
   * Returns the cached value for a key, or creates one when it is missing or its inputs changed.
   *
   * @param key - Serialized settings that fully determine the created value.
   * @param read - Reader used to compare the recorded inputs with the current files.
   * @param create - Creates the value and records the files that it depends on.
   * @returns A lease for the value. The caller must end it after its last use.
   * @throws If reading an input fails, or if creation fails. A failed creation is not cached.
   */
  async use(
    key: string,
    read: InputReader,
    create: () => Promise<{ value: T; inputs: LoaderInputs }>,
  ): Promise<LoaderLease<T>> {
    for (;;) {
      const cached = this.#entries.get(key);
      if (!cached) {
        const created = create().then(({ value, inputs }) => ({
          value,
          inputs,
          active: 0,
          replaced: false,
        }));
        this.#entries.set(key, created);
        created.catch(() => {
          if (this.#entries.get(key) === created) {
            this.#entries.delete(key);
          }
        });
        return this.#lease(await created);
      }
      const entry = await cached.catch(() => undefined);
      if (!entry) {
        continue;
      }
      // Hold the entry while its inputs are read, so that a replacement does not close it early.
      const lease = this.#lease(entry);
      if (await unchanged(entry.inputs, read).catch(() => false)) {
        return lease;
      }
      if (this.#entries.get(key) === cached) {
        this.#entries.delete(key);
        entry.replaced = true;
      }
      lease.done();
    }
  }

  /** Closes every value that no lease uses. Values in use close when their last lease ends. */
  closeIdle(): void {
    for (const pending of this.#entries.values()) {
      void pending.then(
        (entry) => {
          if (!entry.active) {
            clearTimeout(entry.idle);
            this.#close(entry.value);
          }
        },
        () => {},
      );
    }
  }

  #lease(entry: CacheEntry<T>): LoaderLease<T> {
    clearTimeout(entry.idle);
    entry.idle = undefined;
    entry.active += 1;
    let ended = false;
    return {
      value: entry.value,
      inputs: entry.inputs,
      done: () => {
        if (ended) {
          return;
        }
        ended = true;
        entry.active -= 1;
        if (entry.active) {
          return;
        }
        if (entry.replaced) {
          this.#close(entry.value);
        } else if (Number.isFinite(this.#idleMs)) {
          entry.idle = setTimeout(() => {
            entry.idle = undefined;
            this.#close(entry.value);
          }, this.#idleMs);
          entry.idle.unref?.();
        }
      },
    };
  }
}

/**
 * Checks whether recorded inputs still match the files.
 *
 * @param inputs - Inputs to compare. Files registered after creation receive their first digest.
 * @param read - Reader for the current files.
 * @returns A promise for true when every file has its recorded contents, every missing file is
 * still missing, and no directory was recorded.
 * @throws If reading a file fails for a reason other than its absence.
 */
async function unchanged(inputs: LoaderInputs, read: InputReader): Promise<boolean> {
  if (inputs.directories.size) {
    return false;
  }
  const files = await Promise.all(
    [...inputs.files].map(async ([id, recorded]) => {
      const text = await read(id);
      if (text === undefined) {
        return false;
      }
      if (recorded === undefined) {
        inputs.files.set(id, digest(text));
        return true;
      }
      return digest(text) === recorded;
    }),
  );
  const missing = await Promise.all(
    [...inputs.missing].map(async (id) => (await read(id)) === undefined),
  );
  return files.every(Boolean) && missing.every(Boolean);
}

/** Recorder of the files that one value creation reads and registers. */
export interface InputRecorder {
  /** Recorded inputs. Registrations after {@link InputRecorder.finish} still add files. */
  inputs: LoaderInputs;
  /**
   * Reads a file and records its digest.
   *
   * @param id - Absolute file path.
   * @returns A promise for the file text.
   * @throws If the file cannot be read.
   */
  read: (id: string) => Promise<string>;
  /**
   * Records a file dependency. A file that creation did not read receives its digest in
   * {@link InputRecorder.finish}.
   *
   * @param id - Absolute file path.
   */
  watch: (id: string) => void;
  /**
   * Records a missing file dependency.
   *
   * @param id - Absolute file path.
   */
  watchMissing: (id: string) => void;
  /**
   * Records a directory dependency, which prevents reuse of the value.
   *
   * @param id - Absolute directory path.
   */
  watchDirectory: (id: string) => void;
  /**
   * Records digests of registered files that creation did not read.
   *
   * @returns A promise for the recorded inputs.
   * @throws If reading a file fails for a reason other than its absence.
   */
  finish: () => Promise<LoaderInputs>;
}

/**
 * Creates a recorder for the files that one value creation depends on.
 *
 * @param read - Reader through the bundler filesystem. It resolves undefined for a missing file.
 * @returns A recorder whose reads reject for a missing file.
 */
export function recordInputs(read: InputReader): InputRecorder {
  const inputs: LoaderInputs = {
    files: new Map(),
    missing: new Set(),
    directories: new Set(),
  };
  let finished = false;
  const watched = new Set<string>();
  return {
    inputs,
    async read(id) {
      const text = await read(id);
      if (text === undefined) {
        throw Object.assign(new Error(`ENOENT: no such file or directory, open '${id}'`), {
          code: "ENOENT",
          path: id,
        });
      }
      inputs.files.set(id, digest(text));
      return text;
    },
    watch(id) {
      if (finished) {
        if (!inputs.files.has(id)) {
          inputs.files.set(id, undefined);
        }
        return;
      }
      watched.add(id);
    },
    watchMissing(id) {
      inputs.missing.add(id);
    },
    watchDirectory(id) {
      inputs.directories.add(id);
    },
    async finish() {
      await Promise.all(
        [...watched].map(async (id) => {
          if (inputs.files.has(id)) {
            return;
          }
          const text = await read(id);
          if (text === undefined) {
            inputs.missing.add(id);
          } else {
            inputs.files.set(id, digest(text));
          }
        }),
      );
      finished = true;
      return inputs;
    },
  };
}

/**
 * Hashes file text for input comparison.
 *
 * @param text - Exact file contents.
 * @returns A SHA-256 digest encoded as hexadecimal.
 */
function digest(text: string): string {
  return createHash("sha256").update(text).digest("hex");
}
