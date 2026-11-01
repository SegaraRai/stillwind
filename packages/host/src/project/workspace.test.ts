import { mkdir, mkdtemp, rm, symlink, writeFile } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";

import { expect, test, vi } from "vite-plus/test";

import { AnalysisWorkspace } from "./workspace.ts";

test("saved and closed files retain the workspace while configuration files request reopening", async () => {
  const root = await mkdtemp(path.join(import.meta.dirname, ".test-refresh-"));
  const config = path.join(root, "stillwind.config.json");
  const entry = path.join(root, "entry.ts");
  let workspace: AnalysisWorkspace | undefined;
  try {
    await writeFile(
      config,
      JSON.stringify({
        projects: [
          {
            css: "root.css",
            files: ["*.ts"],
          },
        ],
      }),
    );
    workspace = await AnalysisWorkspace.open(config);
    workspace.update(entry, "export const value=1;", 1);
    expect(workspace.refresh([entry])).toBe(true);
    workspace.update(config, "{}", 1);
    expect(workspace.refresh([config])).toBe(false);
    const tsconfig = path.join(root, "tsconfig.json");
    workspace.update(tsconfig, "{}", 1);
    expect(workspace.refresh([tsconfig])).toBe(false);
    workspace.closeDocument(entry);
    const invalidate = vi.spyOn(workspace.projects[0].analysis, "invalidate");
    expect(workspace.refresh([entry])).toBe(true);
    expect(invalidate).toHaveBeenCalledOnce();
  } finally {
    await workspace?.close();
    await rm(root, { recursive: true, force: true });
  }
});

test("file changes matter when analysis read the file or its scope contains the file", async () => {
  const root = await mkdtemp(path.join(import.meta.dirname, ".test-affects-"));
  const config = path.join(root, "stillwind.config.json");
  const css = path.join(root, "root.css");
  const imported = path.join(root, "theme.css");
  let workspace: AnalysisWorkspace | undefined;
  try {
    await writeFile(
      config,
      JSON.stringify({
        projects: [
          {
            css: "root.css",
            files: ["src/**/*.ts"],
            typeFacts: false,
          },
        ],
      }),
    );
    await writeFile(css, '@import "./theme.css";');
    await writeFile(imported, "@utility red { color: red; }");
    await mkdir(path.join(root, "src"));
    await mkdir(path.join(root, "dist"));
    const entry = path.join(root, "src/entry.ts");
    await writeFile(entry, "export {};");
    workspace = await AnalysisWorkspace.open(config);
    workspace.update(entry, "export {};", 1);
    await workspace.analyze(entry);

    expect(workspace.affects([path.join(root, "dist/output.js")])).toBe(false);
    expect(workspace.affects([path.join(root, "dist/output.css")])).toBe(false);
    expect(workspace.affects([path.join(root, "dist/package.json")])).toBe(false);
    expect(workspace.affects([entry])).toBe(false);

    expect(workspace.affects([imported])).toBe(true);
    expect(workspace.affects([path.join(root, "src/target/new.ts")])).toBe(true);
    expect(workspace.affects([path.join(root, "package.json")])).toBe(true);
  } finally {
    await workspace?.close();
    await rm(root, { recursive: true, force: true });
  }
});

test.runIf(process.platform === "win32")(
  "queries reach a dependency that an importing target prepared first",
  async () => {
    const root = await mkdtemp(path.join(import.meta.dirname, ".test-spelling-"));
    const config = path.join(root, "stillwind.config.json");
    const entry = path.join(root, "entry.ts");
    const dependency = path.join(root, "dependency.ts");
    const source = `import { style } from "stillwind/react";\nexport const base = style("red");`;
    let workspace: AnalysisWorkspace | undefined;
    try {
      await writeFile(
        config,
        JSON.stringify({
          projects: [
            {
              css: "root.css",
              files: ["*.ts"],
            },
          ],
        }),
      );
      await writeFile(path.join(root, "root.css"), "@utility red { color: red; }");
      await writeFile(dependency, source);
      const importer = `import { style } from "stillwind/react";
import { base } from "./dependency";
export const card = style(base, "red");`;
      await writeFile(entry, importer);
      workspace = await AnalysisWorkspace.open(config);
      workspace.update(entry, importer, 1);
      await workspace.prepare([entry]);
      await workspace.analyze(entry);
      workspace.update(dependency, source, 1);
      await workspace.prepare([entry, dependency]);
      await workspace.analyze(entry);
      await workspace.analyze(dependency);
      const [answer] = await workspace.query(dependency, {
        kind: "hover",
        offset: source.indexOf("red") + 1,
      });
      expect(answer.result.hover).toBeDefined();
    } finally {
      await workspace?.close();
      await rm(root, { recursive: true, force: true });
    }
  },
);

test("plugin navigation uses unsaved files when the CSS graph cannot load", async () => {
  const root = await mkdtemp(path.join(import.meta.dirname, ".test-plugin-navigation-"));
  let workspace: AnalysisWorkspace | undefined;
  try {
    const config = path.join(root, "stillwind.config.json");
    const css = path.join(root, "theme.css");
    const plugin = path.join(root, "pending.mjs");
    await writeFile(
      config,
      JSON.stringify({
        projects: [
          {
            css: "theme.css",
            files: ["*.tsx"],
            typeFacts: false,
          },
        ],
      }),
    );
    await writeFile(css, '@plugin "missing-plugin";');
    workspace = await AnalysisWorkspace.open(config);
    const project = workspace.projects[0].analysis;
    project.update(plugin, "throw new Error('must not run');", 1);
    const text = '/* 🦀 */ @plugin "./pen';
    project.update(css, text, 1);
    const [answer] = await workspace.query(css, {
      kind: "completion",
      offset: text.length,
    });
    const item = answer.result.completions!.find((item) => item.label === "./pending.mjs")!;
    expect(item).toMatchObject({
      category: "file",
      newText: './pending.mjs"',
    });
    project.update(css, '@plugin "./pending.mjs";', 2);
    const [links] = await workspace.query(css, {
      kind: "links",
    });
    expect(links.result.links?.[0].target).toBe(pathToFileURL(plugin).href);
    const [stale] = await workspace.query(css, {
      kind: "resolve",
      candidate: item.candidate,
      input: item.input,
      snapshot: item.snapshot,
    });
    expect(stale?.result.hover).toBeUndefined();
    const directory = path.join(root, "plugin-source");
    await mkdir(directory);
    await symlink(
      directory,
      path.join(root, "plugins"),
      process.platform === "win32" ? "junction" : "dir",
    );
    project.update(css, '@plugin "./pl', 3);
    const [directories] = await workspace.query(css, {
      kind: "completion",
      offset: '@plugin "./pl'.length,
    });
    expect(directories.result.completions).toContainEqual(
      expect.objectContaining({
        label: "./plugins/",
        category: "directory",
        retrigger: true,
      }),
    );
  } finally {
    await workspace?.close();
    await rm(root, { recursive: true, force: true });
  }
});

test("target filters accept leading dots and absolute literal paths within configured scopes", async () => {
  const root = await mkdtemp(path.join(import.meta.dirname, ".test-config-"));
  let workspace: AnalysisWorkspace | undefined;
  try {
    const config = path.join(root, "stillwind.config.json");
    await writeFile(
      config,
      JSON.stringify({
        projects: [
          {
            css: "root.css",
            files: ["*.tsx"],
            exclude: ["ignored.tsx"],
          },
        ],
      }),
    );
    for (const file of ["entry.tsx", "entry[1].tsx", "ignored.tsx", "outside.txt"]) {
      await writeFile(path.join(root, file), "");
    }
    workspace = await AnalysisWorkspace.open(config);
    const project = workspace.projects[0];
    expect((await workspace.targets(project, ["./entry.tsx"])).map(path.normalize)).toEqual([
      path.join(root, "entry.tsx"),
    ]);
    expect(
      (await workspace.targets(project, [path.join(root, "entry[1].tsx")])).map(path.normalize),
    ).toEqual([path.join(root, "entry[1].tsx")]);
    expect(
      await workspace.targets(project, [
        path.join(root, "ignored.tsx"),
        path.join(root, "outside.txt"),
      ]),
    ).toEqual([]);
  } finally {
    await workspace?.close();
    await rm(root, { recursive: true, force: true });
  }
});

test.each([
  ["object", "settings"],
  ["function", "() => settings"],
  ["async function", "async () => settings"],
  ["promise", "Promise.resolve(settings)"],
])("workspace loads a %s configuration export", async (_kind, expression) => {
  const root = await mkdtemp(path.join(import.meta.dirname, ".test-config-"));
  let workspace: AnalysisWorkspace | undefined;
  try {
    const file = path.join(root, "stillwind.config.ts");
    await writeFile(
      file,
      `import { defineConfig } from "stillwind/config";
const settings = { projects: [{ css: "theme.css", files: ["*.tsx"], typeFacts: false }] };
export default defineConfig(${expression});`,
    );
    workspace = await AnalysisWorkspace.open(file);
    expect(workspace.projects.map((project) => project.config)).toEqual([
      {
        css: "theme.css",
        files: ["*.tsx"],
        typeFacts: false,
      },
    ]);
    expect(workspace.projects[0].analysis.host.root).toBe(root);
  } finally {
    await workspace?.close();
    await rm(root, { recursive: true, force: true });
  }
});

test("workspace validates the result of an asynchronous JavaScript configuration", async () => {
  const root = await mkdtemp(path.join(import.meta.dirname, ".test-config-"));
  try {
    const file = path.join(root, "stillwind.config.js");
    await writeFile(file, "export default async () => ({ projects: [{ files: ['*.tsx'] }] });");
    await expect(AnalysisWorkspace.open(file)).rejects.toThrow("projects[0].css:");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("workspace lints the CSS entry and imported user stylesheets with unsaved buffers", async () => {
  const root = await mkdtemp(path.join(import.meta.dirname, ".test-config-"));
  let workspace: AnalysisWorkspace | undefined;
  try {
    const config = path.join(root, "stillwind.config.json");
    await writeFile(
      config,
      JSON.stringify({
        projects: [{ css: "root.css", files: ["*.tsx"], typeFacts: false }],
      }),
    );
    await writeFile(
      path.join(root, "root.css"),
      '@import "./local.css"; @custom-variant hover (&:hover); @utility flex { display: flex; } @stillwind-slot utilities;',
    );
    const local = path.join(root, "local.css");
    await writeFile(local, ".card { @apply hover:hover:flex; }");
    workspace = await AnalysisWorkspace.open(config);
    const project = workspace.projects[0];
    expect(
      (await workspace.targets(project, undefined, { stylesheets: true })).map(path.normalize),
    ).toEqual([path.join(root, "local.css"), path.join(root, "root.css")]);
    expect(await workspace.targets(project)).toEqual([]);
    const results = await workspace.lint();
    expect(
      results.map(({ result }) => [
        path.basename(result.entry),
        result.status,
        result.reports.map((report) => report.code),
      ]),
    ).toEqual([
      ["local.css", "complete", ["redundant-variant"]],
      ["root.css", "complete", []],
    ]);
    const [answer] = await workspace.query(local, {
      kind: "fixes",
    });
    expect(answer.result.edits).toEqual([
      {
        start: 21,
        end: 27,
        newText: "",
      },
    ]);
    workspace.update(local, ".card { @apply nope; }", 1);
    const [unsaved] = await workspace.analyze(local);
    expect(unsaved.result.reports.map((report) => report.code)).toEqual(["unknown-utility"]);
  } finally {
    await workspace?.close();
    await rm(root, { recursive: true, force: true });
  }
});

test("CSS editor queries recover imported definitions from cold incomplete buffers without changing lint", async () => {
  const root = await mkdtemp(path.join(import.meta.dirname, ".test-css-editor-"));
  let workspace: AnalysisWorkspace | undefined;
  try {
    const config = path.join(root, "stillwind.config.json");
    await writeFile(
      config,
      JSON.stringify({
        projects: [
          {
            css: "root.css",
            files: ["*.tsx"],
            typeFacts: false,
          },
        ],
      }),
    );
    await writeFile(
      path.join(root, "root.css"),
      '@import "./local.css"; @utility flex {display:flex;} @custom-variant hover (&:hover);',
    );
    const local = path.join(root, "local.css");
    const text =
      "@theme { --color-brand:red; } /* 日本語 🦀 */ .a { color:var(--color-brand); @apply fl";
    await writeFile(local, text);
    workspace = await AnalysisWorkspace.open(config);
    const [colors] = await workspace.query(local, { kind: "colors" });
    expect(colors.result.colors).toHaveLength(1);
    expect(await workspace.query(local, { kind: "sort" })).toEqual([]);
    const [answer] = await workspace.query(local, { kind: "completion", offset: text.length });
    const item = answer.result.completions!.find((item) => item.label === "flex")!;
    expect(item.newText).toBe("flex");
    expect(item.start).toBe(text.length - 2);
    const [resolved] = await workspace.query(local, {
      kind: "resolve",
      candidate: item.candidate,
      input: item.input,
      snapshot: item.snapshot,
    });
    expect(resolved.result.hover?.css).toContain("display: flex");
    expect(await workspace.analyze(local)).toEqual([]);
    const next = "@theme { --color-latest:blue; } .a { color:--theme(--color-la";
    workspace.update(local, next, 1);
    const [updated] = await workspace.query(local, { kind: "completion", offset: next.length });
    expect(updated.result.completions?.map((item) => item.label)).toEqual(["--color-latest"]);
    expect(
      await workspace.query(path.join(root, "outside.css"), {
        kind: "completion",
        offset: 0,
      }),
    ).toEqual([]);
  } finally {
    await workspace?.close();
    await rm(root, { recursive: true, force: true });
  }
});
