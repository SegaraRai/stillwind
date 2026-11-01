import { mkdtemp, rm, writeFile } from "node:fs/promises";
import path from "node:path";

import { expect, test } from "vite-plus/test";

import type { CliResult } from "../src/result.ts";
import { run } from "./run.ts";

/**
 * Runs a design-system command in JSON format.
 *
 * @param root - Project directory.
 * @param args - Command and its arguments.
 * @returns The exit code and the parsed result.
 */
async function json(root: string, args: readonly string[]) {
  const { code, stdout } = await run(root, [...args, "--format", "json"]);
  return {
    code,
    result: JSON.parse(stdout) as CliResult,
  };
}

test("theme, utilities, and variants read the design system without source files", async () => {
  const root = await mkdtemp(path.resolve(import.meta.dirname, "../../host/.test-cli-design-"));
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
      `@import "stillwind";
@theme { --color-accent: #123456; --spacing-panel: 3rem; }
@utility surface { color: var(--color-accent); }
@custom-variant hocus (&:hover, &:focus);`,
    );

    const theme = await json(root, ["theme", "--source", "project"]);
    expect(theme.code).toBe(0);
    expect(theme.result.tokens).toEqual([
      {
        project: 0,
        name: "--color-accent",
        value: "#123456",
        source: "project",
        inline: false,
      },
      {
        project: 0,
        name: "--spacing-panel",
        value: "3rem",
        source: "project",
        inline: false,
      },
    ]);
    const matched = await json(root, ["theme", "color-acc*", "spacing-panel"]);
    expect(matched.result.tokens.map(({ name }) => name)).toEqual([
      "--color-accent",
      "--spacing-panel",
    ]);
    const defaults = await json(root, ["theme", "color-red-500"]);
    expect(defaults.result.tokens).toMatchObject([
      {
        source: "default",
      },
    ]);
    const missing = await json(root, ["theme", "color-accent", "color-absent*"]);
    expect(missing.code).toBe(1);
    expect(missing.result.unmatched).toEqual(["color-absent*"]);

    const exact = await json(root, [
      "utilities",
      "p-4",
      "hover:bg-accent",
      "p-[3px]",
      "w-[calc(100%*2)]",
      "surface",
    ]);
    expect(exact.code).toBe(0);
    expect(exact.result.candidates.map(({ name, valid }) => [name, valid])).toEqual([
      ["p-4", true],
      ["hover:bg-accent", true],
      ["p-[3px]", true],
      ["w-[calc(100%*2)]", true],
      ["surface", true],
    ]);
    expect(exact.result.candidates[4].css).toContain("color: var(--color-accent)");

    const unknown = await json(root, ["utilities", "surfce", "flex"]);
    expect(unknown.code).toBe(1);
    expect(unknown.result.candidates).toMatchObject([
      {
        name: "surfce",
        valid: false,
        css: "",
        suggestion: "surface",
      },
      {
        name: "flex",
        valid: true,
      },
    ]);

    const canonical = await json(root, [
      "utilities",
      "[padding:16px]",
      "[&:focus]:[display:flex]",
      "[hyphens:manual]",
      "p-4",
    ]);
    expect(canonical.code).toBe(0);
    expect(canonical.result.candidates.map(({ canonical }) => canonical)).toEqual([
      "p-4",
      "focus:flex",
      undefined,
      undefined,
    ]);
    expect(canonical.result.candidates[0].css).toContain("padding: 16px");
    expect(
      (await json(root, ["variants", "[&:focus]", "[&:hover]"])).result.candidates.map(
        ({ canonical }) => canonical,
      ),
    ).toEqual(["focus", undefined]);
    expect((await run(root, ["utilities", "[padding:16px]", "--color", "never"])).stdout).toContain(
      "help: The canonical spelling is p-4.",
    );

    const search = await json(root, ["utilities", "bg-acc*", "*-panel"]);
    expect(search.code).toBe(0);
    expect(search.result.candidates.map(({ name }) => name)).toEqual(
      expect.arrayContaining(["bg-accent", "p-panel", "m-panel"]),
    );
    const empty = await json(root, ["utilities", "zzz*"]);
    expect(empty.code).toBe(1);
    expect(empty.result.unmatched).toEqual(["zzz*"]);

    const variants = await json(root, ["variants", "hocus", "hocu", "hoc*"]);
    expect(variants.code).toBe(1);
    expect(variants.result.candidates).toMatchObject([
      {
        name: "hocus",
        kind: "variant",
        valid: true,
      },
      {
        name: "hocu",
        kind: "variant",
        valid: false,
        suggestion: "hocus",
      },
    ]);
    expect(variants.result.candidates[0].css).toContain("&:hover");
    expect((await json(root, ["utilities", "hocus:*"])).result.unmatched).toEqual([]);

    expect((await run(root, ["theme", "--color-*"])).code).toBe(2);
    const text = await run(root, ["utilities", "surfce", "--color", "never"]);
    expect(text.stdout).toContain("surfce: unknown utility");
    expect(text.stdout).toContain("help: Did you mean surface?");
    expect((await run(root, ["theme", "color-accent"])).stdout).toContain(
      "--color-accent: #123456",
    );

    await writeFile(path.join(root, "root.css"), "@theme {");
    const broken = await json(root, ["theme"]);
    expect(broken.code).toBe(2);
    expect(broken.result.command).toBe("theme");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
