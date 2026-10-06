import { fork, type ChildProcess } from "node:child_process";
import { fileURLToPath } from "node:url";

import type { WorkspaceResult } from "@stillwind/host";

/** Requests sent to the process that owns trusted config evaluation and plugin lifetimes. */
export type WorkerRequest =
  | {
      method: "open";
      config: string;
    }
  | {
      method: "lint";
      patterns?: string[];
    }
  | {
      method: "analyze";
      file: string;
    }
  | {
      method: "update";
      file: string;
      source: string;
      version: number;
    }
  | {
      method: "closeDocument";
      file: string;
    }
  | {
      method: "invalidate";
    }
  | {
      method: "close";
    };

export interface WorkerReply {
  id: number;
  value?: unknown;
  error?: string;
}

/** Isolates configuration and plugin logs from CLI JSON and the LSP stdout transport. */
export class AnalysisWorker {
  readonly #child: ChildProcess;
  readonly #pending = new Map<
    number,
    { resolve: (value: unknown) => void; reject: (error: Error) => void }
  >();
  readonly #closed: Promise<void>;
  #nextId = 0;
  #exited = false;
  #closing = false;

  constructor() {
    this.#child = fork(fileURLToPath(new URL("./worker.js", import.meta.url)), [], {
      stdio: ["ignore", "pipe", "pipe", "ipc"],
      execArgv: [],
      windowsHide: true,
    });
    this.#child.stdout!.on("data", (data: Buffer) => {
      process.stderr.write(data);
    });
    this.#child.stderr!.on("data", (data: Buffer) => {
      process.stderr.write(data);
    });
    this.#child.on("message", (reply: WorkerReply) => {
      const request = this.#pending.get(reply.id);
      this.#pending.delete(reply.id);
      if (reply.error) {
        request?.reject(new Error(reply.error));
      } else {
        request?.resolve(reply.value);
      }
    });
    this.#child.on("error", (error) => {
      for (const request of this.#pending.values()) {
        request.reject(error);
      }
      this.#pending.clear();
    });
    this.#closed = new Promise((resolve) => {
      this.#child.on("close", (code, signal) => {
        this.#exited = true;
        for (const request of this.#pending.values()) {
          request.reject(new Error(`Stillwind analysis worker closed (${code ?? signal})`));
        }
        this.#pending.clear();
        resolve();
      });
    });
  }

  async request<T = void>(request: WorkerRequest): Promise<T> {
    if (this.#exited) {
      throw new Error("Stillwind analysis worker is closed");
    }
    const id = this.#nextId++;
    return (await new Promise<unknown>((resolve, reject) => {
      this.#pending.set(id, { resolve, reject });
      this.#child.send({ id, request }, (error) => {
        if (error) {
          this.#pending.delete(id);
          reject(error);
        }
      });
    })) as T;
  }

  async open(config: string): Promise<void> {
    await this.request({
      method: "open",
      config,
    });
  }

  analyze(file: string): Promise<WorkspaceResult[]> {
    return this.request({
      method: "analyze",
      file,
    });
  }

  lint(patterns?: string[]): Promise<WorkspaceResult[]> {
    return this.request({
      method: "lint",
      patterns,
    });
  }

  /** Closes project hosts, disconnects IPC, and waits for child process close before returning. */
  async close(): Promise<void> {
    if (this.#closing || this.#exited) {
      await this.#closed;
      return;
    }
    this.#closing = true;
    try {
      await this.request({
        method: "close",
      });
    } finally {
      const timer = setTimeout(() => {
        this.#child.kill();
      }, 5000);
      await this.#closed;
      clearTimeout(timer);
    }
  }
}
