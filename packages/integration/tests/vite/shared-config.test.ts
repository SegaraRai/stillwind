import { mkdtemp, rm, writeFile } from "node:fs/promises";
import path from "node:path";

import { AnalysisWorkspace, resolveProjectOptions } from "@stillwind/host";
import { stillwind } from "@stillwind/integration/vite";
import { build } from "vite";
import { expect, test } from "vite-plus/test";

test("build and standalone analysis consume the same CSS and declaration naming settings", async () => {
  const root = await mkdtemp(path.join(import.meta.dirname, ".test-shared-config-"));
  let workspace: AnalysisWorkspace | undefined;
  try {
    await writeFile(
      path.join(root, "stillwind.config.ts"),
      'export default {projects:[{css:"root.css",files:["entry.ts"],classNamespace:"shared-project",prefix:"--project-"}]};',
    );
    await writeFile(
      path.join(root, "root.css"),
      "@utility red{color:red} @stillwind-slot utilities; @stillwind-slot keyframes;",
    );
    await writeFile(
      path.join(root, "entry.ts"),
      'import {style} from "stillwind/react";import {frames} from "./Tokens";export const props=style(frames(""));',
    );
    await writeFile(
      path.join(root, "Tokens.ts"),
      'import {style} from "stillwind/react";export const frames=style.keyframes({from:"red",to:"red"});',
    );
    workspace = await AnalysisWorkspace.open(path.join(root, "stillwind.config.ts"));
    const [analysis] = await workspace.lint();
    expect(analysis.result.status, JSON.stringify(analysis)).toBe("complete");
    const compiled = analysis.result.graph!.modules.find(
      (module) => module.moduleId === "entry.ts",
    )!;
    expect(compiled.classes.length).toBeGreaterThan(0);
    const result = await build({
      root,
      configFile: false,
      plugins: [
        stillwind({ ...resolveProjectOptions(workspace.projects[0].config, root), types: false }),
      ],
      build: {
        write: false,
        minify: false,
        cssMinify: false,
        lib: { entry: path.join(root, "entry.ts"), formats: ["es"] },
      },
    });
    const outputs = (Array.isArray(result) ? result : [result]).flatMap((result) => {
      if (!("output" in result)) {
        expect.fail("Expected build output");
      }
      return result.output;
    });
    const css = outputs
      .flatMap((output) =>
        output.type === "asset" && output.fileName.endsWith(".css") ? [String(output.source)] : [],
      )
      .join("\n");
    for (const generated of compiled.classes) {
      expect(css).toContain(`.${generated.className}`);
    }
    expect(compiled.symbols.length).toBeGreaterThan(0);
    for (const symbol of compiled.symbols) {
      expect(css).toContain(symbol.name);
    }
    expect(css).toMatch(/color:\s*red/);
  } finally {
    await workspace?.close();
    await rm(root, { recursive: true, force: true, maxRetries: 5 });
  }
});
