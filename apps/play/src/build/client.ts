import type * as vscode from "vscode";

/** Output of a complete browser build from one revision of the virtual workspace. */
export interface ApplicationBuild {
  /** Browser bundle whose React imports use the preview's persistent renderer. */
  code: string;
  /** Styles from Stillwind and authored CSS imports for this build. */
  css: string;
  /** Absolute virtual source paths paired with code after Stillwind macro transformation. */
  transformed: [string, string][];
}

/** Builds the application in a worker and requests Stillwind transformations from the active extension. */
export class ApplicationBuilder {
  readonly #worker = new Worker(new URL("./worker.ts", import.meta.url), { type: "module" });
  readonly #api: typeof vscode;
  readonly #pending = new Map<
    number,
    {
      resolve(result: ApplicationBuild): void;
      reject(error: Error): void;
    }
  >();

  #sequence = 0;

  /**
   * @param api - VS Code API containing the activated public Stillwind extension.
   * @param declarations - Installs generated vocabulary into the editing workspace.
   */
  constructor(api: typeof vscode, declarations: (files: [string, string][]) => Promise<void>) {
    this.#api = api;
    this.#worker.onmessage = async ({ data }) => {
      if (data.kind === "compile") {
        try {
          const value = await api.commands.executeCommand("stillwind.compile", {
            uri: data.uri,
            revision: data.revision,
            generation: data.generation,
          });
          this.#worker.postMessage({ ...data, kind: "compile-reply", value });
        } catch (error) {
          this.#worker.postMessage({ ...data, kind: "compile-reply", error: String(error) });
        }
      } else {
        const pending = this.#pending.get(data.id);
        this.#pending.delete(data.id);
        if (data.kind === "failed") {
          pending?.reject(new Error(data.error));
        } else {
          try {
            await declarations(data.declarations);
            pending?.resolve(data);
          } catch (error) {
            pending?.reject(new Error(String(error)));
          }
        }
      }
    };
    this.#worker.onerror = (event) => {
      for (const pending of this.#pending.values()) {
        pending.reject(new Error(event.message));
      }
      this.#pending.clear();
    };
  }

  /**
   * Captures saved files and open buffers before bundling. Obsolete snapshots are retried twice.
   * @param reload - Reevaluate configuration before capturing the first snapshot.
   * @returns Code and CSS from the same captured revision.
   * @throws If compilation fails or edits invalidate all three attempts.
   */
  async build(reload = false): Promise<ApplicationBuild> {
    for (let attempt = 0; ; attempt++) {
      try {
        return await this.#build(reload && attempt === 0);
      } catch (error) {
        if (attempt >= 2 || !/obsolete|revision/i.test(String(error))) {
          throw error;
        }
      }
    }
  }

  async #build(reload: boolean): Promise<ApplicationBuild> {
    const snapshot = await this.#api.commands.executeCommand<{
      files: [string, string][];
      revision: number;
      generation: number;
    }>("stillwind.snapshot", { reload });
    if (!snapshot) {
      throw new Error("Stillwind extension has not opened the workspace");
    }
    const id = ++this.#sequence;
    return new Promise((resolve, reject) => {
      this.#pending.set(id, { resolve, reject });
      this.#worker.postMessage({ kind: "build", id, ...snapshot });
    });
  }

  /** Terminates the build worker and rejects unfinished requests. */
  dispose(): void {
    this.#worker.terminate();
    for (const pending of this.#pending.values()) {
      pending.reject(new Error("Application builder is closed"));
    }
    this.#pending.clear();
  }
}
