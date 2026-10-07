import { fork, type ChildProcess } from "node:child_process";
import { fileURLToPath } from "node:url";

import type { EditorRequest, WorkspaceResult } from "@stillwind/host";

import type { RewriteInput, RewriteResult } from "./rewrite.ts";

/** Requests sent to the process that owns trusted config evaluation and plugin lifetimes. */
export type WorkerRequest =
  | {
      method: "open";
      /** Configuration path evaluated in the child process, replacing any previously opened workspace. */
      config: string;
    }
  | {
      method: "lint";
      /** Additional target globs relative to each project root. Omission checks all configured targets. */
      patterns?: readonly string[];
    }
  | {
      method: "targets";
      patterns?: readonly string[];
    }
  | {
      method: "rewrite";
      inputs: readonly RewriteInput[];
      operation: "sort" | "fixes";
    }
  | {
      method: "analyze";
      /** Absolute target path. Each project whose scope contains the file returns a result. */
      file: string;
    }
  | {
      method: "query";
      /** Absolute target file path. */
      file: string;
      /** Editor operation in original UTF-16 coordinates. */
      request: EditorRequest;
    }
  | {
      method: "update";
      /** Absolute source path, including dependencies outside target scopes. */
      file: string;
      /** Complete unsaved source text. */
      source: string;
      /** Safe integer that increases while this file is open. */
      version: number;
    }
  | {
      method: "closeDocument";
      /** Absolute path of the buffer to remove. Unknown files have no effect. */
      file: string;
    }
  | {
      method: "invalidate";
    }
  | {
      method: "close";
    };

/** IPC response correlated with one request. Successful void operations can omit value. */
export interface WorkerReply {
  /** Request sequence number assigned by AnalysisWorker. */
  id: number;
  /** Serialized operation result when error is absent. */
  value?: unknown;
  /** Stringified exception from the child operation. */
  error?: string;
}

/** Isolates configuration and plugin logs from CLI JSON and the LSP stdout transport. */
export class AnalysisWorker {
  readonly #child: ChildProcess;
  readonly #pending = new Map<
    number,
    {
      resolve: (value: unknown) => void;
      reject: (error: Error) => void;
    }
  >();
  readonly #closed: Promise<void>;

  #nextId = 0;
  #exited = false;
  #closing = false;

  /** Starts the child process immediately. Call open before analysis and close when finished. */
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

  /**
   * Sends an IPC operation and correlates its reply. Calls are not serialized here.
   *
   * @param request - Operation payload. Await open before using workspace operations, and avoid
   * overlapping replacement or closure with analysis requests.
   * @returns The reply value cast to T. The caller must choose T to match the operation's result.
   * @throws If the child is closed, IPC delivery fails, or the child operation reports an error.
   */
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

  /**
   * Replaces the child workspace, closing any previously opened hosts first.
   *
   * @param config - Configuration path. Relative paths use the child's inherited working directory.
   * @returns A promise that resolves after the configuration and project hosts have opened.
   * @throws If configuration evaluation, project initialization, or transport fails.
   */
  async open(config: string): Promise<void> {
    await this.request({
      method: "open",
      config,
    });
  }

  /**
   * Analyzes a target in each matching project of the opened workspace.
   *
   * @param file - Absolute target path. Unsaved buffers use preceding update requests.
   * @returns Scoped results in project order, or an empty list when no scope includes the target.
   * @throws If the workspace has not opened or transport fails.
   */
  analyze(file: string): Promise<WorkspaceResult[]> {
    return this.request({
      method: "analyze",
      file,
    });
  }

  /**
   * Enumerates and analyzes targets in the opened workspace.
   *
   * @param patterns - Additional globs relative to each project root. Empty or omitted filters
   * retain every configured target.
   * @returns Results in project order and sorted target order within each project.
   * @throws If the workspace has not opened, enumeration fails, or transport fails.
   */
  lint(patterns?: readonly string[]): Promise<WorkspaceResult[]> {
    return this.request({
      method: "lint",
      patterns,
    });
  }

  /** Returns distinct targets in configured scopes, filtered by project-relative globs. */
  targets(patterns?: readonly string[]): Promise<string[]> {
    return this.request({
      method: "targets",
      patterns,
    });
  }

  /** Computes replacements from captured buffers without writing files. Throws on incomplete or conflicting results. */
  rewrite(inputs: readonly RewriteInput[], operation: "sort" | "fixes"): Promise<RewriteResult[]> {
    return this.request({
      method: "rewrite",
      inputs,
      operation,
    });
  }

  /**
   * Requests host cleanup and waits for the child process to close. After the cleanup request
   * settles, a child that remains alive for five seconds is terminated.
   *
   * @returns A promise that resolves after process closure. Repeated calls wait for that closure.
   * @throws If the initial cleanup request or its transport fails.
   */
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
