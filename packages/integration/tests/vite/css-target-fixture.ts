import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
import { runInNewContext } from "node:vm";

import type { BaseConfig } from "@stillwind/config";
import { stillwind } from "@stillwind/integration/vite";
import { rolldown } from "rolldown";
import { build, type InlineConfig } from "vite";

export interface CssTargetCase {
  name: string;
  expected: "modern" | "modern-split" | "safari" | "safari-modern" | "unspecified" | "legacy";
  build?: InlineConfig["build"];
  environments?: InlineConfig["environments"];
  projectTarget?: BaseConfig["target"];
  inlineTarget?: BaseConfig["target"];
  emission?: "single" | "split";
}

export const cssTargetCases: CssTargetCase[] = [
  {
    name: "cssTarget overrides the JavaScript target",
    expected: "modern",
    build: { target: "chrome61", cssTarget: "chrome120" },
  },
  {
    name: "omitted cssTarget inherits the JavaScript target",
    expected: "modern",
    build: { target: "chrome120" },
  },
  {
    name: "Safari before 17 retains currentcolor fallbacks",
    expected: "safari",
    build: { cssTarget: "safari16.4" },
  },
  {
    name: "one older browser keeps the generated fallback for all targets",
    expected: "safari",
    build: { cssTarget: ["chrome120", "safari16.4"] },
  },
  {
    name: "Safari and iOS 17 remove generated fallbacks and retain the Safari guard",
    expected: "safari-modern",
    build: { cssTarget: ["safari17", "ios17"] },
  },
  {
    name: "an empty project target overrides Vite",
    expected: "unspecified",
    projectTarget: [],
    build: { cssTarget: "chrome120" },
  },
  {
    name: "an empty inline target overrides the project",
    expected: "unspecified",
    projectTarget: ["chrome120"],
    inlineTarget: [],
    build: { cssTarget: "chrome120" },
  },
  {
    name: "a project target overrides Vite",
    expected: "safari",
    projectTarget: ["safari16.4"],
    build: { cssTarget: "chrome120" },
  },
  {
    name: "an inline target overrides the project and Vite",
    expected: "modern",
    projectTarget: [],
    inlineTarget: "chrome120",
    build: { cssTarget: "chrome61" },
  },
  {
    name: "JavaScript language levels retain unspecified targets",
    expected: "unspecified",
    build: { cssTarget: "es2022" },
  },
  {
    name: "old browsers prevent modern minifier color notation",
    expected: "legacy",
    build: { cssTarget: "chrome61" },
  },
  {
    name: "client CSS targets take precedence over SSR JavaScript targets",
    expected: "modern",
    build: { target: "node22" },
    environments: {
      client: { build: { cssTarget: "chrome120" } },
      ssr: { build: { target: "node22" } },
    },
  },
  {
    name: "split CSS assets use the same minifier targets",
    expected: "modern-split",
    emission: "split",
    build: { cssTarget: "chrome120" },
  },
];

export const cssTargetFixtureRoot = fileURLToPath(
  new URL("./fixtures/css-targets/", import.meta.url),
);
export const cssTargetExpectedRoot = path.join(cssTargetFixtureRoot, "expected");

/**
 * Builds document CSS and an explicit CSS value from the same fixture sources.
 * @param testCase - Vite and Stillwind target overrides, plus optional emission and minification settings.
 * @returns The emitted CSS, evaluated CSS value, and output assets. Temporary files are removed.
 * @throws If fixture IO, Vite compilation, or evaluation fails, or the expected outputs are missing.
 */
export async function buildCssTargetFixture(testCase: CssTargetCase) {
  const root = await mkdtemp(path.join(import.meta.dirname, ".test-css-target-"));
  try {
    for (const file of ["root.css", "main.ts", "automatic.ts", "icon.svg"]) {
      await writeFile(path.join(root, file), await readFile(path.join(cssTargetFixtureRoot, file)));
    }
    await writeFile(
      path.join(root, "index.html"),
      '<script type="module" src="/main.ts"></script>',
    );
    await writeFile(
      path.join(root, "stillwind.config.json"),
      JSON.stringify({
        projects: [{ css: "root.css", files: ["*.ts"], target: testCase.projectTarget }],
      }),
    );
    const result = await build({
      root,
      configFile: false,
      logLevel: "silent",
      plugins: [stillwind({ target: testCase.inlineTarget, emission: testCase.emission })],
      environments: testCase.environments,
      build: {
        write: false,
        minify: false,
        cssMinify: true,
        assetsInlineLimit: 0,
        sourcemap: true,
        modulePreload: false,
        ...testCase.build,
      },
    });
    if (Array.isArray(result) || !("output" in result)) {
      throw new Error("Expected one completed Vite build");
    }
    const entry = result.output.find((item) => item.type === "chunk" && item.isEntry);
    if (entry?.type !== "chunk") {
      throw new Error("Expected the fixture entry chunk");
    }
    const values: { css?: string } = {};
    const fixture = "\0css-target-fixture";
    const bundle = await rolldown({
      input: fixture,
      transform: {
        define: {
          "import.meta.url": JSON.stringify("https://fixture.example/main.js"),
        },
      },
      plugins: [
        {
          name: "css-target-fixture",
          resolveId(id) {
            if (id === fixture) {
              return id;
            }
            return null;
          },
          load(id) {
            if (id === fixture) {
              return entry.code;
            }
            return null;
          },
        },
      ],
    });
    try {
      const script = await bundle.generate({
        format: "cjs",
        codeSplitting: false,
      });
      const chunk = script.output.find((output) => output.type === "chunk" && output.isEntry);
      if (chunk?.type !== "chunk") {
        throw new Error("Expected the evaluated fixture entry chunk");
      }
      runInNewContext(chunk.code, {
        globalThis: values,
        URL,
      });
    } finally {
      await bundle.close();
    }
    if (typeof values.css !== "string") {
      throw new Error("Expected the fixture CSS value");
    }
    const document = result.output
      .filter((item) => item.type === "asset")
      .filter((item) => item.fileName.endsWith(".css"))
      .map((item) => String(item.source).replace(/\n\/\*# sourceMappingURL=.*?\*\/$/, ""))
      .join("");
    return {
      document,
      fragment: values.css,
      output: result.output,
    };
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
  await mkdir(cssTargetExpectedRoot, { recursive: true });
  const written = new Set<string>();
  for (const testCase of cssTargetCases) {
    if (written.has(testCase.expected)) {
      continue;
    }
    const result = await buildCssTargetFixture(testCase);
    for (const kind of ["document", "fragment"] as const) {
      await writeFile(
        path.join(cssTargetExpectedRoot, `${testCase.expected}.${kind}.css`),
        result[kind] + "\n",
      );
    }
    written.add(testCase.expected);
  }
}
