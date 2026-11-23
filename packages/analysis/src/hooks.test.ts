import path from "node:path";

import { defineHook, type AnalysisHook } from "@stillwind/config";
import type { WorkspaceProject, WorkspaceResult } from "@stillwind/host";
import { expect, test } from "vite-plus/test";

import { applyFormatHooks, ProjectHookSession } from "./hooks.ts";
import type { RewriteResult } from "./rewrite.ts";

function fakeProject(
  root: string,
  hooks?: readonly AnalysisHook[],
  name?: string,
): WorkspaceProject {
  return {
    config: {
      css: "root.css",
      files: ["src/**/*.ts"],
      ...(name ? { name } : {}),
      ...(hooks ? { hooks } : {}),
    },
    configFile: path.join(root, "stillwind.config.ts"),
    analysis: {
      host: {
        root,
      },
    } as WorkspaceProject["analysis"],
  };
}

function fakeWorkspaceResult(project: number, config: string, entry: string): WorkspaceResult {
  return {
    project,
    config,
    result: {
      entry,
      revision: 1,
      status: "complete",
      reports: [],
    },
  };
}

test("ProjectHookSession runs setup/teardown in LIFO order and cleans up on setup failure", async () => {
  const events: string[] = [];
  const root = path.resolve("/workspace/app");
  const first = defineHook({
    name: "first",
    setup: () => {
      events.push("setup:first");
    },
    teardown: () => {
      events.push("teardown:first");
    },
    format: ({ code }) => code,
  });
  const second = defineHook({
    name: "second",
    setup: () => {
      events.push("setup:second");
    },
    teardown: () => {
      events.push("teardown:second");
    },
    format: ({ code }) => code,
  });
  const failing = defineHook({
    name: "failing",
    setup: () => {
      events.push("setup:failing");
      throw new Error("boom");
    },
    teardown: () => {
      events.push("teardown:failing");
    },
    format: ({ code }) => code,
  });

  const session = await ProjectHookSession.open(
    [fakeProject(root, [first, second])],
    "fmt",
    "write",
  );
  expect(events).toEqual(["setup:first", "setup:second"]);
  await session.close();
  expect(events).toEqual(["setup:first", "setup:second", "teardown:second", "teardown:first"]);

  events.length = 0;
  await expect(
    ProjectHookSession.open([fakeProject(root, [first, failing])], "fmt", "write"),
  ).rejects.toThrow('Hook "failing" setup failed: boom');
  expect(events).toEqual(["setup:first", "setup:failing", "teardown:first"]);
});

test("applyFormatHooks chains hooks in order, deduplicates identical overlapping hooks, and reports conflicts or errors", async () => {
  const rootA = path.resolve("/workspace/a");
  const rootB = path.resolve("/workspace/b");
  const sharedFile = path.join(rootA, "shared.ts");
  const failingFile = path.join(rootA, "fail.ts");

  let sharedCalls = 0;
  const sharedHook = defineHook({
    name: "shared",
    format(file) {
      sharedCalls += 1;
      return `${file.code}//changed=${file.changed}\n`;
    },
  });
  const secondHook = defineHook({
    name: "second",
    format(file) {
      if (file.path === failingFile) {
        throw new Error("syntax error");
      }
      return `${file.code}//second=${file.changed}\n`;
    },
  });

  const projectsSame = [
    fakeProject(rootA, [sharedHook, secondHook]),
    fakeProject(rootB, [sharedHook, secondHook]),
  ];
  const sessionSame = await ProjectHookSession.open(projectsSame, "fmt", "write");
  try {
    const outputs: RewriteResult[] = [
      {
        file: sharedFile,
        code: "const a = 1;\n",
        edits: [],
        results: [
          fakeWorkspaceResult(0, projectsSame[0].configFile, sharedFile),
          fakeWorkspaceResult(1, projectsSame[1].configFile, sharedFile),
        ],
      },
      {
        file: failingFile,
        code: "const b = 2;\n",
        edits: [],
        results: [fakeWorkspaceResult(0, projectsSame[0].configFile, failingFile)],
      },
    ];
    const originals = new Map([
      [sharedFile, "const a = 1;\n"],
      [failingFile, "const b = 2;\n"],
    ]);
    const formatted = await applyFormatHooks(
      projectsSame,
      { outputs, failures: [] },
      originals,
      sessionSame,
    );
    expect(sharedCalls).toBe(2);
    expect(formatted.changed).toBe(true);
    expect(formatted.outputs).toHaveLength(1);
    expect(formatted.outputs[0].code).toBe("const a = 1;\n//changed=false\n//second=true\n");
    expect(formatted.failures).toHaveLength(1);
    expect(formatted.failures[0].file).toBe(failingFile);
    expect(formatted.failures[0].message).toContain('Hook "second" failed');
  } finally {
    await sessionSame.close();
  }

  const projectsDiff = [
    fakeProject(rootA, [defineHook({ name: "a", format: ({ code }) => `${code}//a` })]),
    fakeProject(rootB, [defineHook({ name: "b", format: ({ code }) => `${code}//b` })]),
  ];
  const sessionDiff = await ProjectHookSession.open(projectsDiff, "fmt", "write");
  try {
    const formatted = await applyFormatHooks(
      projectsDiff,
      {
        outputs: [
          {
            file: sharedFile,
            code: "x",
            edits: [],
            results: [
              fakeWorkspaceResult(0, projectsDiff[0].configFile, sharedFile),
              fakeWorkspaceResult(1, projectsDiff[1].configFile, sharedFile),
            ],
          },
        ],
        failures: [],
      },
      new Map([[sharedFile, "x"]]),
      sessionDiff,
    );
    expect(formatted.outputs).toHaveLength(0);
    expect(formatted.failures).toHaveLength(1);
    expect(formatted.failures[0].message).toContain("Conflicting project format hooks");
  } finally {
    await sessionDiff.close();
  }
});
