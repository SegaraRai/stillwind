import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { dirname, resolve } from "node:path";
import { pathToFileURL } from "node:url";

import { loadCssGraph } from "@stillwind/host";
import { NativeProjectSession, type NativePreparedSource } from "@stillwind/native";

import themeCss from "../styles/theme.css?raw";

const require = createRequire(import.meta.url);
const theme = `
@layer theme, utilities;
@import "stillwind/theme.css" layer(theme);
@import "stillwind/builtins.css";
@layer utilities { @stillwind-slot utilities; }
${themeCss}
`;

/** Compiles the website specimens against Stillwind's built-in utilities at build time. */
export async function createExamples() {
  const { loader } = await loadCssGraph(
    resolve("website-examples.css"),
    {
      utilityOrder: "tailwind",
      lint: {
        unknownUtility: "deny",
        noArbitraryColors: "deny",
        noArbitraryBreakpoints: "deny",
        preferCanonicalUtilities: {
          severity: "warn",
        },
      },
    },
    {
      resolve: async (specifier, importer) =>
        specifier.startsWith(".")
          ? resolve(dirname(importer), specifier)
          : require.resolve(specifier),
      read: (id) => readFile(id, "utf8"),
      documentUri: (id) => pathToFileURL(id).href,
      sourceMapSource: (id) => id,
      watch() {},
    },
    theme,
  );
  const query = loader.editorQuery();
  const source = (utilities: string): NativePreparedSource => ({
    kind: "direct",
    moduleId: "Button.tsx",
    document: {
      uri: "file:///examples/Button.tsx",
      format: "jsx",
      code: `import { style } from "stillwind/react";\nexport const button = style("${utilities}");`,
    },
    syntax: {
      language: "tsx",
      moduleKind: "module",
    },
  });
  const diagnose = (utilities: string) => {
    const prepared = source(utilities);
    let offset = prepared.document.code.indexOf(utilities);
    const ranges = utilities.split(" ").map((utility) => {
      const start = offset;
      offset += utility.length + 1;
      return { start, end: start + utility.length };
    });
    return new NativeProjectSession(loader, false)
      .diagnose([prepared])[0]!
      .reports.map((report) => {
        const location = report.primary.location;
        const utilityIndex = ranges.findIndex(
          (range) => range.start <= location.startUtf16 && range.end >= location.endUtf16,
        );
        if (utilityIndex < 0) {
          throw new Error("A website diagnostic must identify one displayed utility.");
        }
        return { ...report, utilityIndex };
      });
  };
  const unchecked = "py-2 text-action-foreground px-4 bg-actoin rounded-control";
  const restricted = "py-2 text-action-foreground min-[713px]:px-6 bg-[#d9e2f0] rounded-control";
  const unordered = "py-2 text-action-foreground px-[16px] bg-action rounded-control";
  const canonicalized = unordered
    .split(" ")
    .map((candidate) => query.canonicalCandidate(candidate) ?? candidate)
    .join(" ");
  const ordered = canonicalized
    .split(" ")
    .sort((left, right) => query.compareCandidates(left, right) ?? 0)
    .join(" ");
  const compiled = ordered;
  const session = new NativeProjectSession(loader, false);
  const output = session.compile([source(compiled)]).modules[0]!;
  if (!output.code || output.diagnostics.length) {
    throw new Error("The website's compilation specimen must compile successfully.");
  }

  const cssClassName = /\.(sw_\w+)/.exec(output.css)?.[1];
  if (!cssClassName) {
    throw new Error("Cannot capture output CSS class name.");
  }

  return {
    unchecked,
    unknown: diagnose(unchecked).find((report) => report.code === "unknown-utility")!,
    restricted,
    restrictions: diagnose(restricted).filter((report) =>
      ["no-arbitrary-colors", "no-arbitrary-breakpoints"].includes(report.code),
    ),
    unordered,
    canonical: diagnose(unordered).filter((report) => report.code === "prefer-canonical-utilities"),
    canonicalized,
    ordered,
    compiled,
    css: query.formatCss(output.css),
    cssClassName,
    compiledCode: output.code,
  };
}
