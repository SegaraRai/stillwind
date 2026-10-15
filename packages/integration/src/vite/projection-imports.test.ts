import path from "node:path";

import { expect, test } from "vite-plus/test";

import { projectionImports } from "./projection-imports.ts";

test("projection imports preserve runtime requests and ignore erased TypeScript declarations", async () => {
  const source = `
import type { Only } from "./type?stillwind.css";
export type { Only } from "./export-type?stillwind.css";
export type * from "./export-all-type?stillwind.css";
import { type Inline } from "./inline-type?stillwind.css";
export { type Inline } from "./inline-export-type?stillwind.css";
import { unused } from "./value?stillwind.css";
import "./effect?stillwind.css";
export * from "./export?stillwind.css";
export * as named from "./namespace?stillwind.css";
import { type Other, value } from "./mixed?stillwind.css";
type Query = import("./query-type?stillwind.css").Only;
declare const erased: typeof import("./declared-type?stillwind.css");
const component = <div>{import("./dynamic?stillwind.css")}</div>;
import "./effect?stillwind.css";
import "./other.css";
import "./source?stillwind.css&raw";
const ignored = import(request);
const template = import(\`./template?stillwind.css\`);
`;
  expect(await projectionImports(source, path.join(import.meta.dirname, "Imports.tsx"))).toEqual([
    "./inline-type?stillwind.css",
    "./inline-export-type?stillwind.css",
    "./value?stillwind.css",
    "./effect?stillwind.css",
    "./export?stillwind.css",
    "./namespace?stillwind.css",
    "./mixed?stillwind.css",
    "./dynamic?stillwind.css",
  ]);
});

test.each([
  [
    "vue",
    '<script setup lang="ts">import "./source?stillwind.css";</script><template><div/></template>',
  ],
  ["svelte", '<script lang="ts">import "./source?stillwind.css";</script><div/>'],
  ["astro", '---\nimport "./source?stillwind.css";\n---\n<div/>'],
])("projection imports use the prepared %s script", async (extension, source) => {
  expect(
    await projectionImports(
      source,
      path.join(import.meta.dirname, `Imports.${extension}`).replaceAll("\\", "/"),
    ),
  ).toEqual(["./source?stillwind.css"]);
});

test("projection import parsing reports invalid source with its identity", async () => {
  const id = path.join(import.meta.dirname, "Invalid.ts");
  await expect(projectionImports('import "./source?stillwind.css"; const =', id)).rejects.toThrow(
    id,
  );
});
