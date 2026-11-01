import childProcess from "node:child_process";
import { syncBuiltinESMExports } from "node:module";

/**
 * Runs a function while `child_process.spawn` hides the console windows of new processes.
 *
 * The TypeScript API starts its checker with `spawn` and has no option for `windowsHide`. Under
 * Electron, `Code.exe` is a Windows GUI program that has no console, so Windows opens a visible
 * console for `tsc.exe` unless the spawn call hides it.
 *
 * @param action - Synchronous function that starts processes. Asynchronous work after it returns
 * uses the original `spawn`.
 * @returns The function's return value. `spawn` is restored before the return or the throw.
 * @throws If the function throws.
 */
export function withHiddenSpawnWindows<T>(action: () => T): T {
  const { spawn } = childProcess;
  childProcess.spawn = ((command: string, args: readonly string[], options: object) =>
    spawn(command, args, {
      ...options,
      windowsHide: true,
    })) as typeof spawn;
  // ESM imports of `node:child_process` read a copy that only this call updates.
  syncBuiltinESMExports();
  try {
    return action();
  } finally {
    childProcess.spawn = spawn;
    syncBuiltinESMExports();
  }
}
