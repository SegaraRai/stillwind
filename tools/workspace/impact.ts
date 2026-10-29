import { execFileSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import path from "node:path";

export interface WorkspacePackage {
  name: string;
  directory: string;
  dependencies: string[];
}

export interface WorkspaceImpact {
  full: boolean;
  reason?: string;
  changedPackages: string[];
  affectedPackages: string[];
}

/** Reads pnpm's package listing and resolves dependencies between workspace packages. */
export async function readWorkspacePackages(
  root: string,
  listing: readonly { name: string; path: string }[],
): Promise<WorkspacePackage[]> {
  const names = new Set(listing.map((entry) => entry.name));
  return Promise.all(
    listing.map(async (entry) => {
      const directory = path.relative(root, entry.path).split(path.sep).join("/") || ".";
      if (directory.startsWith("../") || path.isAbsolute(directory)) {
        throw new Error(`Workspace package is outside the repository: ${entry.path}`);
      }
      const manifest = JSON.parse(await readFile(path.join(entry.path, "package.json"), "utf8"));
      if (manifest.name !== entry.name) {
        throw new Error(`Workspace package name changed: ${entry.path}`);
      }
      const dependencies = Object.entries<string>({
        ...manifest.dependencies,
        ...manifest.devDependencies,
        ...manifest.peerDependencies,
        ...manifest.optionalDependencies,
      });
      for (const [name, specifier] of dependencies) {
        if (specifier.startsWith("workspace:") && !names.has(name)) {
          throw new Error(`Unknown workspace dependency: ${entry.name} -> ${name}`);
        }
      }
      return {
        name: entry.name,
        directory,
        dependencies: dependencies.map(([name]) => name).filter((name) => names.has(name)),
      };
    }),
  );
}

/**
 * Returns changed packages and their transitive consumers. Unowned paths select the full workspace.
 * Source prefixes assign generated or cross-language inputs to their consuming packages.
 */
export function workspaceImpact(
  packages: readonly WorkspacePackage[],
  files: readonly string[] | null,
  sourcePrefixes: Readonly<Record<string, readonly string[]>> = {},
): WorkspaceImpact {
  const all = packages.map((entry) => entry.name).sort();
  const full = (reason: string): WorkspaceImpact => ({
    full: true,
    reason,
    changedPackages: all,
    affectedPackages: all,
  });
  if (files === null) {
    return full("No complete comparison is available");
  }
  const owners = [...packages]
    .filter((entry) => entry.directory !== ".")
    .sort((left, right) => right.directory.length - left.directory.length);
  const changed = new Set<string>();
  for (const file of files) {
    const source = Object.entries(sourcePrefixes).find(([prefix]) => file.startsWith(prefix));
    const owner = owners.find((entry) => file.startsWith(`${entry.directory}/`));
    if (source) {
      for (const name of source[1]) {
        if (!all.includes(name)) {
          return full(`Unknown source consumer: ${name}`);
        }
        changed.add(name);
      }
    } else if (owner) {
      changed.add(owner.name);
    } else {
      return full(`Unclassified input: ${file}`);
    }
  }
  const affected = new Set(changed);
  let expanded = true;
  while (expanded) {
    expanded = false;
    for (const entry of packages) {
      if (!affected.has(entry.name) && entry.dependencies.some((name) => affected.has(name))) {
        affected.add(entry.name);
        expanded = true;
      }
    }
  }
  return {
    full: false,
    changedPackages: [...changed].sort(),
    affectedPackages: [...affected].sort(),
  };
}

/**
 * Compares the checked-out tree with the PR base or a push baseline.
 * A supplied baseline overrides the commit before a push, including an empty, unavailable baseline.
 * Returns null for manual runs, missing history, and incomplete or invalid event data.
 * Renames include both paths, and pushes include every commit in the push.
 */
export function changedFiles(
  root: string,
  eventName: string,
  event: { before?: string; after?: string; pull_request?: { base?: { sha?: string } } },
  pushBaseline?: string,
): string[] | null {
  const base =
    eventName === "pull_request" ? event.pull_request?.base?.sha : (pushBaseline ?? event.before);
  if (
    !["pull_request", "push"].includes(eventName) ||
    !base ||
    !/^[0-9a-f]{40,64}$/.test(base) ||
    /^0+$/.test(base)
  ) {
    return null;
  }
  const git = (args: string[]) =>
    execFileSync("git", args, { cwd: root, encoding: "utf8", maxBuffer: 16 * 1024 * 1024 });
  try {
    const head = git(["rev-parse", "HEAD"]).trim();
    if (eventName === "push" && event.after !== head) {
      return null;
    }
    git(["cat-file", "-e", `${base}^{commit}`]);
    return git(["diff", "--name-only", "--no-renames", "-z", base, head])
      .split("\0")
      .filter(Boolean);
  } catch {
    return null;
  }
}
