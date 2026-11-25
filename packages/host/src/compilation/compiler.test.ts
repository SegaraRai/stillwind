import path from "node:path";

import { scanSource, type NativeProjectSession } from "@stillwind/native";
import { expect, test } from "vite-plus/test";

import type { TypeScriptFactsService } from "../semantics/node/service.ts";
import { compileModule } from "./compiler.ts";
import { fileModuleDescriptor } from "./filesystem.ts";

test("a rejected generator name is reported through the host warnings", async () => {
  // A rejected name comes from the host's generator rather than from the source, so it has no
  // position to report as a diagnostic or a lint finding.
  const session = {
    scanSource,
    compile: () => ({
      modules: [
        {
          moduleId: "rejected.tsx",
          code: "export const x = undefined;",
          diagnostics: [],
          lintFindings: [],
          lint_policy: {},
          composition: [],
          rejectedNames: [
            {
              kind: "class",
              hash: "a".repeat(64),
              reason: "the host generator failed",
              substituted: true,
            },
          ],
        },
      ],
    }),
  } as unknown as NativeProjectSession;
  const warnings: string[] = [];
  const output = await compileModule(
    session,
    () => ({}) as TypeScriptFactsService,
    "export const x = 1;",
    path.resolve("rejected.tsx"),
    {
      resolve: async () => null,
      read: async () => {
        expect.fail("unexpected read");
      },
      describe: (id) => ({
        ...fileModuleDescriptor(process.cwd(), id),
        document: {
          ...fileModuleDescriptor(process.cwd(), id).document,
          uri: `untitled:${encodeURIComponent(id)}`,
        },
      }),
      watch: () => {},
      warn: (message) => warnings.push(message),
    },
  );
  expect(output?.code).toBeTruthy();
  expect(warnings.length).toBe(1);
  expect(warnings[0]!).toMatch(/class/);
  expect(warnings[0]!).toMatch(/the host generator failed/);
  expect(warnings[0]!).toMatch(/a{64}/);
});

test("an import orders the module with styles that it reaches through modules without styles, and watches them", async () => {
  const root = path.resolve("ordering");
  const files: Record<string, string> = {
    "app.tsx": `import { style } from "stillwind/react";
import { Wrapper } from "./wrapper";
export const App = () => <Wrapper {...style("p-4")} />;
`,
    "wrapper.tsx": `import { Button } from "./ui";
export const Wrapper = (props: object) => <Button {...props} />;
`,
    "ui.ts": `export { Button } from "./button";
export { Card } from "./card";
`,
    "button.tsx": `import { style } from "stillwind/react";
export const Button = (props: object) => <button {...style("p-2", style.forward(props))} />;
`,
    "card.tsx": `import { style } from "stillwind/react";
export const Card = () => <div {...style("p-1")} />;
`,
  };
  const file = (id: string) => path.relative(root, id).replaceAll("\\", "/");
  let compiled: {
    moduleId: string;
    orderingModules?: {
      moduleId: string;
    }[];
  }[] = [];
  const watched = new Set<string>();
  const session = {
    scanSource,
    compile: (sources: typeof compiled) => {
      compiled = sources;
      return {
        modules: [
          {
            moduleId: sources[0]!.moduleId,
            code: "",
            diagnostics: [],
            lintFindings: [],
            composition: [],
            rejectedNames: [],
          },
        ],
      };
    },
  } as unknown as NativeProjectSession;
  await compileModule(
    session,
    () => ({}) as TypeScriptFactsService,
    files["app.tsx"]!,
    path.join(root, "app.tsx"),
    {
      resolve: async (specifier, importer) => {
        if (!specifier.startsWith("./")) {
          return null;
        }
        const base = path.join(path.dirname(importer), specifier);
        const name = [".tsx", ".ts"]
          .map((extension) => base + extension)
          .find((id) => {
            return file(id) in files;
          });
        return name ? { id: name } : null;
      },
      read: async (id) => files[file(id)]!,
      describe: (id) => fileModuleDescriptor(root, id),
      watch: (id) => watched.add(file(id)),
      warn: (message) => expect.fail(message),
    },
  );
  const app = compiled.find((source) => source.moduleId.endsWith("app.tsx"));
  expect(app?.orderingModules?.map((ordering) => ordering.moduleId)).toEqual(["button.tsx"]);
  // A change to the import of a module on the path changes the ordering of the importer.
  expect([...watched].sort()).toEqual(["button.tsx", "ui.ts", "wrapper.tsx"]);
});
