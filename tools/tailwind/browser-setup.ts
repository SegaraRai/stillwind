import { resolve } from "node:path";

import type { TestProject } from "vite-plus/test/node";

import { compileParity } from "./comparison.ts";
import { compileTailwindCss } from "./compiler.ts";
import { stillwindFixtureCss } from "./stillwind-css.ts";

export interface BrowserCssCase {
  name: string;
  properties: string[];
  expected: string;
  actual: string;
  expectedClass: string;
  actualClass: string;
  parent?: string;
  extra?: string;
  different?: boolean;
}

declare module "vite-plus/test" {
  interface ProvidedContext {
    tailwindCases: BrowserCssCase[];
  }
}

export default async function setup(project: TestProject) {
  const root = resolve(import.meta.dirname, "../..");
  const baseInput =
    '@import "tailwindcss/theme" layer(theme);@import "tailwindcss/utilities" layer(utilities);';
  const cases = [
    {
      name: "arbitrary shadow composes with blur and color",
      candidates: ["blur-sm", "drop-shadow-[0_1px_2px_rgb(0_0_0/0.25)]", "drop-shadow-red-500"],
      properties: ["filter"],
    },
    {
      name: "transform-cpu inherits unregistered input",
      candidates: ["transform-cpu"],
      properties: ["transform"],
      parent: "--tw-rotate-x:rotateX(30deg)",
    },
    {
      name: "transform-gpu inherits unregistered input",
      candidates: ["transform-gpu"],
      properties: ["transform"],
      parent: "--tw-rotate-x:rotateX(30deg)",
    },
    {
      name: "transform explicitly registers its inputs",
      candidates: ["transform"],
      properties: ["transform"],
      parent: "--tw-rotate-x:rotateX(30deg)",
    },
    {
      name: "ring-offset alone preserves inherited color",
      candidates: ["ring-offset-2"],
      properties: ["--tw-ring-offset-color"],
      parent: "--tw-ring-offset-color:red",
    },
    {
      name: "ring and ring-offset activate support together",
      candidates: ["ring-2", "ring-offset-2"],
      properties: ["box-shadow"],
    },
    {
      name: "theme slot can be overridden by a later layer",
      candidates: ["bg-brand"],
      properties: ["background-color"],
      input: "@theme {--color-brand:red;}",
      extra: "@layer overrides{:root{--color-brand:green}}",
    },
    {
      name: "mask initial values survive fallback omission",
      candidates: ["mask-b-from-0"],
      properties: ["mask-image", "mask-composite"],
    },
    {
      name: "empty blur remains an empty value",
      candidates: ["blur-none", "brightness-50"],
      properties: ["filter"],
    },
    {
      name: "theme keyframes deliberately allow layered overrides",
      candidates: ["animate-spin"],
      properties: ["opacity", "transform"],
      extra: "@layer overrides{@keyframes spin{from{opacity:1}to{opacity:0}}}",
      different: true,
    },
  ];
  const payload = [];
  for (const entry of cases) {
    const input = baseInput + (entry.input ?? "");
    const expected = await compileTailwindCss({
      css: input,
      candidates: entry.candidates,
      base: resolve(root, "fixtures/tailwind-v4"),
    });
    const native = compileParity(
      `import {style} from 'stillwind/react';style(${JSON.stringify(entry.candidates.join(" "))});`,
      stillwindFixtureCss(input),
    );
    if (native.diagnostics.length) {
      throw new Error(JSON.stringify(native.diagnostics));
    }
    payload.push({
      ...entry,
      expected,
      actual: native.css,
      expectedClass: entry.candidates.join(" "),
      actualClass: native.classes[0].className,
    });
  }
  project.provide("tailwindCases", payload);
}
