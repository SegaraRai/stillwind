import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";

import type { Loader } from "astro/loaders";

import { documentationCompiler } from "../reference/compiler";
import type { UtilityCatalog } from "./search";

let catalog: Promise<UtilityCatalog> | undefined;
export function loadUtilityCatalog() {
  return (catalog ??= readCatalog());
}

async function readCatalog(): Promise<UtilityCatalog> {
  const compiler = await documentationCompiler();
  const labels = new Map(compiler.query.completeCandidate("").map((entry) => [entry.label, entry]));
  const families = [
    "p",
    "px",
    "py",
    "pt",
    "pb",
    "ps",
    "pe",
    "m",
    "mx",
    "my",
    "mt",
    "mb",
    "ms",
    "me",
    "gap",
    "gap-x",
    "gap-y",
    "w",
    "h",
    "size",
    "min-w",
    "max-w",
    "min-h",
    "max-h",
    "top",
    "left",
    "right",
    "bottom",
    "inset",
    "space-x",
    "space-y",
  ];
  const samples = [
    "0",
    "0.5",
    "1",
    "1.5",
    "2",
    "2.5",
    "3",
    "3.5",
    "4",
    "5",
    "6",
    "8",
    "10",
    "12",
    "16",
    "20",
    "24",
    "32",
    "48",
    "64",
    "96",
  ];
  for (const family of families) {
    for (const value of samples) {
      const label = `${family}-${value}`;
      if (!labels.has(label)) {
        labels.set(label, { label, marker: false });
      }
    }
  }
  const records = [...labels.values()]
    .filter(({ label }) => !label.endsWith(":"))
    .sort((a, b) => a.label.localeCompare(b.label, "en"))
    .map(({ label, marker }) => {
      const css = compiler.query.previewCandidate(label);
      return {
        name: label,
        marker,
        css: css
          ? compiler.query.formatCss(css, {
              themeValues: true,
              pixelEquivalents: true,
              colorEquivalents: false,
            })
          : "",
      };
    })
    .filter(({ css, marker }) => css || marker);
  const require = createRequire(import.meta.url);
  const { version } = JSON.parse(
    await readFile(require.resolve("stillwind/package.json"), "utf8"),
  ) as { version: string };
  return {
    version,
    records,
    revision: createHash("sha256").update(JSON.stringify(records)).digest("hex"),
  };
}

export function utilityCatalogLoader(): Loader {
  return {
    name: "stillwind-utilities",
    async load(context) {
      const catalog = await loadUtilityCatalog();
      const data = await context.parseData({ id: "default", data: { ...catalog } });
      context.store.set({ id: "default", data, digest: context.generateDigest(data) });
    },
  };
}

export function utilityCatalogMarkdown(catalog: UtilityCatalog) {
  return `## Built-in utility catalog\n\nStillwind ${catalog.version}. Default design system. Revision: \`${catalog.revision}\`.\n\nRule previews omit document-level theme and support declarations. Numeric and arbitrary families accept values beyond this finite catalog.\n\n${catalog.records.map(({ name, css, marker }) => `### \`${name}\`\n\n${marker ? "Adds a marker class.\n\n" : ""}${css ? `\`\`\`css\n${css.trim()}\n\`\`\`\n` : "Emits no utility declarations.\n"}`).join("\n")}`;
}
