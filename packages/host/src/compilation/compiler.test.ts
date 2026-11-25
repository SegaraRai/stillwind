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

/**
 * Compiles `app.tsx` from in-memory files and returns the ordering modules of each compiled
 * source with the number of host resolutions of each request.
 *
 * @param files - Module texts keyed by path relative to the virtual root.
 * @returns Ordering module identities keyed by compiler identity, and resolution counts keyed by
 * `importer -> specifier`.
 */
async function compileOrdering(files: Record<string, string>) {
  const root = path.resolve("ordering");
  const file = (id: string) => path.relative(root, id).replaceAll("\\", "/");
  const ordering = new Map<string, string[]>();
  const resolutions = new Map<string, number>();
  const session = {
    scanSource,
    compile: (
      sources: {
        moduleId: string;
        orderingModules?: {
          moduleId: string;
        }[];
      }[],
    ) => {
      for (const source of sources) {
        ordering.set(
          source.moduleId,
          (source.orderingModules ?? []).map((entry) => entry.moduleId),
        );
      }
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
        const request = `${file(importer)} -> ${specifier}`;
        resolutions.set(request, (resolutions.get(request) ?? 0) + 1);
        if (!specifier.startsWith("./")) {
          return null;
        }
        const base = path.join(path.dirname(importer), specifier);
        const name = [".tsx", ".ts"]
          .map((extension) => base + extension)
          .find((id) => file(id) in files);
        return name ? { id: name } : null;
      },
      read: async (id) => files[file(id)]!,
      describe: (id) => fileModuleDescriptor(root, id),
      watch: () => {},
      warn: (message) => expect.fail(message),
    },
  );
  return {
    ordering,
    resolutions,
  };
}

const button = `import { style } from "stillwind/react";
export const Button = (props: object) => <button {...style("p-2", style.forward(props))} />;
`;
const card = `import { style } from "stillwind/react";
export const Card = () => <div {...style("p-1")} />;
`;

test("a named import through export * orders only the module that exports the name", async () => {
  const { ordering } = await compileOrdering({
    "app.tsx": `import { style } from "stillwind/react";
import { Button } from "./ui";
import * as all from "./all";
export const App = () => <Button {...style("p-4")} />;
export const Both = all;
`,
    "ui.ts": `export * from "./button";
export * from "./card";
`,
    "all.ts": `export * from "./button";
export * from "./card";
`,
    "button.tsx": button,
    "card.tsx": card,
  });
  expect(ordering.get("app.tsx")?.sort()).toEqual(["button.tsx", "button.tsx", "card.tsx"]);
});

test("a module that several imports reach is traversed once per graph", async () => {
  const { ordering, resolutions } = await compileOrdering({
    "app.tsx": `import { style } from "stillwind/react";
import { First } from "./first";
import { Second } from "./second";
export const App = () => <><First {...style("p-4")} /><Second /></>;
`,
    "first.tsx": `import { Button } from "./ui";
export const First = (props: object) => <Button {...props} />;
`,
    "second.tsx": `import { Button } from "./ui";
export const Second = (props: object) => <Button {...props} />;
`,
    "ui.ts": `export * from "./button";
export * from "./card";
`,
    "button.tsx": button,
    "card.tsx": card,
  });
  expect(ordering.get("app.tsx")).toEqual(["button.tsx", "button.tsx"]);
  expect(resolutions.get("ui.ts -> ./button")).toBe(1);
});
