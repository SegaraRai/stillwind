import { expect, test } from "vite-plus/test";

import { BrowserProject } from "./index.ts";

test("failed graphs invalidate cached dependencies and keep unrelated CSS", async () => {
  const project = new BrowserProject({
    root: "/workspace",
    config: { css: "theme.css", files: ["*.ts"], typeFacts: false, types: false },
    files: new Map([
      [
        "/workspace/styles.ts",
        'import {style} from "stillwind/react"; export const card = style("block");',
      ],
      [
        "/workspace/other.ts",
        'import {style} from "stillwind/react"; export const other = style("flex");',
      ],
      [
        "/workspace/entry.ts",
        'import {style} from "stillwind/react"; import {card} from "./styles"; export const value = style(card, style("block"));',
      ],
      [
        "/workspace/theme.css",
        "@utility block { display:block; } @utility flex { display:flex; } @stillwind-slot utilities;",
      ],
    ]),
  });
  try {
    await project.analyze("/workspace/styles.ts");
    await project.analyze("/workspace/other.ts");
    expect((await project.analyze("/workspace/entry.ts")).generation).toBe("blocked");
    expect(project.css()).toContain("display:flex");
    expect(project.css()).not.toContain("display:block");
    expect([...project.cssValues().keys()]).toEqual(["/workspace/other.ts"]);
    await project.analyze("/workspace/styles.ts");
    expect(project.css()).toContain("display:block");
  } finally {
    await project.close();
  }
});

test("browser projects deliver explicit CSS values and resolve captured SVG resources", async () => {
  const files = new Map([
    [
      "/workspace/entry.ts",
      'import {style} from "stillwind/react"; import css from "stillwind/module-css"; export const card = style("red"); export {css};',
    ],
    [
      "/workspace/theme.css",
      '@utility red { color:red; background-image:url("./icon.svg"); } @stillwind-slot utilities;',
    ],
    ["/workspace/icon.svg", '<svg xmlns="http://www.w3.org/2000/svg"/>'],
    ["/workspace/src/stillwind.generated.d.ts", "stale generated declarations"],
  ]);
  const project = new BrowserProject({
    root: "/workspace",
    files,
    config: {
      css: "theme.css",
      files: ["**/*.ts"],
      automatic: false,
      typeFacts: false,
      types: false,
    },
  });
  try {
    const graph = await project.analyze("/workspace/entry.ts");
    expect(graph.generation).toBe("complete");
    expect(project.css()).not.toContain("color:red");
    const value = project.cssValues().get("/workspace/entry.ts");
    expect(value?.css).toContain("color:red");
    expect(value?.css).toContain("data:image/svg+xml");
    expect(value?.parts.map((part) => part.css).join("")).toBe(value?.css);
    expect(project.declarations().size).toBe(0);
  } finally {
    await project.close();
  }
});
