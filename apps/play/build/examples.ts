/**
 * Checks or fixes the preset workspaces in `examples/`. Each preset must have no Stillwind lint
 * findings and must keep Stillwind's utility order and Prettier's default formatting, so the
 * playground's Fix and Format buttons leave it unchanged.
 *
 * Usage: `node build/examples.ts check` or `node build/examples.ts fix`.
 */

import { spawnSync } from "node:child_process";
import { readdirSync } from "node:fs";
import path from "node:path";

const root = path.resolve(import.meta.dirname, "..");
const mode = process.argv[2];
if (mode !== "check" && mode !== "fix") {
  throw new Error("Pass check or fix");
}

function run(command: string, args: string[]): void {
  // The local binaries are shell scripts on Windows. The arguments contain no spaces.
  const result = spawnSync([command, ...args].join(" "), {
    cwd: root,
    stdio: "inherit",
    shell: true,
  });
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

for (const preset of readdirSync(path.join(root, "examples"))) {
  const config = `examples/${preset}/stillwind.config.ts`;
  if (mode === "fix") {
    run("stillwind", ["lint", "--fix", "-c", config]);
    run("stillwind", ["fmt", "-c", config]);
  } else {
    run("stillwind", ["lint", "--max-warnings=0", "-c", config]);
    run("stillwind", ["fmt", "--check", "-c", config]);
  }
}
run("prettier", [mode === "fix" ? "--write" : "--check", "examples"]);
