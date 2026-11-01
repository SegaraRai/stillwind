import { mkdtemp, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { expect, test } from "vite-plus/test";

import type { CliResult } from "../src/result.ts";
import { run } from "./run.ts";

const entry = [
  'import {style} from "stillwind/react";',
  'export const first=style("red");',
  "export const second=style(",
  '  "blue",',
  ");",
  "export const plain=1;",
  "",
].join("\r\n");

/**
 * Runs inspect in JSON format.
 *
 * @param root - Project directory.
 * @param args - Arguments after the command.
 * @returns The process result, with each preview reduced to its file, line, and source.
 */
async function inspect(root: string, args: readonly string[]) {
  const { code, stdout } = await run(root, ["inspect", ...args, "--format", "json"]);
  const result = JSON.parse(stdout) as CliResult;
  return {
    code,
    result,
    previews: result.previews.map(({ location, source }) => [
      path.basename(fileURLToPath(location.uri)),
      location.startLine,
      source.replace(/\s+/g, ""),
    ]),
  };
}

test("inspect selects generated CSS by file, line, position, and glob", async () => {
  const root = await mkdtemp(path.resolve(import.meta.dirname, "../../host/.test-cli-inspect-"));
  try {
    await writeFile(
      path.join(root, "stillwind.config.json"),
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
      "@utility red{color:red} @utility blue{color:blue} @stillwind-slot utilities;",
    );
    await writeFile(path.join(root, "entry.tsx"), entry);
    await writeFile(
      path.join(root, "other.tsx"),
      'import {style} from "stillwind/react";export const other=style("red");',
    );
    await writeFile(
      path.join(root, "broken.tsx"),
      'import {style} from "stillwind/react";import {value} from "./missing.ts";export const broken=style(value);',
    );

    const first = ["entry.tsx", 1, 'style("red")'];
    const second = ["entry.tsx", 2, 'style("blue",)'];
    const other = ["other.tsx", 0, 'style("red")'];

    const file = await inspect(root, ["entry.tsx"]);
    expect(file.code).toBe(0);
    expect(file.previews).toEqual([first, second]);
    expect(file.result).toMatchObject({
      command: "inspect",
      diagnostics: [],
      exitCode: 0,
      summary: {
        files: 1,
        previews: 2,
      },
    });
    expect(file.result.previews[0].css).toContain("color: red");
    expect(file.result.previews[1].location).toMatchObject({
      startLine: 2,
      startColumn: 20,
      endLine: 4,
      endColumn: 1,
    });

    expect((await inspect(root, ["entry.tsx:4"])).previews).toEqual([second]);
    expect((await inspect(root, ["entry.tsx:2:22"])).previews).toEqual([first]);
    expect((await inspect(root, ["entry.tsx:2:27"])).previews).toEqual([["entry.tsx", 1, "red"]]);
    expect((await inspect(root, ["./entry.tsx:2", "other.tsx"])).previews).toEqual([first, other]);
    expect((await inspect(root, ["e*.tsx", "entry.tsx:2"])).previews).toEqual([first, second]);
    expect((await inspect(root, [path.join(root, "other.tsx")])).previews).toEqual([other]);

    for (const target of ["entry.tsx:6", "entry.tsx:99", "entry.tsx:2:999"]) {
      const empty = await inspect(root, [target]);
      expect(empty.code, target).toBe(1);
      expect(empty.result.previews).toEqual([]);
    }

    for (const target of ["absent.tsx", "root.css", "entry.tsx:0", "broken.tsx"]) {
      expect((await inspect(root, [target])).code, target).toBe(2);
    }

    await writeFile(
      path.join(root, "partial.tsx"),
      'import {style} from "stillwind/react";\nexport const good=style("red");\nexport const bad=style("missing");\nexport const mixed=style("red missing");\n',
    );
    const partial = await inspect(root, ["partial.tsx"]);
    expect(partial.code).toBe(1);
    expect(partial.result.summary).toMatchObject({
      previews: 3,
      invalidPreviews: 2,
    });
    expect(
      partial.result.previews.map(({ css, valid, diagnostics }) => [
        css !== "",
        valid,
        diagnostics.map(({ severity, code }) => [severity, code]),
      ]),
    ).toEqual([
      [true, true, []],
      [false, false, [["error", "unknown-utility"]]],
      [true, false, [["error", "unknown-utility"]]],
    ]);
    expect((await inspect(root, ["partial.tsx:2"])).code).toBe(0);
    expect(partial.result.previews[1].diagnostics[0].docs).toMatch(
      /lints[/\\]unknown-utility(\.md)?$/,
    );
    const failed = await inspect(root, ["partial.tsx:3:24"]);
    expect(failed.code).toBe(1);
    expect(failed.result.previews).toMatchObject([
      {
        valid: false,
      },
    ]);
    expect((await run(root, ["inspect", "partial.tsx"])).stdout).toContain(
      "error unknown-utility: unknown utility `missing`",
    );

    await writeFile(
      path.join(root, "stillwind.config.json"),
      JSON.stringify({
        projects: [
          {
            css: "root.css",
            files: ["*.tsx"],
            typeFacts: false,
            lint: { rules: { "unknown-utility": "warn" } },
          },
        ],
      }),
    );
    const warned = await inspect(root, ["partial.tsx"]);
    expect(warned.code).toBe(0);
    expect(warned.result.summary).toMatchObject({
      previews: 3,
      invalidPreviews: 0,
    });
    expect(warned.result.previews[1]).toMatchObject({
      css: "",
      valid: true,
      diagnostics: [
        {
          severity: "warning",
          code: "unknown-utility",
        },
      ],
    });

    const text = await run(root, ["inspect", "entry.tsx:4"]);
    expect(text.code).toBe(0);
    expect(text.stdout).toContain("entry.tsx:3:21 [project 1]");
    expect(text.stdout).toContain("1 preview in 1 file.");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
