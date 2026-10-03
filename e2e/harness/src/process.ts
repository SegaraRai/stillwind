import { spawn, type ChildProcess } from "node:child_process";
import { setTimeout as delay } from "node:timers/promises";

/**
 * Stops a runner and its descendants, including framework CLI processes on Windows.
 * For a running child, waits for its exit and the closure of its output pipes.
 * Rejects if taskkill fails before the child reports its exit.
 */
export async function stopProcess(child: ChildProcess): Promise<void> {
  if (!child.pid || child.exitCode !== null || child.signalCode !== null) {
    return;
  }
  const closed = new Promise<void>((resolve) => {
    child.once("close", () => resolve());
  });
  if (process.platform === "win32") {
    await new Promise<void>((resolve, reject) => {
      const killer = spawn("taskkill", ["/pid", String(child.pid), "/t", "/f"], {
        windowsHide: true,
        stdio: "ignore",
      });
      killer.once("error", reject);
      killer.once("close", (code) => {
        if (code === 0 || child.exitCode !== null || child.signalCode !== null) {
          resolve();
        } else {
          reject(new Error(`taskkill exited with code ${code}`));
        }
      });
    });
  } else {
    try {
      process.kill(-child.pid, "SIGTERM");
    } catch {
      child.kill("SIGTERM");
    }
    await delay(300);
    try {
      process.kill(-child.pid, "SIGKILL");
    } catch {
      /* The group may already have exited. */
    }
  }
  await closed;
}

/**
 * Runs Node with bounded output and a deadline; callers must stop long-lived servers.
 */
export function startNode(
  args: string[],
  cwd: string,
  env: NodeJS.ProcessEnv = {},
  timeout = 180_000,
) {
  const childEnv = { ...process.env, ...env };
  // Frameworks use VITEST to disable server middleware and alter dependency resolution.
  for (const key of Object.keys(childEnv)) {
    if (key === "VITEST" || key.startsWith("VITEST_") || key === "VP_RESOLVING_CONFIG_METADATA") {
      delete childEnv[key];
    }
  }
  const child = spawn(process.execPath, args, {
    cwd,
    env: childEnv,
    windowsHide: true,
    detached: process.platform !== "win32",
    stdio: ["ignore", "pipe", "pipe"],
  });
  let output = "";
  for (const stream of [child.stdout, child.stderr]) {
    stream?.on("data", (data: Buffer) => {
      output = (output + data.toString()).slice(-200_000);
    });
  }
  const {
    promise: done,
    resolve: resolveDone,
    reject: rejectDone,
  } = Promise.withResolvers<number | null>();
  child.once("error", rejectDone);
  child.once("close", resolveDone);
  void done.catch(() => {});
  const timer = setTimeout(() => {
    void stopProcess(child).catch(rejectDone);
  }, timeout);
  timer.unref();
  void done.finally(() => clearTimeout(timer)).catch(() => {});
  return {
    child,
    done,
    logs: () => output,
    stop: () => stopProcess(child),
  };
}

/**
 * Waits for a successful HTTP response and fails promptly when the server exits.
 */
export async function waitForServer(
  url: string,
  processHandle: ReturnType<typeof startNode>,
  timeout = 90_000,
): Promise<void> {
  const deadline = Date.now() + timeout;
  let lastResponse = "No response";
  while (Date.now() < deadline) {
    if (processHandle.child.exitCode !== null || processHandle.child.signalCode !== null) {
      throw new Error(processHandle.logs());
    }
    try {
      const response = await fetch(url, {
        signal: AbortSignal.timeout(2000),
      });
      if (response.ok) {
        return;
      }
      lastResponse = `HTTP ${response.status}: ${(await response.text()).slice(0, 1000)}`;
    } catch (error) {
      lastResponse = String(error);
    }
    await delay(100);
  }
  throw new Error(
    `Server did not become ready: ${url}\n${lastResponse}\n${processHandle.logs().slice(-10_000)}`,
  );
}
