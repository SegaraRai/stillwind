import { mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import path from "node:path";

import { build } from "vite";
import { expect, test } from "vite-plus/test";

import { compilerPlugin } from "../../src/vite/plugin.ts";
import { createViteProject } from "../../src/vite/project.ts";
import { packageRoot, redStyleSource } from "./vite-server-fixture.ts";

test("editing only root.css updates the CSS value exported by unchanged JavaScript", async () => {
  const root = await mkdtemp(path.join(packageRoot, ".test-css-value-watch-"));
  const cssFile = path.join(root, "root.css");
  await writeFile(cssFile, "@utility red{color:red} @stillwind-slot utilities;");
  await writeFile(path.join(root, "index.html"), '<script type="module" src="/main.ts"></script>');
  await writeFile(
    path.join(root, "main.ts"),
    `
import css from 'stillwind/module-css';
${redStyleSource}
globalThis.css = css;
`,
  );
  const project = createViteProject({ css: "root.css", types: false });
  const watcher = await build({
    configFile: false,
    root,
    logLevel: "silent",
    plugins: [compilerPlugin(project)],
    build: { watch: {}, minify: false, cssMinify: false, modulePreload: false },
  });
  if (Array.isArray(watcher) || !("on" in watcher)) {
    expect.fail("Expected a build watcher");
  }
  let completedBuilds = 0;
  let buildError: Error | undefined;
  watcher.on("event", (event) => {
    if (event.code === "END") {
      completedBuilds++;
    }
    if (event.code === "ERROR") {
      buildError = event.error;
    }
  });
  async function waitForBuild(count: number) {
    await expect
      .poll(() => {
        if (buildError) {
          throw buildError;
        }
        return completedBuilds;
      })
      .toBeGreaterThanOrEqual(count);
  }
  try {
    await waitForBuild(1);
    expect(await readBuiltJavaScript(root)).toMatch(/color:\s*red/);

    await writeFile(cssFile, "@utility red{color:blue} @stillwind-slot utilities;");
    await waitForBuild(2);

    expect(await readBuiltJavaScript(root)).toMatch(/color:\s*blue/);
    expect(buildError).toBeUndefined();
  } finally {
    await watcher.close();
    await rm(root, { recursive: true, force: true });
  }
});

async function readBuiltJavaScript(root: string) {
  const assetsDirectory = path.join(root, "dist/assets");
  const javascriptFiles = (await readdir(assetsDirectory)).filter((file) => file.endsWith(".js"));
  const contents = await Promise.all(
    javascriptFiles.map((file) => readFile(path.join(assetsDirectory, file), "utf8")),
  );
  return contents.join("\n");
}
