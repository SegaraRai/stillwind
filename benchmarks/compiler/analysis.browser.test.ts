import { __napiBindingTarget } from "@stillwind/native";
import { expect, inject, test } from "vite-plus/test";

import { BrowserAnalysisWorkspace } from "../../packages/analysis/src/browser.ts";

declare module "vite-plus/test" {
  interface ProvidedContext {
    /** Virtual package metadata and declarations supplied by the browser benchmark config. */
    analysisBrowserFiles: [string, string][];
  }
}

test.for([false, true])(
  "browser analysis retains definitions across source edits (type facts: %s)",
  async (typed, context) => {
    const uri = "file:///workspace/entry.ts";
    expect(crossOriginIsolated).toBe(true);
    expect(__napiBindingTarget).toBe("wasm32-wasi");
    const source = [
      'import { style } from "stillwind/react";',
      ...(typed
        ? [
            'const first = style.ref("first");',
            'const second = style.ref("second");',
            "declare const active: boolean;",
            "const selected = active ? first : second;",
            'export const value = style(selected, "p-[16px] flex");',
          ]
        : ['export const value = style("p-[16px] flex");']),
    ].join("\n");
    const workspace = new BrowserAnalysisWorkspace({
      config: "file:///workspace/stillwind.config.ts",
      projects: [
        {
          root: "/workspace",
          files: ["*.ts"],
          css: "theme.css",
          ...(typed ? {} : { typeFacts: false as const }),
        },
      ],
      files: new Map<string, string>([
        ...(typed ? inject("analysisBrowserFiles") : []),
        ["/workspace/theme.css", inject("canonicalBrowserCss")],
        ["/workspace/entry.ts", source],
      ]),
    });
    /** Each sample records elapsed milliseconds for one operation. */
    const measurements: { operation: string; samples: number[] }[] = [];
    try {
      const start = performance.now();
      expect((await workspace.analyze(uri))[0].result.status).toBe("complete");
      measurements.push({ operation: "cold-lint", samples: [performance.now() - start] });
      const edits: number[] = [];
      const sorts: number[] = [];
      const builds: number[] = [];
      for (let version = 1; version <= 12; version++) {
        const changed = source.replace("value", `value${version}`);
        const editStart = performance.now();
        workspace.update(uri, changed, version);
        const [analysis] = await workspace.analyze(uri);
        edits.push(performance.now() - editStart);
        expect(analysis.result.status).toBe("complete");
        const sortStart = performance.now();
        expect((await workspace.query(uri, { kind: "sort" }))[0].result.revision).toBeTruthy();
        sorts.push(performance.now() - sortStart);
        const buildStart = performance.now();
        expect((await workspace.compile(uri, workspace.revision)).graph.generation).toBe(
          "complete",
        );
        builds.push(performance.now() - buildStart);
      }
      measurements.push(
        { operation: "edit-to-diagnostics", samples: edits },
        { operation: "sort-after-diagnostics", samples: sorts },
        { operation: "build-after-diagnostics", samples: builds },
      );
      context.task.meta.analysisBrowser = {
        userAgent: navigator.userAgent,
        typeFacts: typed,
        measurements,
      };
    } finally {
      await workspace.close();
    }
  },
);
