import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";

import { expect, test } from "vite-plus/test";

import { run } from "./run.ts";

test("CLI fmt, check, and info execute project format hooks in memory while lint skips them", async () => {
  const root = await mkdtemp(path.resolve(import.meta.dirname, "../../host/.test-cli-hooks-"));
  try {
    const logFile = path.join(root, "lifecycle.log");
    await writeFile(
      path.join(root, "root.css"),
      "@utility block{display:block} @utility p-4{padding:1rem} @stillwind-slot utilities;",
    );
    const file = path.join(root, "entry.tsx");
    await writeFile(
      file,
      'import { style } from "stillwind/react";\nexport const box = style("p-4 block");\n',
    );
    await writeFile(
      path.join(root, "stillwind.config.ts"),
      `import { appendFileSync } from "node:fs";
const log = ${JSON.stringify(logFile)};
export default {
  projects: [
    {
      name: "app",
      css: "root.css",
      files: ["*.tsx"],
      typeFacts: false,
      hooks: [
        {
          name: "post-fmt",
          setup(ctx) {
            appendFileSync(log, \`setup:\${ctx.command}:\${ctx.mode}\\n\`);
          },
          teardown(ctx) {
            appendFileSync(log, \`teardown:\${ctx.command}:\${ctx.mode}\\n\`);
          },
          format(file) {
            if (!file.path.endsWith(".tsx")) {
              return;
            }
            return file.code.replaceAll(";", "");
          },
        },
      ],
    },
  ],
};
`,
    );

    const info = await run(root, ["info"]);
    expect(info.code, info.stderr).toBe(0);
    expect(info.stdout).toContain("hooks: post-fmt");

    const fmtCheck = await run(root, ["fmt", "--check", "--format", "json"]);
    expect(fmtCheck.code, fmtCheck.stderr).toBe(1);
    expect(JSON.parse(fmtCheck.stdout).changed.map(path.normalize)).toEqual([file]);
    expect(await readFile(file, "utf8")).toContain(";");

    // lint --fix does not run format hooks.
    const lintFix = await run(root, ["lint", "--fix"]);
    expect(lintFix.code, lintFix.stderr).toBe(0);
    expect(await readFile(file, "utf8")).toContain('style("p-4 block");');

    // check --fix runs Stillwind sort + format hook in one pass.
    const checkFix = await run(root, ["check", "--fix"]);
    expect(checkFix.code, checkFix.stderr).toBe(0);
    expect(await readFile(file, "utf8")).toBe(
      'import { style } from "stillwind/react"\nexport const box = style("block p-4")\n',
    );

    const checkClean = await run(root, ["check"]);
    expect(checkClean.code, checkClean.stderr).toBe(0);

    const lifecycle = await readFile(logFile, "utf8");
    expect(lifecycle).toContain("setup:fmt:check\nteardown:fmt:check\n");
    expect(lifecycle).toContain("setup:check:write\nteardown:check:write\n");
    expect(lifecycle).toContain("setup:check:check\nteardown:check:check\n");
    expect(lifecycle).not.toContain("lint");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
