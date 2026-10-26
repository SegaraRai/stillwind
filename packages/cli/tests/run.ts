import { spawn } from "node:child_process";
import path from "node:path";

const main = path.resolve(import.meta.dirname, "../dist/main.js");

/**
 * Runs the built CLI in a child process and collects its output.
 *
 * @param cwd - Working directory of the CLI process.
 * @param args - Command-line arguments after the CLI entry point.
 * @param env - Variables added to the current process environment.
 * @returns The exit code, which is null when a signal ends the process, and the complete stdout and stderr text.
 * @throws If the process cannot start.
 */
export async function run(
  cwd: string,
  args: readonly string[],
  env: NodeJS.ProcessEnv = {},
): Promise<{
  code: number | null;
  stdout: string;
  stderr: string;
}> {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, [main, ...args], {
      cwd,
      windowsHide: true,
      env: {
        ...process.env,
        ...env,
      },
    });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (data: Buffer) => {
      stdout += data.toString();
    });
    child.stderr.on("data", (data: Buffer) => {
      stderr += data.toString();
    });
    child.on("error", reject);
    child.on("close", (code) => {
      resolve({
        code,
        stdout,
        stderr,
      });
    });
  });
}
