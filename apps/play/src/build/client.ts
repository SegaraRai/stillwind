import type { BrowserStylesheetOutput } from "@stillwind/host/browser";
import type * as vscode from "vscode";

/**
 * Output of a complete browser build from one revision of the virtual workspace.
 */
export interface ApplicationBuild {
  /** Browser bundle whose React imports use the preview's persistent renderer. */
  code: string;
  /** Styles from Stillwind and authored CSS imports for this build. */
  css: string;
  /** Absolute virtual source paths paired with code after Stillwind macro transformation. */
  transformed: [string, string][];
  /**
   * Absolute virtual source paths paired with the CSS that Stillwind generated for the module.
   * The compiler omits rules that a module compiled earlier in the same build already emitted.
   */
  moduleCss: [string, string][];
  /** Naming fallbacks observed during compilation and stylesheet assembly. */
  rejectedNames: BrowserStylesheetOutput["rejectedNames"];
}

/**
 * Builds the application in a worker and requests Stillwind transformations from the active extension.
 */
export class ApplicationBuilder {
  #worker?: Worker;
  #closed = false;
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
   */
  constructor(api: typeof vscode) {
    this.#api = api;
  }

  #startWorker(): Worker {
    const worker = new Worker(new URL("./worker.ts", import.meta.url), {
      type: "module",
    });
    this.#worker = worker;
    worker.onmessage = async ({ data }) => {
      if (this.#worker !== worker) {
        return;
      }
      if (data.kind === "compile") {
        try {
          const value = await this.#api.commands.executeCommand("stillwind.compile", {
            uri: data.uri,
            revision: data.revision,
            generation: data.generation,
          });
          this.#post(worker, {
            ...data,
            kind: "compile-reply",
            value,
          });
        } catch (error) {
          this.#post(worker, {
            ...data,
            kind: "compile-reply",
            error: String(error),
          });
        }
      } else {
        const pending = this.#pending.get(data.id);
        this.#pending.delete(data.id);
        if (data.kind === "failed") {
          pending?.reject(new Error(data.error));
        } else {
          pending?.resolve(data);
        }
      }
    };
    worker.onerror = (event) => {
      this.#failWorker(worker, new Error(event.message || "Application build worker failed"));
    };
    worker.onmessageerror = () => {
      this.#failWorker(
        worker,
        new Error("Cannot read a message from the application build worker"),
      );
    };
    return worker;
  }

  #failWorker(worker: Worker, error: Error): void {
    if (this.#worker !== worker) {
      return;
    }
    this.#worker = undefined;
    worker.onmessage = null;
    worker.onerror = null;
    worker.onmessageerror = null;
    worker.terminate();
    for (const pending of this.#pending.values()) {
      pending.reject(error);
    }
    this.#pending.clear();
  }

  #post(worker: Worker, message: unknown): void {
    if (this.#worker !== worker) {
      return;
    }
    try {
      worker.postMessage(message);
    } catch (error) {
      this.#failWorker(worker, error instanceof Error ? error : new Error(String(error)));
    }
  }

  /**
   * Captures saved files and open buffers before bundling. Obsolete snapshots are retried twice.
   * A worker failure rejects unfinished builds. A later build starts a replacement worker.
   *
   * @param reload - Reevaluate configuration before capturing the first snapshot.
   * @returns Code and CSS from the same captured revision.
   * @throws If the builder is closed, the worker fails, compilation fails, or edits invalidate all three attempts.
   */
  async build(reload = false): Promise<ApplicationBuild> {
    if (this.#closed) {
      throw new Error("Application builder is closed");
    }
    const worker = this.#worker ?? this.#startWorker();
    for (let attempt = 0; ; attempt++) {
      try {
        return await this.#build(worker, reload && attempt === 0);
      } catch (error) {
        if (this.#worker !== worker || attempt >= 2 || !/obsolete|revision/i.test(String(error))) {
          throw error;
        }
      }
    }
  }

  #build(worker: Worker, reload: boolean): Promise<ApplicationBuild> {
    const id = ++this.#sequence;
    return new Promise((resolve, reject) => {
      this.#pending.set(id, {
        resolve,
        reject,
      });
      const capture = async () => {
        const snapshot = await this.#api.commands.executeCommand<{
          files: [string, string][];
          revision: number;
          generation: number;
        }>("stillwind.snapshot", {
          reload,
        });
        if (!snapshot) {
          throw new Error("Stillwind extension has not opened the workspace");
        }
        this.#post(worker, {
          kind: "build",
          id,
          ...snapshot,
        });
      };
      void capture().catch((error: unknown) => {
        this.#pending.delete(id);
        reject(error instanceof Error ? error : new Error(String(error)));
      });
    });
  }

  /**
   * Terminates the build worker and rejects unfinished requests.
   */
  dispose(): void {
    this.#closed = true;
    if (this.#worker) {
      this.#failWorker(this.#worker, new Error("Application builder is closed"));
    }
  }
}
