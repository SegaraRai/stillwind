import { lintPresets } from "@stillwind/config";
import { expect, test } from "vite-plus/test";

import { BrowserAnalysisWorkspace } from "./browser.ts";

test("browser workspace captures configuration and files across inventory revisions", async () => {
  const id = "/workspace/entry.ts";
  const uri = `file://${id}`;
  const code = 'import {style} from "stillwind/react"; export const card = style("block");';
  const project = {
    root: "/workspace",
    files: ["*.ts"],
    css: "theme.css",
    typeFacts: false as const,
  };
  const projects = [project];
  const files = new Map([
    [id, code],
    ["/workspace/theme.css", "@utility block { display:block; } @stillwind-slot utilities;"],
  ]);
  const options = {
    config: "file:///workspace/stillwind.config.ts",
    projects,
    files,
  };
  const workspace = new BrowserAnalysisWorkspace(options);
  try {
    const first = await workspace.compile(uri, workspace.revision);
    options.config = "changed";
    project.css = "missing.css";
    project.files[0] = "*.vue";
    projects.length = 0;
    files.clear();
    expect(workspace.files().get(id)).toBe(code);
    const cached = await workspace.compile(uri, workspace.revision);
    expect(cached.css).toBe(first.css);
    workspace.update(uri, code.replace("card", "updated"), 1);
    const updated = await workspace.compile(uri, workspace.revision);
    expect(updated.css).toContain("display:block");
    expect(updated.graph.generation).toBe("complete");
  } finally {
    await workspace.close();
  }
});

test("browser builds report a blocked dependency graph after a cached producer was analyzed", async () => {
  const workspace = new BrowserAnalysisWorkspace({
    config: "file:///workspace/stillwind.config.ts",
    projects: [
      {
        root: "/workspace",
        files: ["*.ts"],
        css: "theme.css",
        typeFacts: false,
      },
    ],
    files: new Map([
      [
        "/workspace/styles.ts",
        'import {style} from "stillwind/react"; export const card = style("block");',
      ],
      [
        "/workspace/entry.ts",
        'import {style} from "stillwind/react"; import {card} from "./styles"; export const value = style(card, style("block"));',
      ],
      ["/workspace/theme.css", "@utility block { display: block; } @stillwind-slot utilities;"],
    ]),
  });
  try {
    await workspace.query("file:///workspace/styles.ts", {
      kind: "hover",
      offset: 60,
    });
    const result = await workspace.compile("file:///workspace/entry.ts", workspace.revision);
    expect(result.graph.generation).toBe("blocked");
    expect(result.graph.batches.flatMap((batch) => batch.reports)).not.toHaveLength(0);
    expect(result.css).toBe("");
    expect(result.cssValues).toEqual([]);
    const recovered = await workspace.compile("file:///workspace/styles.ts", workspace.revision);
    expect(recovered.graph.generation).toBe("complete");
    expect(recovered.css).toContain("display: block");
    expect(recovered.cssValues.map(([id]) => id)).toEqual(["/workspace/styles.ts"]);
  } finally {
    await workspace.close();
  }
});

test("browser editor requests keep project policies separate and discard superseded buffers", async () => {
  const uri = "file:///workspace/entry.tsx";
  const code =
    'import {style} from "stillwind/react"; export const card=style("p-[16px] flex block");';
  const css =
    "@theme { --spacing:.25rem; } @utility p-* { padding:--value(--spacing(),[length]); } @utility block { display:block; } @utility flex { display:flex; } @stillwind-slot utilities;";
  const workspace = new BrowserAnalysisWorkspace({
    config: "file:///workspace/stillwind.config.ts",
    projects: [
      {
        root: "/workspace",
        files: ["*.tsx"],
        css: "theme.css",
        typeFacts: false,
      },
      {
        root: "/workspace",
        files: ["*.tsx"],
        css: "theme.css",
        typeFacts: false,
        lint: {
          extends: [lintPresets.recommended],
          rules: { "prefer-canonical-utilities": "off" },
        },
      },
    ],
    files: new Map([
      ["/workspace/entry.tsx", code],
      ["/workspace/theme.css", css],
    ]),
  });
  try {
    const fixes = await workspace.query(uri, { kind: "fixes" });
    expect(fixes.map((entry) => entry.result.edits?.length)).toEqual([1, 0]);
    const sorting = await workspace.query(uri, { kind: "sort" });
    expect(sorting[0].result.edits).toEqual(sorting[1].result.edits);
    expect(await workspace.query(uri, { kind: "colors" }, AbortSignal.abort())).toEqual([]);
    const superseded = workspace.query(uri, { kind: "fixes" });
    workspace.update(uri, code.replace("p-[16px]", "p-4"), 1);
    expect(await superseded).toEqual([]);
    expect(
      (await workspace.query(uri, { kind: "fixes" })).every(
        (entry) => entry.result.edits?.length === 0,
      ),
    ).toBe(true);
  } finally {
    await workspace.close();
  }
});
