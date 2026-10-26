import { definePlugin, lintPresets } from "@stillwind/config";
import { expect, test } from "vite-plus/test";

import { BrowserAnalysisWorkspace } from "./browser.ts";

test("diagnostics, queries and builds share CSS across source edits and reload observed plugin inputs", async () => {
  const uri = "file:///workspace/entry.ts";
  const code = 'import {style} from "stillwind/react"; export const value=style("demo-red");';
  let loads = 0;
  let unrelatedLoads = 0;
  const calls: string[][] = [];
  const workspace = new BrowserAnalysisWorkspace({
    config: "file:///workspace/stillwind.config.ts",
    projects: [
      {
        root: "/workspace",
        css: "theme.css",
        files: ["*.ts"],
        typeFacts: false,
        plugins: [
          definePlugin({
            name: "demo",
            css: async (context) => {
              loads++;
              context.watch("/workspace/plugin-input.txt");
              return `@stillwind-dynamic demo-*; @utility selected { color:${await context.read("/workspace/plugin-input.txt")}; }`;
            },
            resolve: (candidates) => {
              calls.push([...candidates]);
              return candidates
                .map((candidate) => `@utility ${candidate}{color:${candidate.slice(5)}}`)
                .join("");
            },
          }),
        ],
      },
      {
        root: "/unused",
        files: ["*.ts"],
        css: "theme.css",
        typeFacts: false,
        plugins: [
          definePlugin({
            name: "unused",
            css: () => {
              unrelatedLoads++;
              return "@utility red{color:red}";
            },
          }),
        ],
      },
    ],
    files: new Map([
      ["/workspace/entry.ts", code],
      ["/workspace/other.ts", code.replace("demo-red", "demo-blue")],
      ["/workspace/plugin-input.txt", "red"],
      ["/workspace/theme.css", '@plugin "demo"; @stillwind-slot utilities;'],
      ["/unused/entry.ts", code],
      ["/unused/theme.css", '@plugin "unused";'],
    ]),
  });
  try {
    const analysis = (await workspace.analyze(uri))[0].result;
    expect(analysis.status, JSON.stringify(analysis)).toBe("complete");
    expect((await workspace.compile(uri, workspace.revision)).css).toContain("color:red");
    await workspace.query(uri, { kind: "sort" });
    expect(loads).toBe(1);
    expect(calls).toHaveLength(1);
    expect(calls[0].toSorted()).toEqual(["demo-blue", "demo-red"]);
    workspace.update(uri, code.replace("value", "updated"), 1);
    expect((await workspace.analyze(uri))[0].result.status).toBe("complete");
    expect((await workspace.compile(uri, workspace.revision)).css).toContain("color:red");
    expect(loads).toBe(1);
    workspace.update("file:///workspace/plugin-input.txt", "blue", 1);
    workspace.update(uri, code.replace("demo-red", "selected"), 2);
    expect((await workspace.compile(uri, workspace.revision)).css).toContain("color:blue");
    expect(loads).toBe(2);
    expect(unrelatedLoads).toBe(0);
    workspace.update(
      "file:///workspace/theme.css",
      "@utility selected{color:green} @stillwind-slot utilities;",
      1,
    );
    expect((await workspace.compile(uri, workspace.revision)).css).toContain("color:green");
  } finally {
    await workspace.close();
  }
});

test("CSS colors recover cold buffers while sorting requires original CSS after recovery", async () => {
  const uri = "file:///workspace/theme.css";
  const definitions =
    "@stillwind-theme-namespace --color-* [color]; @theme {--color-brand:red;} @utility a {display:block;} @utility b {padding:1px;} @utility bg-* {background-color:--value(--color-*);}";
  const source = `${definitions} /* 日本語 🦀 */ .a { @apply bg-brand`;
  const workspace = new BrowserAnalysisWorkspace({
    config: "config",
    projects: [
      {
        root: "/workspace",
        files: ["*.ts"],
        css: "theme.css",
        typeFacts: false,
        lint: false,
        sort: {
          applyStatements: true,
        },
      },
    ],
    files: new Map([["/workspace/theme.css", source]]),
  });
  try {
    const [answer] = await workspace.query(uri, { kind: "colors" });
    const color = answer.result.colors![0];
    expect(source.slice(color.start, color.end)).toBe("bg-brand");
    const [presentation] = await workspace.query(uri, {
      kind: "colorPresentation",
      start: color.start,
      end: color.end,
      color: {
        red: 0,
        green: 0,
        blue: 1,
        alpha: 1,
      },
    });
    expect(presentation.result.edits![0].newText).toBe("bg-[#0000ff]");
    await expect(workspace.query(uri, { kind: "sort" })).rejects.toThrow();
    workspace.update(uri, `${definitions} .a { @apply b; @apply a; }`, 1);
    const [sort] = await workspace.query(uri, { kind: "sort" });
    expect(sort.result.edits![0].newText).toBe("@apply a; @apply b;");
  } finally {
    await workspace.close();
  }
});

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
    workspace.update(uri, "const broken = (", 2);
    const broken = (await workspace.analyze(uri))[0].result;
    expect(broken.error).toBeUndefined();
    expect(broken.reports[0].severity).toBe("error");
    workspace.update(uri, code, 3);
    expect((await workspace.compile(uri, workspace.revision)).graph.generation).toBe("complete");
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
    "@stillwind-theme-namespace --spacing-* [length] scale; @theme { --spacing:.25rem; } @utility p-* { padding:--value(--spacing(),[length]); } @utility block { display:block; } @utility flex { display:flex; } @stillwind-slot utilities;";
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
    workspace.update(uri, code.replace("p-[16px] flex block", "p-4 flex"), 1);
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
