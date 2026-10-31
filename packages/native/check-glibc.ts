import { execFileSync } from "node:child_process";

/** The newest glibc version that a published Linux binding may require. */
export const glibcLimit = "2.17";

function compareVersions(left: string, right: string): number {
  const a = left.split(".").map(Number);
  const b = right.split(".").map(Number);
  for (let index = 0; index < Math.max(a.length, b.length); index++) {
    const difference = (a[index] ?? 0) - (b[index] ?? 0);
    if (difference) {
      return difference;
    }
  }
  return 0;
}

/**
 * Returns the newest glibc version named in `objdump -T` output.
 * Throws when the output names no glibc version, because a failed inspection must not pass.
 */
export function newestGlibc(symbols: string): string {
  const versions = [...symbols.matchAll(/\bGLIBC_(\d+(?:\.\d+)+)\b/g)].map((match) => match[1]!);
  if (!versions.length) {
    throw new Error("The binding names no glibc symbol versions");
  }
  return versions.reduce((newest, version) =>
    compareVersions(version, newest) > 0 ? version : newest,
  );
}

/**
 * Throws when a binding's symbols require a glibc version newer than glibcLimit.
 */
export function checkGlibc(symbols: string): void {
  const newest = newestGlibc(symbols);
  if (compareVersions(newest, glibcLimit) > 0) {
    throw new Error(`The binding requires glibc ${newest}, and the limit is ${glibcLimit}`);
  }
  console.log(`The binding requires glibc ${newest}`);
}

if (import.meta.main) {
  checkGlibc(
    execFileSync("objdump", ["-T", process.argv[2]!], {
      encoding: "utf8",
      maxBuffer: 64 * 1024 * 1024,
    }),
  );
}
