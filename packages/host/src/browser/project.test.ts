import { expect, test } from "vite-plus/test";

import { BrowserProject } from "./index.ts";

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
