import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { parseArgs } from "node:util";

import { compile } from "tailwindcss";
import tailwind from "tailwindcss/package.json" with { type: "json" };

import { extractTailwindBanner, rustString } from "./banner-extraction.ts";

const { version } = tailwind;

const {
  values: { check },
} = parseArgs({
  options: {
    check: {
      type: "boolean",
      default: false,
    },
  },
});

const compiler = await compile("@tailwind utilities;");
const banner = extractTailwindBanner(compiler.build(["block"]), version);
const rust = `//! Generated from tailwindcss ${version} by tools/tailwind/banner.ts.

pub(super) const TAILWIND_BANNER: &str =
    ${rustString(banner)};
`;
const target = resolve(import.meta.dirname, "../../crates/compiler/src/banner/tailwind.rs");

const uptodate =
  existsSync(target) && readFileSync(target, "utf8").replaceAll("\r\n", "\n") === rust;
if (!uptodate) {
  if (check) {
    throw new Error("Generated Tailwind banner is stale");
  }

  writeFileSync(target, rust);
  console.log(`Generated Tailwind ${version} banner`);
  process.exit(0);
}

if (check) {
  console.log(`Verified Tailwind ${version} banner`);
} else {
  console.log("Already up-to-date");
}
