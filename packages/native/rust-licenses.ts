/**
 * Writes `RUST-THIRD-PARTY-LICENSES` with the licenses of the crates that the native
 * binaries link. `--check` fails when the file differs instead of writing it.
 *
 * The list contains the normal dependencies of `stillwind-napi` for every target, so
 * it can name crates that one platform binary does not link. Build dependencies and
 * procedural macros run only during compilation and are excluded.
 */

import { execFileSync } from "node:child_process";
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";

import {
  authorName,
  licenseTexts,
  renderThirdPartyLicenses,
  type ThirdPartyPackage,
} from "@stillwind-internal/tools-workspace/third-party-licenses";

interface CargoPackage {
  id: string;
  name: string;
  version: string;
  license: string | null;
  repository: string | null;
  authors: string[];
  source: string | null;
  manifest_path: string;
  targets: {
    kind: string[];
  }[];
}

interface CargoMetadata {
  packages: CargoPackage[];
  resolve: {
    nodes: {
      id: string;
      deps: {
        pkg: string;
        dep_kinds: {
          kind: string | null;
        }[];
      }[];
    }[];
  };
}

const root = import.meta.dirname;
const destination = path.join(root, "RUST-THIRD-PARTY-LICENSES");

const metadata = JSON.parse(
  execFileSync("cargo", ["metadata", "--format-version", "1", "--locked"], {
    cwd: path.resolve(root, "../.."),
    encoding: "utf8",
    maxBuffer: 256 * 1024 * 1024,
  }),
) as CargoMetadata;
const packages = new Map(metadata.packages.map((entry) => [entry.id, entry]));
const nodes = new Map(metadata.resolve.nodes.map((node) => [node.id, node]));

const binding = metadata.packages.find((entry) => entry.name === "stillwind-napi");
if (!binding) {
  throw new Error("cargo metadata does not contain stillwind-napi.");
}
const linked = new Set<string>();
const pending = [binding.id];
while (pending.length) {
  const id = pending.pop()!;
  if (linked.has(id)) {
    continue;
  }
  linked.add(id);
  for (const dependency of nodes.get(id)?.deps ?? []) {
    const crate = packages.get(dependency.pkg)!;
    const normal = dependency.dep_kinds.some((kind) => kind.kind === null);
    const macro = crate.targets.some((target) => target.kind.includes("proc-macro"));
    if (normal && !macro) {
      pending.push(dependency.pkg);
    }
  }
}

const crates: ThirdPartyPackage[] = [...linked]
  .map((id) => packages.get(id)!)
  // Workspace crates have no source and are covered by the Stillwind license.
  .filter((crate) => crate.source !== null)
  .map((crate) => ({
    name: crate.name,
    version: crate.version,
    license: crate.license ?? "unknown",
    repository: crate.repository ?? undefined,
    authors: crate.authors.map(authorName).filter(Boolean),
    texts: licenseTexts(path.dirname(crate.manifest_path)),
  }));

const output = renderThirdPartyLicenses(
  "The Stillwind native binaries include the following Rust crates.",
  crates,
);
if (process.argv.includes("--check")) {
  const current = await readFile(destination, "utf8").catch(() => undefined);
  if (current?.replaceAll("\r\n", "\n") !== output) {
    throw new Error(
      `${destination} is stale. Run vp run @stillwind/native#generate:rust-licenses.`,
    );
  }
} else {
  await writeFile(destination, output);
}
