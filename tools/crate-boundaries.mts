import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

interface Dependency {
  name: string;
  kind: "dev" | "build" | null;
}

interface Package {
  id: string;
  name: string;
  dependencies: Dependency[];
}

const root = fileURLToPath(new URL("../", import.meta.url));
const metadata = JSON.parse(
  execFileSync("cargo", ["metadata", "--no-deps", "--format-version", "1"], {
    cwd: root,
    encoding: "utf8",
  }),
) as { packages: Package[]; workspace_members: string[] };

// Production edges form the compiler's responsibility boundaries. Tests may
// compose a frontend and backend without making that a production dependency.
const allowed: Record<string, string[]> = {
  source: [],
  diagnostics: ["source"],
  syntax: ["source"],
  css: ["source"],
  semantic: ["diagnostics", "source"],
  ir: ["diagnostics", "source", "semantic"],
  "js-oxc": ["diagnostics", "source", "semantic", "ir"],
  "design-system": ["diagnostics", "source", "css", "syntax"],
  compiler: ["diagnostics", "source", "syntax", "css", "semantic", "ir", "design-system"],
  project: ["diagnostics", "source", "semantic", "ir", "compiler", "design-system"],
  napi: [
    "diagnostics",
    "source",
    "css",
    "semantic",
    "ir",
    "js-oxc",
    "design-system",
    "compiler",
    "project",
  ],
  cli: ["diagnostics", "source", "ir", "js-oxc", "design-system", "compiler"],
};

const workspace = metadata.packages.filter((pkg) => metadata.workspace_members.includes(pkg.id));
assert.deepEqual(
  workspace.map((pkg) => pkg.name).sort(),
  Object.keys(allowed)
    .map((name) => `stillwind-${name}`)
    .sort(),
  "Workspace crates must match the agreed architecture; do not add compatibility facades.",
);

for (const pkg of workspace) {
  const name = pkg.name.slice("stillwind-".length);
  for (const dependency of pkg.dependencies.filter((dep) => dep.kind !== "dev")) {
    if (dependency.name.startsWith("stillwind-")) {
      const target = dependency.name.slice("stillwind-".length);
      assert(allowed[name]?.includes(target), `${pkg.name} must not depend on ${dependency.name}`);
    }
    if (dependency.name === "oxc") {
      assert.equal(name, "js-oxc", "Only the Oxc frontend owns Oxc AST dependencies.");
    }
  }
}

console.log(`Validated responsibility boundaries for ${workspace.length} Rust crates.`);
