// Copies the version of `@stillwind/native`, which ships the Rust crates, to the Rust workspace.
// The compiler reads the version of its license comment from Cargo, and `pnpm version -r` updates
// only package manifests.

import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";

const root = path.resolve(import.meta.dirname, "../..");

function read(file: string): string {
  return readFileSync(path.join(root, file), "utf8");
}

function write(file: string, text: string): void {
  writeFileSync(path.join(root, file), text);
}

const { version } = JSON.parse(read("packages/native/package.json")) as { version: string };
write(
  "Cargo.toml",
  read("Cargo.toml").replace(
    /^(\[workspace\.package\][^[]*?^version = ")[^"]+/m,
    (_, prefix: string) => prefix + version,
  ),
);
write(
  "Cargo.lock",
  read("Cargo.lock").replace(
    /^(name = "stillwind-[^"]+"\r?\nversion = ")[^"]+/gm,
    (_, prefix: string) => prefix + version,
  ),
);
