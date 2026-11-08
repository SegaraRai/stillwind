import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";

import { expect, onTestFinished, test } from "vite-plus/test";
import type { LoaderContext } from "webpack";

import nextCssLoader from "./css-loader.ts";
import type { NextLoaderOptions } from "./entries.ts";

for (const change of ["add", "remove before reading", "remove after reading"] as const) {
  test(`Next CSS follows route changes during compilation: ${change}`, async () => {
    const root = await mkdtemp(path.join(import.meta.dirname, ".test-route-change-"));
    onTestFinished(() => rm(root, { recursive: true, force: true }));
    const page = path.join(root, "app/page.jsx");
    const other = path.join(root, "app/other/page.jsx");
    const source = (utility: string) =>
      `import {style} from 'stillwind/react';export const props=style('${utility}');`;
    await mkdir(path.dirname(page), { recursive: true });
    await writeFile(
      path.join(root, "root.css"),
      "@utility red{color:red}@utility blue{color:blue}@stillwind-slot utilities;",
    );
    await writeFile(page, source("red"));
    if (change !== "add") {
      await mkdir(path.dirname(other), { recursive: true });
      await writeFile(other, source("blue"));
    }
    let changed = false;
    const output = await new Promise<string>((resolve, reject) => {
      const context = {
        rootContext: root,
        resourcePath: path.join(root, "stillwind-turbopack.css"),
        getOptions: (): NextLoaderOptions => ({
          compiler: { css: "root.css" },
          root,
          entries: [],
          mode: "development",
          extensions: ["jsx"],
        }),
        fs: {
          readFile(file: string, done: (error: unknown, contents?: Buffer) => void) {
            void (async () => {
              if (!changed && file.endsWith(".jsx")) {
                changed = true;
                if (change === "add") {
                  await mkdir(path.dirname(other), { recursive: true });
                  await writeFile(other, source("blue"));
                } else if (change === "remove after reading") {
                  const contents = await readFile(file);
                  await rm(other);
                  return contents;
                } else {
                  await rm(other);
                }
              }
              return readFile(file);
            })().then((contents) => done(null, contents), done);
          },
        },
        addDependency: () => {},
        addContextDependency: () => {},
        addMissingDependency: () => {},
        getResolve: () => () => expect.fail("This fixture has no imports to resolve"),
        emitWarning: (error: Error) => expect.fail(error.message),
        async: () => (error: unknown, css: string) => (error ? reject(error) : resolve(css)),
      } as unknown as LoaderContext<NextLoaderOptions>;
      nextCssLoader.call(context);
    });
    expect(output).toMatch(/color:\s*red/);
    if (change === "add") {
      expect(output).toMatch(/color:\s*blue/);
    } else {
      expect(output).not.toMatch(/color:\s*blue/);
    }
  });
}
